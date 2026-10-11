# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **Numpad and system keys make sound again on Wayland.** The numpad keys, NumLock, ScrollLock, PrintScreen, Pause and the extra key next to the left Shift produced no sound on Wayland while X11 worked: the input-device table had no entry for them and dropped them silently. They are mapped to the same names X11 emits, so the sound packs that already define them keep working.

## [0.8.3] - 2026-10-07

### Changed

- **Logo and background customization now apply as you pick them.** The "Save changes" buttons are gone: choosing a colour or an image updates the app straight away, so what you see is the result rather than a preview you had to commit. Reset is still there and now takes effect immediately too.
- **The autostart settings use the same wording on every platform.** "Start with Windows" is now "Start at login", and its description and the "Start minimized" description no longer mention Windows.
- **macOS: permissions are now documented properly.** Key capture needs both **Accessibility** (while the window is focused) and **Input Monitoring** (while it is minimized or another app is in front). Without Input Monitoring the app runs normally but is silent unless its window has focus. Because the app is ad-hoc signed, macOS can keep showing the permissions as on after you install a new build while they have stopped working; remove and re-add MechvibesDX in both lists, or run `tccutil reset ListenEvent com.hainguyents13.mechvibesdx` and `tccutil reset Accessibility com.hainguyents13.mechvibesdx`. The README, `README-macos.txt` and the troubleshooting section now say so.
- **The macOS installer's name no longer says "experimental".** The DMG is now named `mechvibes-dx-<version>-macos-<arch>.dmg`, and its line in the release notes no longer calls it experimental or untested: it just says the app is ad-hoc signed, not notarized, and how to open it past Gatekeeper.

### Fixed

- **The Reset button under Logo and Background now works.** Pressing it appeared to do nothing and left the old colours in place. The panels kept their own copy of your settings and quietly wrote it back over anything you changed, which also made individual colour changes fail to stick.
- **Turning on background customization no longer leaves the background unchanged.** With no colour picked yet, the app wrote an empty style rule that browsers discard, so the setting looked active while nothing happened. It now starts from your theme's colour. Choosing "Use image" before picking a file also falls back to the colour instead of blanking the background.
- **macOS: the app no longer crashes on the first keystroke.** On recent macOS releases (Sonoma, Sequoia and later) the keyboard listener asked the system for the typed character from a background thread, which macOS only allows on the main thread, so the whole app was killed the moment you pressed a key. Typing now works while the window is focused and while it is minimized.
- **macOS: the keyboard listener for the focused window no longer errors out when Accessibility permission is missing.** It used to run a startup check that always asked macOS for Accessibility access with a prompt, and stopped with an error if access had not been granted. It now checks first without prompting, asks at most once if access is missing, and if it is still missing logs that and leaves that listener off. Turn Accessibility on as described in the README to bring it back.
- **macOS: "Start at login" now actually works.** The setting was saved but did nothing, and the app reset it to off on every launch because it could not see any login item. MechvibesDX now registers itself as a real login item, so it appears under System Settings > General > Login Items, where you can also remove it. This needs macOS 13 or later and the app installed in `/Applications`. If macOS asks you to allow it, the app opens that settings page for you, and if registering fails the switch turns itself back off instead of staying on for a login item that does not exist.
- **macOS: "Start minimized to tray" now takes effect.** It was ignored because of the reset above. It also no longer relies on a launch flag: it is read from your saved settings, and those are now written to disk the moment you flip either switch, so they survive a logout or reboot right after you change them. This applies on every platform. If both switches are on, opening the app by hand also starts it hidden, as it already did on Windows.
- **macOS: "Show MechvibesDX" in the menu bar now brings the window to the front.** After the app started minimized (for example from the login item), choosing it put the window on screen but behind whatever app you were using, so it looked like nothing happened. The app is now activated as well, and a window you had minimized to the Dock is restored. Windows and Linux are unchanged.

### For developers

