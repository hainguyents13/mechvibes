//! Asks before installing a pack that a `mechvibes://install/<id>` link
//! points at, then downloads and installs it.
//!
//! Mounted once at the root, so a link opens it whatever page is showing. A
//! link only names a pack; nothing is downloaded until the user has seen its
//! name, author and size and clicked Install.

use crate::libs::audio::AudioContext;
use crate::state::app::use_state_trigger;
use crate::utils::library_client::{ self, LibraryPack };
use crate::utils::soundpack_installer::{ self, LibraryInstall };
use dioxus::prelude::*;
use lucide_dioxus::Download;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
enum Stage {
    Hidden,
    Loading,
    Confirm {
        pack: LibraryPack,
        kind: LibraryInstall,
    },
    Working {
        pack: LibraryPack,
        updating: bool,
    },
    Done {
        message: String,
        updating: bool,
    },
    Failed(String),
}

/// `418 KB`, `1.2 MB`.
pub(crate) fn format_size(bytes: u64) -> String {
    if bytes < 1024 * 1024 {
        format!("{} KB", (bytes + 1023) / 1024)
    } else {
        format!("{:.1} MB", (bytes as f64) / 1024.0 / 1024.0)
    }
}

fn category_label(pack: &LibraryPack) -> &'static str {
    match pack.soundpack_type() {
        Some(crate::state::soundpack::SoundpackType::Mouse) => "Mouse",
        _ => "Keyboard",
    }
}

