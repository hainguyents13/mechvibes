//! Linux: asks the sound server, falling back to ALSA.
//!
//! Desktop Linux runs PulseAudio or PipeWire, and PipeWire answers the
//! PulseAudio protocol too, so one client covers both. A recording stream
//! ("source output") that is not paused, reads from a real input rather than
//! the monitor of a speaker, and is not a level meter means an app is
//! recording from a microphone. Screen recorders that capture system sound
//! read a monitor, so they do not count.
//!
//! Without a sound server, the kernel's ALSA status files say whether any
//! capture device is running.

use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::{ Duration, Instant };

use pulseaudio::protocol::{ self, Command, CommandReply, Prop, SourceInfoList, SourceOutputInfo, SourceOutputInfoList };

/// How long one question to the sound server may take before it counts as
/// failed. The watcher asks once a second.
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// After a failed connection, how long to wait before trying the server again.
/// Meanwhile the ALSA files answer.
const RECONNECT_DELAY: Duration = Duration::from_secs(30);

/// PA_NATIVE_COOKIE_LENGTH.
const COOKIE_LENGTH: usize = 256;

/// Level meters, which record to draw a bar and are not a call.
const LEVEL_METER_APPS: &[&[u8]] = &[b"org.PulseAudio.pavucontrol"];

pub(super) struct Detector {
    server: Option<SoundServer>,
    /// When to try connecting again, after a failed attempt.
    retry_at: Option<Instant>,
}

impl Detector {
    pub(super) fn new() -> Self {
        Self { server: None, retry_at: None }
    }

    /// True when some app is recording from a microphone right now. A failed
    /// question counts as "not recording".
    pub(super) fn microphone_in_use(&mut self) -> bool {
        if self.server.is_none() && self.retry_at.is_none_or(|at| Instant::now() >= at) {
            match SoundServer::connect() {
                Ok(server) => {
                    self.server = Some(server);
                    self.retry_at = None;
                }
                Err(e) => {
                    crate::always_eprint!("⚠️ [MicActivity] No sound server, checking ALSA instead: {}", e);
                    self.retry_at = Some(Instant::now() + RECONNECT_DELAY);
                }
            }
        }
        let Some(server) = &mut self.server else {
            return alsa_capture_running();
        };
        match server.microphone_in_use() {
            Ok(in_use) => in_use,
            Err(e) => {
                crate::always_eprint!("⚠️ [MicActivity] Lost the sound server, reconnecting: {}", e);
                self.server = None;
                false
            }
        }
    }
}

/// One connection to the PulseAudio (or PipeWire) server, kept open between
/// questions.
struct SoundServer {
    socket: BufReader<UnixStream>,
    version: u16,
    sequence: u32,
}

impl SoundServer {
    fn connect() -> Result<Self, protocol::ProtocolError> {
        let path = pulseaudio::socket_path_from_env().ok_or_else(|| not_found("no sound server socket"))?;
        let socket = UnixStream::connect(path)?;
        socket.set_read_timeout(Some(REPLY_TIMEOUT))?;
        socket.set_write_timeout(Some(REPLY_TIMEOUT))?;
        let mut server = Self { socket: BufReader::new(socket), version: protocol::MAX_VERSION, sequence: 0 };

        let auth: protocol::AuthReply = server.ask(
            Command::Auth(protocol::AuthParams {
                version: protocol::MAX_VERSION,
                supports_shm: false,
                supports_memfd: false,
                cookie: auth_cookie(),
            })
        )?;
        server.version = protocol::MAX_VERSION.min(auth.version);

        let mut props = protocol::Props::new();
        props.set(Prop::ApplicationName, c"Mechvibes microphone check");
        let _: protocol::SetClientNameReply = server.ask(Command::SetClientName(props))?;
        Ok(server)
    }

    fn ask<T: CommandReply>(&mut self, command: Command) -> Result<T, protocol::ProtocolError> {
        self.sequence = self.sequence.wrapping_add(1);
        protocol::write_command_message(self.socket.get_mut(), self.sequence, &command, self.version)?;
        let (_, reply) = protocol::read_reply_message::<T>(&mut self.socket, self.version)?;
        Ok(reply)
    }

    fn microphone_in_use(&mut self) -> Result<bool, protocol::ProtocolError> {
        let outputs: SourceOutputInfoList = self.ask(Command::GetSourceOutputInfoList)?;
        if outputs.is_empty() {
            return Ok(false);
        }
        let sources: SourceInfoList = self.ask(Command::GetSourceInfoList)?;
        let monitors: Vec<u32> = sources
            .iter()
            .filter(|source| source.monitor_of_sink_index.is_some())
            .map(|source| source.index)
            .collect();
        Ok(outputs.iter().any(|output| is_microphone_recording(output, &monitors)))
    }
}