- **A release now comes from the same run that built it.** `.github/workflows/ci.yml` replaces `release.yml` and is the only build workflow: a matrix (Linux, Windows and macOS, so macOS is checked on every pull request) that runs `scripts/ci-check.sh --release`, then each OS's packaging script. On a pull request into `main` it only verifies and uploads nothing; on a push to `main` (or manual dispatch) the packages are kept as artifacts of the run. Tagging a commit does not start a second run, so the commit is built once and the three packages and the draft release are never split across two runs: when the builds finish, a `Find release tag` job asks the remote whether a `v*` tag points at the built commit, and if one does, the same run extracts the release notes and drafts the release from those packages. The tag can be pushed with the commit or while the builds are running; if it arrives after the run has finished, re-run that job and nothing is rebuilt. The release is refused if the tag does not match `Cargo.toml`'s version or already has a release (draft or published). A later push no longer cancels a commit's run (only a pull request cancels its own superseded runs), so a release cannot be killed halfway through and no commit is skipped. The workflow only installs what a script cannot (the cache, the OS's system libraries, `cargo-deb`, Inno Setup); the Linux library list lives in one shared action, and only `main` saves the cache. Each script verifies what it built: the AppImage and DMG are unpacked or mounted and compared with the source tree, and the Windows installer's name is checked against the auto-updater's filter.
- **`scripts/lib/common.sh` gains `resolve_release_tag`**, which also runs locally: it prints the release tag the remote has for a commit.
- **The scripts check that Rust is at least the `rust-version` in `Cargo.toml`.** `scripts/ci-check.sh` and the packaging scripts fail up front with a clear message if the active Rust is older than that floor (1.88), instead of failing part-way through a build; any newer Rust passes. An old Rust gets a hint to run `rustup update stable`, and a `Cargo.toml` with no `rust-version` fails with a clear message.
- **`rdev` is vendored in `patches/rdev`** and wired in through `[patch.crates-io]`, because rdev 0.5.3 asks macOS for the typed character from the wrong thread (the crash fixed above). Only the macOS keyboard code differs from upstream. Drop the patch once rdev publishes a release with the fix.
- **Building and running from source now works out of the box.** `cargo build` copies the built-in sound packs next to the binary (`target/<profile>/soundpacks`), so `./target/release/mechvibes-dx` and `cargo run` play sound instead of loading zero packs. Nothing is deleted and a copy failure never fails the build.
- **The minimum supported Rust is now 1.88.0**, declared as `rust-version` in `Cargo.toml`, so an older compiler fails with a clear message and any newer Rust keeps working. `.mise.toml` pins the matching `dioxus-cli` 0.7.10.
- **The packaging scripts build for you.** `build-macos-app.sh` and `build-linux-appimage.sh` now run `cargo build --release --locked` first, and `build-windows-installer.ps1` builds with `--locked`. Each takes `--skip-build` (`-SkipBuild` on Windows) to reuse an existing binary, in which case it runs `cargo check --locked` instead, so a Rust older than `rust-version` or a stale `Cargo.lock` fails at the start. With the flag they also refuse a binary older than the sources it is built from, so a stale binary is never packaged under a new version. CI no longer needs the flag: each leg runs the script, which builds, packages and checks its own output.
- **`build-linux-appimage.sh` now also builds the `.deb`** from the same binary (`--no-deb` skips it on a machine without `cargo-deb`).
- **A release now needs every platform to build.** A failed Linux or macOS job used to be tolerated; it now stops the release rather than shipping without that platform's asset.

## [0.8.2] - 2026-08-16

### Added

- **Sound packs can now be sorted by name or by when you added them, and new ones are marked.** The lists have a sort button, and a pack you have just imported carries a "New" badge in both the pack manager and the pack picker until you select it. Previously the lists were ordered by the folder's modification time, which moved whenever anything inside a pack was touched and had nothing to do with when the pack arrived. Packs already in your library when you update keep their place and are not marked as new.
- **The sound on/off switch is now on the Home screen**, next to the volume sliders it controls, along with a link to the sound pack editor.
- **Updating is now one click.** The button downloads, installs and restarts by itself instead of asking a second time. If a download fails it turns into a Retry button and stays there until it works or a newer version appears, with the manual download link still available.
- **Run without the window: `mechvibes-dx --headless`.** Starts the sounds from a terminal with no window, no tray icon and no webview, using the same keyboard capture and the same settings as the normal app. Ctrl+Alt+M still mutes, Ctrl+C exits. This also gets the app working on machines where the window itself will not open, which on Windows means a broken or missing WebView2 and on Linux a broken webkit2gtk. Add `--soundpack <name>` (and `--mouse-soundpack <name>`) to try a different pack for that run only: your saved settings are read but never written, so nothing you have set up in the app changes.