#[component]
pub fn LibraryInstallModal() -> Element {
    let audio_ctx: Arc<AudioContext> = use_context();
    let state_trigger = use_state_trigger();
    let mut stage = use_signal(|| Stage::Hidden);

    // Each link that arrives opens the dialog, unless one is already asking
    // or installing. A page could otherwise swap the pack under the user's
    // cursor just before they click Install. Clicking the link again after
    // closing the dialog works as usual.
    use_hook(move || {
        spawn(async move {
            let Some(mut ids) = crate::utils::deep_link::take_pack_receiver() else {
                return;
            };
            while let Some(id) = ids.recv().await {
                if
                    matches!(
                        *stage.peek(),
                        Stage::Loading | Stage::Confirm { .. } | Stage::Working { .. }
                    )
                {
                    continue;
                }
                stage.set(Stage::Loading);
                match library_client::fetch_pack(&id).await {
                    Ok(pack) => {
                        let kind = match pack.soundpack_type() {
                            Some(soundpack_type) =>
                                soundpack_installer::library_install_kind(
                                    &pack.id,
                                    soundpack_type,
                                    pack.revision
                                ),
                            None => LibraryInstall::New,
                        };
                        stage.set(Stage::Confirm { pack, kind });
                    }
                    Err(e) => stage.set(Stage::Failed(e)),
                }
            }
        })
    });

    let install = move |_| {
        let Stage::Confirm { pack, kind } = stage() else {
            return;
        };
        let updating = matches!(kind, LibraryInstall::Update { .. });
        stage.set(Stage::Working { pack: pack.clone(), updating });
        let audio_ctx = audio_ctx.clone();
        spawn(async move {
            let result = match library_client::download_pack(&pack).await {
                Ok(zip) => {
                    let zip_path = zip.to_string_lossy().to_string();
                    // The same structure check a ZIP import runs, then the
                    // unpacking off the UI thread.
                    let installed = match
                        crate::utils::soundpack_validator::validate_soundpack_structure(
                            &zip_path
                        ).await
                    {
                        Ok(_) => {
                            let (id, soundpack_type) = (pack.id.clone(), pack.soundpack_type());
                            let zip_path = zip_path.clone();
                            tokio::task
                                ::spawn_blocking(move || {
                                    soundpack_installer::install_library_pack(
                                        &zip_path,
                                        &id,
                                        soundpack_type
                                    )
                                }).await
                                .unwrap_or_else(|_| Err("The install stopped unexpectedly.".to_string()))
                        }
                        Err(e) => Err(e),
                    };
                    let _ = std::fs::remove_file(&zip);
                    installed
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(info) => {
                    // Picks up the new files when the updated pack is the one
                    // playing, and lists a new one.
                    crate::state::app::reload_current_soundpacks(&audio_ctx);
                    state_trigger.call(());
                    let verb = if updating { "Updated" } else { "Installed" };
                    stage.set(Stage::Done {
                        message: format!("{} {}. Pick it on the Home page to hear it.", verb, info.name),
                        updating,
                    });
                }
                Err(e) => stage.set(Stage::Failed(e)),
            }
        });
    };

    let close = move |_| {
        if !matches!(*stage.peek(), Stage::Working { .. }) {
            stage.set(Stage::Hidden);
        }
    };

    let current = stage();
    if current == Stage::Hidden {
        return rsx! {};
    }
    let busy = matches!(current, Stage::Loading | Stage::Working { .. });

    rsx! {
      div { class: "modal modal-open",
        div { class: "modal-box",
          h3 { class: "font-bold text-lg mb-4",
            match &current {
                Stage::Confirm { kind: LibraryInstall::Update { .. }, .. } => "Update sound pack",
                Stage::Working { updating: true, .. } => "Updating sound pack",
                Stage::Done { updating: true, .. } => "Sound pack updated",
                Stage::Done { .. } => "Sound pack installed",
                _ => "Install sound pack",
            }
          }

          match &current {
              Stage::Loading => rsx! {
                div { class: "flex items-center gap-3 text-sm text-base-content/70",
                  span { class: "loading loading-spinner loading-sm" }
                  "Looking up the sound pack..."
                }
              },
              Stage::Confirm { pack, .. } | Stage::Working { pack, .. } => rsx! {
                div { class: "card border border-base-300 bg-base-200 p-4 space-y-1",
                  div { class: "font-medium text-base-content", "{pack.name}" }
                  if let Some(author) = &pack.author {
                    div { class: "text-sm text-base-content/70", "by {author}" }
                  }
                  div { class: "text-xs text-base-content/60",
                    "{category_label(pack)} · Version {pack.revision} · {format_size(pack.zip_size)}"
                  }
                }
              },
              Stage::Done { message, .. } => rsx! {
                div { class: "alert alert-success alert-soft", "{message}" }
              },
              Stage::Failed(message) => rsx! {
                div { class: "alert alert-error alert-soft", "{message}" }
              },
              _ => rsx! {},
          }

          if let Stage::Confirm { kind, .. } = &current {
            match kind {
                LibraryInstall::Update { installed: Some(installed) } => rsx! {
                  div { class: "text-sm text-base-content/70 mt-3", "You have version {installed}. Updating replaces it." }
                },
                LibraryInstall::Update { installed: None } => rsx! {
                  div { class: "text-sm text-base-content/70 mt-3", "You already have this pack. Updating replaces it." }
                },
                LibraryInstall::Current { installed } => rsx! {
                  div { class: "text-sm text-base-content/70 mt-3", "You already have version {installed} of this pack." }
                },
                LibraryInstall::New => rsx! {
                  div { class: "text-sm text-base-content/70 mt-3", "Only install sound packs you trust." }
                },
            }
          }

          div { class: "modal-action mt-6",
            button { class: "btn btn-sm btn-ghost", disabled: busy, onclick: close,
              if matches!(current, Stage::Confirm { .. }) { "Cancel" } else { "Close" }
            }
            match &current {
                Stage::Confirm { kind: LibraryInstall::Current { .. }, .. } => rsx! {},
                Stage::Confirm { kind, .. } => rsx! {
                  button { class: "btn btn-sm btn-neutral", onclick: install,
                    Download { class: "w-4 h-4 mr-2" }
                    if matches!(kind, LibraryInstall::Update { .. }) { "Update" } else { "Install" }
                  }
                },
                Stage::Working { updating, .. } => rsx! {
                  button { class: "btn btn-sm btn-neutral", disabled: true,
                    span { class: "loading loading-spinner loading-sm mr-2" }
                    if *updating { "Updating..." } else { "Installing..." }
                  }
                },
                _ => rsx! {},
            }
          }
        }
        div { class: "modal-backdrop", onclick: close }
      }
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn sizes_read_as_kilobytes_or_megabytes() {
        assert_eq!(format_size(428304), "419 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }
}
