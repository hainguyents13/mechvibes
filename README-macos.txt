MechvibesDX for macOS - EXPERIMENTAL, UNTESTED
==============================================

READ THIS BEFORE INSTALLING.

This build is produced by CI and has never been run on a real Mac by the
maintainer. It carries only an ad-hoc signature and is NOT notarized. It is
published so that macOS users can try it and report back - not because it is
known to work.

Known and expected rough edges:
  - Gatekeeper will refuse a normal double-click on first launch.
  - Input capture needs Accessibility and Input Monitoring permission, which
    the app cannot request properly without a signed bundle.
  - The tray icon, audio device switching, and soundpack loading are all
    unverified on macOS.

If something is broken, please open an issue with the macOS version and the
console output - that feedback is the whole point of this build.


1. Installing
--------------

Open the .dmg and drag MechvibesDX onto the Applications shortcut inside it.
Then eject the disk image.


2. Clearing Gatekeeper
-----------------------

Because the app is not notarized, macOS quarantines it on download. The
simplest fix, and the one to prefer:

  Right-click (or Control-click) MechvibesDX in Applications, choose "Open",
  then confirm the warning dialog.

That whitelists this one app; afterwards it opens normally. A plain
double-click on first launch will just be refused.

If that is not offered, or macOS reports the app "is damaged and can't be
opened", clear the quarantine flag by hand - it is the same problem, not
actual corruption:

  xattr -cr /Applications/MechvibesDX.app


3. Accessibility and Input Monitoring permissions
-------------------------------------------------

Global key capture requires BOTH:

  System Settings -> Privacy & Security -> Accessibility
  System Settings -> Privacy & Security -> Input Monitoring

Add /Applications/MechvibesDX.app to each list and enable the toggle.

  - Accessibility covers key capture while the MechvibesDX window is focused.
  - Input Monitoring covers key capture while the window is minimized or
    another app is focused. Without it, sounds only play while the
    MechvibesDX window has focus.

Click the MechvibesDX icon in the menu bar to see where you stand. Each
permission is listed there: "Granted" in green when it is on, or a red
"Grant ..." line when it is missing. Clicking that line opens System
Settings at the right page.

You have to remove and re-add both entries after replacing the app with a
newer build, because macOS keys the permission to the binary's identity. The
toggle can look on and still not work. Or reset them from a terminal and let
macOS prompt again on the next launch:

  tccutil reset ListenEvent com.hainguyents13.mechvibesdx
  tccutil reset Accessibility com.hainguyents13.mechvibesdx


4. Where your settings live
---------------------------

The app bundle itself is read-only, so settings, custom soundpacks and themes
are stored in your home directory:

  ~/Library/Application Support/Mechvibes/

Deleting the app does not remove that folder; delete it by hand for a clean
uninstall.


5. Architecture
---------------

The asset filename ends in the CPU architecture it was built for:

  arm64   Apple Silicon (M1 and later)
  x86_64  Intel

There is no universal binary yet. If your architecture is not published in a
release, build from source: `cargo build --release`.


6. Updates
----------

The in-app auto-updater only handles the Windows installer. On macOS,
download new releases manually from the project's GitHub Releases page.