### Fixed

- **Mouse sound packs from the community now actually make sound.** Packs made for Mechvibes++ were built in its keyboard editor, so their buttons were stored the way keyboard keys are. Importing one produced a pack that loaded, reported success, and then stayed completely silent no matter how much you clicked. Left, right and middle click are now recognised, and the separate press and release sounds are both kept instead of one replacing the other. Packs you imported before this update start working without being reinstalled.
- **The app no longer stalls for a second or two after every click.** Dragging a volume slider saved the settings file on each tiny movement, dozens of times per drag, and each save blocked the interface. Settings are now written once you stop moving the slider.
- **Imported sound packs no longer appear twice in the list.** A pack installed by you was recorded under two different names internally, so it showed up as a duplicate. Built-in packs were never affected. Press refresh in the Sound packs tab once after updating to clear the stale entries.
- **The audio device list is now filled in as soon as you open Settings**, instead of showing "click refresh to load available devices" until you did.
- **Exported logs no longer contain your Windows account name.** Log lines include file paths, which carry your user folder. The account name is now replaced with `[username]` in the exported file; everything else in the path is kept, since that is the part that helps with diagnosing.
- **A saved output device that no longer exists now falls back to your system default** and shows that in Settings, rather than leaving nothing selected. If you had picked a specific device before this update, it is reset to the system default once: the old way of remembering it could silently point at a different device after you plugged or unplugged anything, so it is not worth carrying over. Pick your device again and it will stay put from now on.
- **Converted Mechvibes packs no longer play some keys at chipmunk pitch.** When a classic pack mixed audio files recorded at different sample rates, the converter copied the odd ones in unchanged and labeled them with the wrong rate, so keys like the arrows, Home/End, or Delete played sped up and an octave too high. The converter now properly resamples every file to a common rate (and correctly folds down packs with unusual channel layouts). Already-converted packs keep the old audio: re-import the classic pack to get the corrected sound.
- **The window now opens fully on screen, whatever your display.** On a 1366x768 laptop the app was taller than the usable screen, so the dock and the buttons above it sat below the bottom edge and could not be reached. On some machines the window also opened partly outside the desktop entirely, with no way to drag it back because the title bar itself was off screen. The window is now measured against the work area of the monitor it opens on, shrunk to fit if it is too tall, and centred there. Display scaling at 125% and 150% is accounted for, the taskbar is excluded from the space considered usable, and monitors sitting left of or above the primary one are handled. When the window is shortened, the content scrolls instead of being cut off.

- **Ambiance sounds now play on installed Linux builds.** Rain, forest, campfire and the rest opened their audio files relative to whatever folder the app happened to be launched from, so on the `.deb` and the AppImage they were never found and the toggles did nothing. The bundled sounds are now located the same way soundpacks already were: inside the AppImage's own mount for AppImage users, under `/usr/lib/mechvibes-dx/assets` for the `.deb`, and beside the app everywhere else. When a sound genuinely cannot be opened, the error now names the exact path it tried.

## [0.8.1] - 2026-08-06

### Added

- **Linux AppImage**: releases now include a portable `mechvibes-dx-<version>-x86_64.AppImage`. Download, `chmod +x`, run on any distro. Like the `.deb`, it needs your user in the `input` group (`sudo usermod -a -G input $USER`, then re-log). Settings are stored in `~/.local/share/mechvibes`.

### Fixed