/// The server's cookie, or zeros when there is none. The server reads exactly
/// `COOKIE_LENGTH` bytes and drops a client that sends fewer, so a missing
/// cookie (common under PipeWire) is sent as zeros, as libpulse does.
fn auth_cookie() -> Vec<u8> {
    pulseaudio
        ::cookie_path_from_env()
        .and_then(|path| std::fs::read(path).ok())
        .filter(|cookie| cookie.len() == COOKIE_LENGTH)
        .unwrap_or_else(|| vec![0; COOKIE_LENGTH])
}

/// Whether this recording stream is an app recording from a microphone.
fn is_microphone_recording(output: &SourceOutputInfo, monitor_sources: &[u32]) -> bool {
    let level_meter =
        output.resample_method.as_deref() == Some(c"peaks") ||
        output.props
            .get(Prop::ApplicationId)
            .is_some_and(|id| LEVEL_METER_APPS.contains(&without_nul(id)));
    // A stream with no client belongs to a server module, such as a loopback
    // or echo canceller, which stays open whether or not anyone is talking.
    let from_an_app = output.client_index.is_some();
    from_an_app && !output.corked && !level_meter && !monitor_sources.contains(&output.source_index)
}

/// Property values arrive with their C string terminator.
fn without_nul(value: &[u8]) -> &[u8] {
    value.strip_suffix(&[0]).unwrap_or(value)
}

fn not_found(message: &str) -> protocol::ProtocolError {
    std::io::Error::new(std::io::ErrorKind::NotFound, message.to_string()).into()
}

/// Whether any ALSA capture device is running, from
/// `/proc/asound/card*/pcm*c/sub*/status`.
fn alsa_capture_running() -> bool {
    let Ok(cards) = std::fs::read_dir("/proc/asound") else {
        return false;
    };
    cards
        .flatten()
        .filter(|card| card.file_name().to_string_lossy().starts_with("card"))
        .filter_map(|card| std::fs::read_dir(card.path()).ok())
        .flatten()
        .flatten()
        .filter(|pcm| {
            let name = pcm.file_name().to_string_lossy().to_string();
            name.starts_with("pcm") && name.ends_with('c')
        })
        .filter_map(|pcm| std::fs::read_dir(pcm.path()).ok())
        .flatten()
        .flatten()
        .any(|sub| {
            std::fs::read_to_string(sub.path().join("status")).is_ok_and(|status| alsa_status_is_running(&status))
        })
}

/// An ALSA substream status file says `state: RUNNING` while it captures, and
/// `closed` when nothing has it open.
fn alsa_status_is_running(status: &str) -> bool {
    status.lines().any(|line| line.trim() == "state: RUNNING")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn recording(source_index: u32) -> SourceOutputInfo {
        SourceOutputInfo { source_index, client_index: Some(7), ..Default::default() }
    }

    #[test]
    fn an_app_recording_a_microphone_counts() {
        assert!(is_microphone_recording(&recording(1), &[2]));
    }

    #[test]
    fn recording_the_speakers_paused_streams_and_server_modules_do_not_count() {
        assert!(!is_microphone_recording(&recording(2), &[2]), "a monitor source is system sound");
        assert!(!is_microphone_recording(&SourceOutputInfo { corked: true, ..recording(1) }, &[]));
        assert!(!is_microphone_recording(&SourceOutputInfo { client_index: None, ..recording(1) }, &[]));
    }

    #[test]
    fn level_meters_do_not_count() {
        let peaks = SourceOutputInfo { resample_method: Some(CString::new("peaks").unwrap()), ..recording(1) };
        assert!(!is_microphone_recording(&peaks, &[]));

        let mut pavucontrol = recording(1);
        pavucontrol.props.set(Prop::ApplicationId, CString::new("org.PulseAudio.pavucontrol").unwrap());
        assert!(!is_microphone_recording(&pavucontrol, &[]));
    }

    #[test]
    fn alsa_status_running_only_while_capturing() {
        assert!(alsa_status_is_running("state: RUNNING\nowner_pid   : 1234\ntrigger_time: 1.0\n"));
        assert!(!alsa_status_is_running("closed\n"));
        assert!(!alsa_status_is_running("state: PREPARED\n"));
    }
}
