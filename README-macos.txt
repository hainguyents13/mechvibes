Mechvibes for macOS
===================

READ THIS BEFORE INSTALLING.

The app carries only an ad-hoc signature and is NOT notarized, so macOS
needs a few extra steps before it runs. They are all below.

Tested on a real Mac: 3.5.7 on an Apple Silicon M3 with macOS 27.3. Opening
it past Gatekeeper, both permissions, key capture in other apps, the menu
bar icon, every bundled soundpack and switching to headphones all worked.
The Intel build has not been tried on a real Mac yet.

If something is broken, please open an issue with your macOS version, your
Mac's chip and the console output.


1. Installing
--------------

Open the .dmg and drag Mechvibes onto the Applications shortcut inside it.
Then eject the disk image.

Coming from MechvibesDX (0.8.x and earlier)? The app has been renamed:
quit it, then move MechvibesDX from Applications to the Trash. Your settings
and sound packs are kept. If you had the original Mechvibes (the Electron
app, 2.x), Finder asks whether to replace it: this version is its successor.


2. Clearing Gatekeeper
-----------------------

Because the app is not notarized, macOS quarantines it on download. The
simplest fix, and the one to prefer:

  Right-click (or Control-click) Mechvibes in Applications, choose "Open",
  then confirm the warning dialog.

That whitelists this one app; afterwards it opens normally. A plain
double-click on first launch will just be refused.

If that is not offered, or macOS reports the app "is damaged and can't be
opened", clear the quarantine flag by hand - it is the same problem, not
actual corruption:

  xattr -cr /Applications/Mechvibes.app


3. Accessibility and Input Monitoring permissions
-------------------------------------------------

Global key capture requires BOTH:

  System Settings -> Privacy & Security -> Accessibility
  System Settings -> Privacy & Security -> Input Monitoring

Add /Applications/Mechvibes.app to each list and enable the toggle.

  - Accessibility covers key capture while the Mechvibes window is focused.
  - Input Monitoring covers key capture while the window is minimized or
    another app is focused. Without it, sounds only play while the
    Mechvibes window has focus.

Click the Mechvibes icon in the menu bar to see where you stand. Each
permission is listed there: "Granted" in green when it is on, or a red
"Grant ..." line when it is missing. Clicking that line opens System
Settings at the right page. Granting Accessibility takes effect straight
away. After granting Input Monitoring, quit and relaunch Mechvibes: macOS
only lets an app capture keys globally if it had the permission when it
started, so until you relaunch the menu can say "Granted" while sounds still
stop when another app is in front.

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