- **Windows settings now save on every install.** If you installed to `Program Files` (the default when you install for all users), the app tried to keep your settings inside that folder, which Windows does not let a normal user write to. Settings reset on every restart and importing a soundpack failed with "Access is denied (os error 5)". Settings now live in `%APPDATA%\Mechvibes`, alongside where your custom soundpacks were already kept, and importing works again. Your existing settings are copied over automatically the first time you run this version, so nothing is lost.
- **Linux settings now actually save.** The `.deb` build tried to write its config next to the binary in a system directory normal users cannot write to, so every setting silently reset on each launch. Settings now live in `~/.local/share/mechvibes` following the XDG convention, and a config previously saved by running as root is migrated over automatically.
- **Linux fonts and styling restored.** The `.deb` package never shipped the app's fonts and stylesheets, so the interface fell back to system fonts. They are now installed alongside the app.
- **Soundpack version checks were broken for every pack.** A type mismatch and a wrong field name meant no soundpack ever passed the version check as designed; sound still played, but import validation was running on luck. Also, a soundpack made for a future format version now gets a clear "requires a newer version of MechvibesDX" message instead of a confusing missing-fields error.
- **Discord release announcements no longer get cut off**: they now carry a one-line digest per change and link to the full notes.

## [0.8.0] - 2026-08-04

### Added

- **Debug section in Settings**: a live log viewer showing the app's most recent log lines in a small terminal-style window, an "Export logs" button that writes them to a file you can attach to bug reports, and a Verbose toggle that adds per-keystroke timing lines for diagnosing latency issues. Everything stays in memory and on your device until you press Export, key identities are always masked, and the Verbose toggle resets on every launch.

### Fixed

- **"Start with Windows" no longer switches itself off.** The app compared its registry entry against the wrong form of the path whenever "Start minimized" was also enabled, concluded autostart was off, and helpfully saved that conclusion over your setting. Both toggles now work, so Task Scheduler workarounds are no longer needed.
- **Keys used as global hotkeys by other apps now play sound.** If another program (Rainmeter, macro tools, and similar) claims a key like F1 as a hotkey, Windows hides the key press from us but still delivers the release; the app now recognizes that pattern and plays the sound anyway. Mechvibes classic behavior restored.
- **Launching the app while it is already running now brings up the running window** instead of doing nothing.
- **A damaged config file no longer wipes your settings.** Unknown or invalid fields fall back to sensible defaults for that field only, and if the file is truly unreadable it is preserved as `config.json.corrupt` instead of being overwritten, so nothing is lost. Hand-editors: a soundpack slot set to `""` is accepted and simply loads no pack for that slot.
- **Muting with Ctrl+Alt+M now updates the window immediately**, matching the tray and in-app buttons.
- The classic-Mechvibes converter backs up an existing converted audio file before overwriting it, and stops instead of proceeding when any backup fails.

### Changed

- **Every settings write now goes through a single writer with atomic file saves.** This is the structural fix behind the "my settings reverted" saga: previously twelve different places in the app could each save a whole copy of the settings, and whichever finished last silently undid the others. That mistake is now impossible to write, not merely discouraged, and a half-written settings file can no longer exist even if the app is killed mid-save.
- The app name, version and credits moved from the Home page to the bottom of Settings, leaving Home to the things you actually use.

## [0.7.2] - 2026-08-03

### Added

- **macOS DMG (experimental)**: the macOS build now ships as a proper `.dmg` with a drag-to-Applications app bundle instead of a bare-binary archive. Still unsigned and not notarized: right-click the app and choose Open the first time. Settings are stored in `~/Library/Application Support/Mechvibes`. Untested on real hardware; reports welcome.

### Fixed

- **Settings really stop reverting now.** The 0.7.1 fix covered one path but missed the main one: a background service kept a copy of your settings from the moment the app started and wrote it back seconds later (and again every 24 hours), undoing whatever you had changed in between. Whichever setting you touched first after opening the app was the one you would see reset, which is why reports about "which setting resets" never agreed. Verified against the running app this time, not only in tests.

## [0.7.1] - 2026-08-03

### Added

- **Anonymous usage statistics**: the app now sends one anonymous ping per launch (OS and app version, nothing else) so we can tell how many people use MechvibesDX daily. No keystrokes, no personal data, no persistent identifiers. You can turn it off in Settings, Privacy section; details in the README.

