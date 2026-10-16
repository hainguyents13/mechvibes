use crate::libs::tray::{
    handle_tray_events, store_tray, take_tray, with_tray, TrayManager, TrayMessage,
};
use crate::libs::tray_service::TRAY_UPDATE_SERVICE;
use crate::libs::window_manager::{ WindowAction, WINDOW_MANAGER };
use crate::libs::AudioContext;
use crate::{ debug_print, always_eprint };
use dioxus::desktop::use_window;
use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

/// Brings the main window back and in front, from hidden or minimized.
///
/// `set_visible(true)` + `set_focus()` is enough on Windows and Linux. On
/// macOS it is not for a window the user minimized to the Dock: tao's
/// `set_visible(true)` only orders the window front, which leaves a
/// miniaturized window where it is, and `set_focus()` returns early for a
/// minimized window, so nothing appears and the app is never activated. macOS
/// therefore restores it from the Dock before showing it, the order #97 was
/// written against, and activates the app explicitly afterwards. Windows and
/// Linux keep the other order: they un-minimize after showing.
///
/// The `set_focus()` below is not what raises the window on Linux or Windows:
/// it runs while `set_visible(true)` is still only queued for tao's event loop,
/// and tao drops a focus request for a window it has not shown yet. The focus is
/// asked for again, once the window reports itself visible, in the loop that
/// calls this function. It is kept here because it is the path macOS was
/// written against, where `activate_app()` then does the real work.
fn show_window(window: &dioxus::desktop::DesktopContext) {
    #[cfg(target_os = "macos")]
    crate::always_print!(
        "🔼 Show requested (macOS): minimized={}, visible={}",
        window.is_minimized(),
        window.is_visible()
    );

    // Restore from the Dock before showing. tao's `set_visible(true)` only
    // orders a miniaturized window front without un-minimizing it, so on macOS
    // the restore has to come first.
    #[cfg(target_os = "macos")]
    window.set_minimized(false);

    window.set_visible(true);

    #[cfg(not(target_os = "macos"))]
    window.set_minimized(false);

    window.set_focus();

    #[cfg(target_os = "macos")]
    crate::libs::native_window::activate_app();
}