### Fixed

- **Settings no longer revert on their own shortly after starting the app.** The background update check kept a copy of your settings from launch time and wrote it back after checking for updates, silently undoing anything you changed in between (volume, theme, toggles). This mostly hit people on slower connections who start the app once a day, which is why it was hard to reproduce.
- **Volume changes on the Home page could be lost when switching tabs quickly.** The change is now applied immediately; only the disk write is delayed.
- **Muting from the tray menu now updates the window immediately** instead of showing a stale icon until something else changed.

## [0.7.0] - 2026-08-03

### Fixed

- **Severe input lag when a specific audio output device was selected**: the app checked device presence inside the audio engine loop every second, and that check could take up to ~2 seconds depending on your device's position in the system list — delaying both the sound and the logo animation by that much, seemingly at random. Sound now plays within ~15ms of a keystroke regardless of which output device is selected.
- **Vietnamese input methods (Telex/UniKey/EVKey) no longer cause bursts of clumped sounds**: when an IME rewrites text (e.g. "dd" → "đ", or restoring a mistyped syllable), it synthetically injects backspaces and replacement keys — each of which used to play a sound all at once. Software-injected keystrokes are now ignored; only keys you physically press make sound. Note this also means on-screen keyboards and macro tools no longer trigger sounds.
- **Logo animation keeps up with fast typing**: the pressed-state transition was too slow to render each stroke at speed; each keystroke now shows a distinct pulse.

### Changed

- **Removed the automatic switch to the system default output when the selected device disappears.** The background polling this required is gone completely — zero periodic device probing while you type. If you unplug the device you selected, the app stays silent until you pick another one in Settings (or restart); your saved choice is kept. Selecting "System Default" is unaffected — the OS handles device changes itself.
- Settings → Devices note updated: device changes apply immediately, no restart needed (restart remains a troubleshooting option).

### Added

- **Diagnostic trace mode**: launch with the environment variable `MECHVIBES_TRACE=1` to print one line per keystroke with per-stage timings (input capture → audio engine → sound → UI) — useful when reporting latency issues.

## [0.6.3] - 2026-08-03

### Fixed

- **The app no longer offers an update to the version you are already running.** A version recorded before a manual upgrade could linger and make the app advertise itself as an update; the saved value is now cleared at startup and every update prompt double-checks the version before showing anything.
- **The app now relaunches itself after a one-click update.** Previously "Restart to finish update" closed the app and the freshly installed version never started — an over-cautious installer guard was skipping the relaunch step. Verified end to end: close, silent install, automatic restart.

## [0.6.2] - 2026-08-03

### Added

- **One-click update install (Windows)**: when a new version is available, the Settings page now shows a "Download & install" button that downloads the installer in the background, verifies its SHA-256 checksum against the release's `SHA256SUMS.txt`, and — after you confirm — installs silently and relaunches the app. Nothing is downloaded until you click, and choosing "Later" keeps the verified download ready for next time. If anything fails (offline, checksum mismatch, older release without checksums), the button falls back to opening the download page as before.
- **Linux `.deb` package**: releases now include an installable Debian/Ubuntu package. Note: it does not add your user to the `input` group — run `sudo usermod -a -G input $USER` and re-log once after installing.
- **macOS build (experimental)**: an unsigned, untested arm64 build now ships with each release for adventurous testers; see the bundled README for Gatekeeper and Accessibility steps.
- Releases now include a `SHA256SUMS.txt` covering every asset.

### Changed

- **Tray icon dims while muted**, and the tray's mute entry is now a fixed-label "Mute sounds" item with a check mark instead of swapping text. The correct state also shows immediately when the app starts already muted.
- The update notification in the title bar now takes you to Settings instead of opening a browser download directly, so every install path goes through checksum verification.
- On Linux and macOS, the updater no longer offers the Windows installer; it links to the releases page instead.

### Fixed

- Removed noisy window-focus logging and dead internal plumbing left over from the pre-worker input architecture.

## [0.6.1] - 2026-08-02

### Fixed

- **Mute buttons had no effect on sound**: the mute toggles on the home page, in Settings, and in the tray menu only saved the preference without telling the running audio engine, and the icon could get out of sync and toggle back on the next click. All mute paths now apply immediately and stay consistent with the Ctrl+Alt+M hotkey.
- **Soundpack selector dropdown was transparent/unreadable**: a stale CSS variable from an older theme version made the dropdown background compute to transparent.
- **Device list disappeared when switching tabs**: the enumerated audio/input device list is now remembered for the rest of the session instead of resetting to the "refresh" placeholder every time you leave and re-enter Settings.
- **Reset to Defaults** now applies volume and mute state to the running engine immediately instead of requiring a restart.
- Removed constant config file read/write churn (the app was rewriting its config about once per second while typing) and noisy window-focus logging in release builds.

## [0.6.0] - 2026-08-02

### Added

- **Sound keeps working while the MechvibesDX window is focused (Windows)**: keyboard and mouse capture moved to a dedicated worker process using the Raw Input API, removing the old focused-window workaround (100Hz polling with ~10ms latency and missed keys during focus changes). Typing into the app window itself now sounds identical to typing anywhere else, and the Ctrl+Alt+M hotkey works regardless of focus.
- **Per-device input filtering on Windows**: disabling a specific keyboard or mouse in Settings now actually silences that device (and only that device), effective immediately without a restart. Previously the setting existed but had no effect.
- **Automatic fallback when the audio device disappears**: if the output device in use is unplugged (even mid-typing), sound automatically moves to the system default within a few seconds instead of going silent. Your saved device choice is kept on disk, so restarting the app returns to it once the device is back.

### Changed

- **Audio playback moved to a dedicated engine thread**: switching the output device in Settings now applies instantly at runtime (previously it only took effect after a restart, and could crash). The ambiance player follows the selected device too, including on startup. Input-to-sound latency is also slightly improved by replacing polling loops with blocking channel reads.
- The input worker process supervises itself: if it crashes it restarts automatically, if the app exits it shuts down with it, and if it can't be sustained the app falls back to the previous capture method so sound never fully stops.
- Removed the unused built-in music player (dead code — it was never reachable from the UI).

### Fixed

- Selecting an audio output device in Settings previously only saved the choice without applying it; ambiance sounds resume correctly after a device switch.
- Hardened the Windows input path: fixed a supervisor crash that could permanently stop input in debug builds after a healthy worker restart, a stuck-key state when disabling a keyboard while a key was held, undefined behavior in raw-input buffer handling, and missing system message cleanup.

## [0.5.2] - 2026-07-31

### Fixed

- **Sound cutting off / clicking when typing fast**: keyboard and mouse sounds now use a proper voice pool (oldest-first eviction) instead of a hashmap keyed by key name, so rapid repeated keys no longer cut each other's tails off. Added short fade-in/fade-out (2ms/5ms) to eliminate clicks/pops at segment boundaries, and soft (ramped) eviction instead of a hard cut when the voice pool is full.
- **Sound continuing to play after releasing all keys ("ghost typing")**: the keyboard/mouse/hotkey event loops now drain their entire backlog every tick instead of processing one event at a time, so a fast burst of keystrokes can no longer queue up and keep playing sound after the user has already lifted their hands.
- **Poor audio quality from realtime resampling**: soundpacks are now resampled once at load time to the output device's sample rate (using a high-quality sinc resampler) instead of relying on the audio backend's realtime linear resampling.
- Removed a redundant device probe on every soundpack load; the output device's sample rate is now probed once at startup and cached, avoiding unnecessary device enumeration (which could briefly interrupt audio on Linux/ALSA).
- Sample-rate lookup failures no longer fall back to a hardcoded 44100 Hz guess (which could cause audio to be resampled twice); they now skip resampling and keep the file's native rate instead.

### Changed

- Increased the keyboard/mouse voice pool limit (`max_voices`) from 20 to 32 to give more headroom for overlapping sound tails during fast typing.

## [0.5.1] and earlier

See git history for changes prior to this changelog's introduction.