#[component]
pub fn WindowController() -> Element {
    let window = use_window();

    // The tray's mute item toggles the same engine-cached flag the UI does,
    // so it has to notify the engine rather than only rewriting the config.
    let audio_ctx = use_context::<Arc<AudioContext>>();

    // `set_sound_enabled` persists the flag but does not publish it to the
    // shared config signal, so the UI would keep rendering the pre-toggle mute
    // state until some unrelated write happened to republish it.
    let (_config, update_config) = crate::utils::config::use_config();

    // Bring the window inside the screen before anything else runs.
    //
    // This cannot happen at build time in `main.rs`: which monitor the window
    // opens on, and that monitor's work area and scale factor, are only known
    // once the window exists. `use_hook` runs it exactly once per mount, on the
    // first render, so the correction lands before the user sees the window
    // rather than as a visible jump afterwards.
    {
        let window = window.clone();
        use_hook(move || {
            crate::libs::window_bounds::fit_window_to_monitor(
                &window,
                crate::libs::window_bounds::DEFAULT_WINDOW_SIZE,
                crate::libs::window_bounds::MIN_WINDOW_SIZE
            );
        });
    }

    // The window-action receiver is created exactly once, on the first render,
    // and handed to the loop below. It cannot live in a `Signal`: on Linux that
    // loop runs on the glib main context, outside the Dioxus runtime, where
    // reading a `Signal` is not allowed. `use_hook` returns a cloneable handle
    // because a `Receiver` is not `Clone` itself; the receiver is taken out of
    // it once, before the driver is installed.
    let window_action_receiver = use_hook(|| {
        let (tx, rx) = mpsc::channel::<WindowAction>();
        WINDOW_MANAGER.set_action_sender(tx);
        Rc::new(RefCell::new(Some(rx)))
    });

    // Initialize tray using use_resource to avoid reactive scope warnings
    let _tray_init = use_resource(move || async move {
        match TrayManager::new() {
            Ok(tray) => {
                debug_print!("✅ System tray initialized successfully");
                store_tray(tray);
            }
            Err(e) => {
                always_eprint!("❌ Failed to initialize system tray: {}", e);
            }
        }
    });

    // Release the tray when this component goes away.
    //
    // The tray lives in a thread-local, so nothing in the virtual DOM owns it
    // any more and closing the window is not enough to release it. Only the
    // tray's own Exit item used to, which left the icon behind in the
    // notification area on Windows whenever the window was closed from the
    // titlebar or after an update install. Releasing it here covers every one of
    // those paths.
    //
    // The explicit release in the Exit item stays as well, so that path
    // releases the icon before the window closes rather than after; the second
    // attempt finds nothing left to release.
    //
    // Hiding the window does not reach this: the component stays mounted, which
    // is what keeps the tray available to bring the window back.
    use_drop(|| {
        drop(take_tray());
    });

    // Use effect to listen for both window actions and tray events
    use_effect(move || {
        let window_clone = window.clone();
        let audio_ctx = audio_ctx.clone();
        let update_config = update_config.clone();

        // Take the receiver once, before the driver is installed: the loop
        // owns it rather than reading a Signal per tick, so it can run outside
        // the Dioxus runtime.
        let Some(window_action_receiver) = window_action_receiver.borrow_mut().take() else {
            return;
        };

        // Set by the show paths below and cleared once the window has actually
        // become visible. See the focus block near the end of the tick.
        let mut pending_show_focus = false;

        let mut tick = move || {
            // Handle window actions from internal sources
            if let Ok(action) = window_action_receiver.try_recv() {
                match action {
                    WindowAction::Show => {
                        show_window(&window_clone);
                        WINDOW_MANAGER.set_visible(true);
                        pending_show_focus = true;
                        crate::always_print!("🔼 Window show requested from internal action");
                    }
                    WindowAction::Hide => {
                        window_clone.set_visible(false);
                        WINDOW_MANAGER.set_visible(false);
                        // A show that has not been applied yet must not survive
                        // the hide: the focus block below runs on a later tick
                        // and would raise the window again after it was hidden.
                        pending_show_focus = false;
                        crate::always_print!("🔽 Window hidden from internal action");
                    }
                }
            }
            // Handle tray update requests from other parts of the application
            if let Some(_) = TRAY_UPDATE_SERVICE.try_receive() {
                let _ = with_tray(|tray| {
                    if let Err(e) = tray.update_menu() {
                        crate::always_eprint!("❌ Failed to update tray menu from global request: {}", e);
                    } else {
                        crate::always_print!("✅ Tray menu updated from global request");
                    }
                });
            }

            // Handle tray events
            if let Some(tray_message) = handle_tray_events() {
                match tray_message {
                    TrayMessage::Show => {
                        // Only ask for the show here. Asking for the focus on
                        // the next line drops it: see the block near the end
                        // of this loop for why.
                        show_window(&window_clone);
                        WINDOW_MANAGER.set_visible(true);
                        pending_show_focus = true;
                        crate::always_print!("🔼 Window show requested from tray");
                    }
                    TrayMessage::ToggleMute => {
                        // Toggle the global sound enable flag. This goes
                        // through the audio context (not a bare config
                        // write) so the engine thread, which caches this
                        // flag in its own state, actually stops playing.
                        let enabled = !audio_ctx.is_sound_enabled();
                        audio_ctx.set_sound_enabled(enabled);
                        // Publish the new flag so the mute button and the
                        // disabled sliders re-render; the audio context
                        // only writes the file.
                        update_config(
                            Box::new(move |config| {
                                config.enable_sound = enabled;
                            })
                        );
                        debug_print!(
                            "🔇 Sounds {} via tray menu",
                            if enabled { "enabled" } else { "disabled" }
                        );
                        // Update tray menu to reflect new state
                        let _ = with_tray(|tray| {
                            if let Err(e) = tray.update_menu() {
                                always_eprint!("❌ Failed to update tray menu: {}", e);
                            }
                        });
                    }
                    TrayMessage::OpenGitHub => {
                        let url = "https://github.com/hainguyents13/mechvibes-dx";
                        if let Err(e) = open::that(url) {
                            always_eprint!("❌ Failed to open GitHub URL: {}", e);
                        } else {
                            debug_print!("🐙 Opened GitHub repository in browser");
                        }
                    }
                    TrayMessage::OpenDiscord => {
                        let url = "https://discord.com/invite/MMVrhWxa4w";
                        if let Err(e) = open::that(url) {
                            crate::always_eprint!("❌ Failed to open Discord URL: {}", e);
                        } else {
                            crate::always_print!("💬 Opened Discord community in browser");
                        }
                    }
                    TrayMessage::OpenWebsite => {
                        let url = "https://mechvibes.com";
                        if let Err(e) = open::that(url) {
                            crate::always_eprint!("❌ Failed to open website URL: {}", e);
                        } else {
                            crate::always_print!("🌐 Opened official website in browser");
                        }
                    }
                    TrayMessage::Exit => {
                        crate::always_print!("📢 Tray: Exit requested - closing application");
                        // The tray lives in a thread-local. Dropping it here
                        // removes it from the notification area before the
                        // window closes, which is what Windows needs to not
                        // keep a ghost icon; the release on teardown would
                        // otherwise only happen once the window is gone.
                        drop(take_tray());
                        // Close the window which will trigger app exit
                        window_clone.close();
                    }
                }
            }
            // `set_visible` only queues a request for tao's event loop to
            // apply, and tao's `set_focus` refuses to queue a focus request
            // unless the window already reports itself visible. Called back
            // to back, the focus is dropped every time, so a window brought
            // back from the tray returned without focus and stayed behind
            // whatever was already on screen. Send the focus on a later tick,
            // once the visibility request has been applied.
            if pending_show_focus && window_clone.is_visible() {
                window_clone.set_focus();
                pending_show_focus = false;
                crate::always_print!("🔼 Window raised and focused");
            }
        };

        // Linux: the Dioxus runtime stops polling its tasks while the window is
        // unmapped (`set_visible(false)`, what "Hide to tray" does), so nothing
        // would drain `MenuEvent::receiver()` and every tray click - `Show`
        // included - would be silently ignored. The glib main context keeps
        // running while the window is hidden, so the loop is driven from a
        // glib timeout source instead. Dioxus runs its VDOM on that same main
        // thread, so the `!Send` tray and window objects never leave it.
        #[cfg(target_os = "linux")]
        {
            let _ = glib::timeout_add_local(
                std::time::Duration::from_millis(50),
                move || {
                    tick();
                    glib::ControlFlow::Continue
                }
            );
        }

        // Windows and macOS keep the async timer they always had. Windows never
        // unmaps the window, so the runtime keeps polling. macOS does unmap it
        // (see #113), and only some of its tray items are handled by AppKit on
        // the main thread: Show and the two permission items. Mute, Exit and
        // the two links still wait for this loop to be polled, so on macOS they
        // keep exactly the dependency the Linux driver above exists to remove.
        #[cfg(not(target_os = "linux"))]
        {
            spawn(async move {
                loop {
                    tick();
                    futures_timer::Delay::new(std::time::Duration::from_millis(50)).await;
                }
            });
        }
    });

    rsx! {
        // This component doesn't render anything visible
        span { style: "display: none;" }
    }
}

#[cfg(test)]
mod tests {
    /// The tray release, checked against the source that ships.
    ///
    /// The tray lives in a thread-local, so the virtual DOM no longer owns it:
    /// closing the window from the titlebar, or after an update install, goes
    /// through the window's own close rather than through the tray's Exit item.
    /// Losing the release on the way out leaves the icon behind in the
    /// notification area on Windows, and no other test can see that.
    #[test]
    fn the_window_releases_the_tray_when_it_goes_away() {
        const SOURCE: &str = include_str!("window_controller.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes the tests");

        let release_hook = runtime
            .split("use_drop(")
            .nth(1)
            .expect(
                "the window must release the tray when the component goes away, which is what covers closing from the titlebar and closing after an update install"
            )
            .split("});")
            .next()
            .expect("the release hook must be closed");
        assert!(
            release_hook.contains("take_tray()"),
            "the release hook must drop the tray the thread-local holds"
        );
        let exit_item = runtime
            .split("TrayMessage::Exit =>")
            .nth(1)
            .expect("the tray's own Exit item must keep its release")
            // The match ends where the comment about `set_visible` begins, so the
            // region stops there. Unbounded, the searches below would run to the
            // end of the component and could match an unrelated later call,
            // letting this pass for the wrong reason.
            .split("`set_visible` only queues")
            .next()
            .expect("the Exit item must end before the rest of the component");
        let release_at = exit_item
            .find("take_tray()")
            .expect("the Exit item must take the tray out itself");
        let close_at = exit_item
            .find(".close()")
            .expect("the Exit item must close the window");
        assert!(
            release_at < close_at,
            "the Exit item must take the icon out before it closes the window, not after"
        );
    }

    /// The tray loop's driver on Linux, checked against the source that ships.
    ///
    /// The failure this guards is invisible to every other test: the Dioxus
    /// runtime stops polling its tasks while the window is unmapped, so moving
    /// the loop back onto `spawn` silently makes every tray menu click - "Show"
    /// included - get ignored again. Only a source guard can catch that
    /// regression before a user does.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_drives_the_tray_loop_from_glib_not_the_dioxus_runtime() {
        const SOURCE: &str = include_str!("window_controller.rs");
        let runtime = SOURCE.split("#[cfg(test)]").next().expect("runtime code precedes the tests");

        let linux_driver = runtime
            .split("#[cfg(target_os = \"linux\")]")
            .nth(1)
            .expect("the Linux driver must be selected explicitly")
            .split("#[cfg(not(target_os = \"linux\"))]")
            .next()
            .expect("the Linux driver must end before the other platforms' driver");

        assert!(
            linux_driver.contains("glib::timeout_add_local"),
            "Linux must drive the tray loop from the glib main context"
        );
        assert!(
            linux_driver.contains("glib::ControlFlow::Continue"),
            "the glib timeout source must keep ticking"
        );
        assert!(
            !linux_driver.contains("futures_timer::Delay"),
            "Linux must not drive the tray loop with a Dioxus timer"
        );
        assert!(
            !linux_driver.contains("spawn("),
            "Linux must not run the tray loop on the Dioxus runtime"
        );

        let other_driver = runtime
            .split("#[cfg(not(target_os = \"linux\"))]")
            .nth(1)
            .expect("the other platforms' driver must be selected explicitly");
        assert!(
            other_driver.contains("futures_timer::Delay"),
            "Windows and macOS must keep the async timer they always had"
        );
        assert!(
            !other_driver.contains("glib::timeout_add_local"),
            "glib is a Linux-only dependency and must stay out of the other platforms' driver"
        );
    }
}
