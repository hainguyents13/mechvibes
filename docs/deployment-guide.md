# Deployment Guide

How to ship a new release of Mechvibes, end to end.

Windows is the primary target: it is the only platform with an installer and the only one the in-app auto-updater serves. Linux and macOS binaries ride along on the same tag.

## Prerequisites (one-time, per machine)

- Rust toolchain (`rustup`) and Cargo on `PATH`.
- [Inno Setup 6](https://jrsoftware.org/isinfo.php) installed, for local installer builds. CI installs it automatically via `choco install innosetup`.
- Push access to `main` and permission to create tags on the repo.
- Activate the repo's local quality gates (versioned git hooks):

  ```powershell
  git config core.hooksPath .githooks
  ```

## Local gates (every commit and push)

The `.githooks/` hooks enforce that nothing broken leaves your machine:

- **pre-commit** runs `cargo check` — the tree must compile before any commit.
- **pre-push** runs `cargo test` — the full suite must pass before anything reaches origin (mirrors the CI smoke gate, so a red push never wastes a CI run or lands on another session's clone).

`--no-verify` bypasses them in an emergency, but CI runs the same gates on every pull request and every push to `main`, so a bypassed failure only postpones the red light.

## Release steps

1. **Bump the version**

   ```powershell
   .\scripts\bump-version.ps1 -Version 0.6.0
   ```

   This updates `Cargo.toml`, regenerates `Cargo.lock` (via `cargo check`), and either dates the existing `## [Unreleased]` CHANGELOG section or inserts a new empty `## [0.6.0] - <date>` section. It does **not** commit anything.

2. **Fill in the CHANGELOG**

   Edit `CHANGELOG.md` and replace the empty `### Added` / `### Changed` / `### Fixed` headings under the new version section with real, user-facing entries. This becomes the GitHub release's notes verbatim (via `extract-changelog.ps1`), so write it for end users, not for other developers.

   **Start every bullet with a short bold lead that states the outcome, not the defect.** The Discord announcement keeps only that bold lead, one line per change, so each lead must read as good news on its own: "**Windows settings now save on every install.**" works standalone; "**Soundpack version checks were broken for every pack.**" reads as bad news with no resolution. Phrase leads as what is better now; put the defect story in the sentences after the bold. No em dashes anywhere in the file.

3. **Commit**

   ```powershell
   git add -A
   git commit -m "chore: bump version to 0.6.0"
   ```

4. **Tag and push**

   ```powershell
   git tag v0.6.0
   git push origin main --tags
   ```

   Pushing the commit to `main` starts `.github/workflows/ci.yml`, the workflow that builds every pull request and push. **The tag does not start a run of its own.** When that run's builds finish, its `Find release tag` job asks the remote whether a `v*` tag points at the commit that was built. If one does, the same run drafts the release from the packages it just built.

   **This gives you one run per commit, not two.** A tag-triggered release workflow would start a second run on the same commit (one for the branch push, one for the tag), which means the commit is built twice and the results are split: the packages in the branch's CI run, the release notes and the draft in the tag's run. Here the branch run does all of it, so a release has a single place to look. That one run's Actions page holds:
   - the three packages (`windows-release`, `linux-release`, `macos-release`) as its artifacts,
   - the test and build results for every OS,
   - and the draft release, built from exactly those packages.

   - **Push the tag with the commit, or while the builds are running.** Both work, because the remote is only asked after the builds finish. `git push origin main --tags` is fine.
   - **For a squash or rebase merge, tag the new commit on `main`**, not the PR's branch commit. The tag has to point at the commit that was built.
   - **Tagged after the run finished?** Open the run in the Actions tab and re-run the `Find release tag` job (and the `Draft release` job follows it). The packages are kept with the run (90 days by default), so nothing is rebuilt. For a commit with no run at all, start one with `gh workflow run ci.yml --ref <branch-or-tag>`.

5. **Wait for GitHub Actions**

   **One workflow, one run per commit.** `.github/workflows/ci.yml` runs on:

   - **a pull request into `main`** (from this repo or a fork): verification only, nothing is uploaded.
   - **a push to `main`, or manual dispatch**: builds and keeps `windows-release`, `linux-release` and `macos-release` as artifacts of the run. If a release tag points at the commit, the run also drafts the release. A branch with no pull request is not built; run `gh workflow run ci.yml --ref <branch>` to build one.

   A pull request cancels its own superseded runs; every other run has a group of its own and is never cancelled, so a later push cannot kill a release halfway through, and no commit is skipped because a newer one arrived while it was queued.

   **Only a tagged commit creates a release**, and it refuses a tag that already has a release, draft or published, so it can never add to or replace an existing one. Stages (~15 minutes end to end):

   - **`build`** — one matrix leg per OS (Windows, Linux, macOS), running in parallel, on every run. Every leg does the same thing: install what a script cannot (the cargo cache, the OS's system libraries, packaging tools), run `scripts/ci-check.sh --release`, then run that OS's packaging script, then upload `dist/` (except for pull requests).
     - **Windows** — `scripts/build-windows-installer.ps1` builds the installer with Inno Setup (installed by the leg).
     - **Linux** — `scripts/build-linux-appimage.sh` builds the `.deb` (via `cargo-deb`, driven by `[package.metadata.deb]` in `Cargo.toml`, installed by the leg) and the AppImage from one binary. `cargo deb --no-build` leaves `target/release/mechvibes` unstripped and byte-identical, so the same BuildID lands inside the AppImage.
     - **macOS** — `scripts/build-macos-app.sh` hand-assembles `Mechvibes.app` and packages it as a DMG.
   - **`tag`** (`Find release tag`; not on pull requests) — after every leg passed, runs `resolve_release_tag` from `scripts/lib/common.sh` against the remote. It finds `v<Cargo.toml version>` pointing at the built commit (annotated and lightweight tags both work), finds no tag (an ordinary build, the job just says so), or fails if the commit has a `v*` tag that does not match `Cargo.toml`'s version. You can run the same function locally.
   - **`release`** (`Draft release`; only when `tag` found one) — extracts the CHANGELOG section as the release notes (`scripts/extract-changelog.ps1`, so a build that is not a release never needs one), downloads this run's packages, generates `SHA256SUMS.txt`, checks once more that no release exists for the tag, and creates one **draft** release with everything attached.

   These are the same scripts you run locally, so a local run is a faithful rehearsal of CI. Each one:
   - checks that the active Rust is at least the `rust-version` in `Cargo.toml` (the crate's minimum supported Rust) and fails if not (`scripts/lib/common.sh`, with a PowerShell twin in the Windows script). It is a floor, not an exact match: any newer compiler passes;
   - builds with `--locked` unless given `--skip-build` (`-SkipBuild` on Windows), in which case it runs `cargo check --locked` instead and refuses a binary older than `src/`, `assets/`, `patches/`, `Cargo.toml`, `Cargo.lock` or `build.rs`;
   - checks what it produced: the AppImage and DMG are unpacked or mounted and their soundpack, font and binary contents compared with the source tree, and the Windows installer name is asserted against the auto-updater's filter.

   The legs run in parallel and each uploads artifacts; only the final `release` job writes to GitHub Releases. That single-writer design is deliberate — having each job call `action-gh-release` against the same tag races, and the last writer can drop the others' assets.

   `release` requires **every** leg to succeed. A red Linux or macOS leg stops the release rather than shipping it without that platform's asset.

6. **Review the draft release**

   Go to the repo's Releases page, open the draft, and check the notes and assets. The notes end with GitHub's generated "What's Changed" and "New Contributors" lists, which credit each merged PR and its author. Remove any line for a PR that already shipped in the previous release.

   | Asset | Platform | Notes |
   |---|---|---|
   | `Mechvibes-<version>-Setup-x64.exe` | Windows | The installer. **The only asset the auto-updater consumes** — its name must keep containing `x64` and ending in `.exe`. |
   | `mechvibes_<version>_amd64.deb` | Debian/Ubuntu | `sudo dpkg -i`. Does **not** add the user to the `input` group (see below). |
   | `mechvibes-<version>-x86_64.AppImage` | Any Linux distro | Portable, no install. Needs `chmod +x` first, and has the **same** `input` group requirement as the `.deb`. |
   | `mechvibes-<version>-macos-<arch>.dmg` | macOS | **Ad-hoc signed, not notarized.** Contains `Mechvibes.app` and an `/Applications` symlink. |
   | `README-macos-<version>.txt` | macOS | DMG install, Gatekeeper and Accessibility instructions. |
   | `SHA256SUMS.txt` | all | Digests of every other asset. **The in-app auto-updater refuses to run an installer that is not listed here with a matching hash**, so a release missing this file silently degrades every Windows user to a manual download. |

   Download and install the Windows installer on a real machine if this is a release you're not fully confident in.

   **`SHA256SUMS.txt` is what makes silent auto-update possible.** The `release` job generates it with `sha256sum *` over the collected assets and then asserts the Windows installer is covered, so it cannot go out empty or stale. The app downloads it from the same release, finds the line for the installer, and verifies the file before executing anything; a mismatch deletes the download. Note the honest limit of this: the hash and the binary come from the same GitHub release, so it protects against corrupted transfers and tampering in flight, **not** against a compromised repository. Code signing remains the open item for that.

   Releases published before this step existed (v0.6.1 and earlier) have no `SHA256SUMS.txt`. That is handled, not broken: the app reports "checksums unavailable" and falls back to the browser download button, which is the pre-Phase-6 behavior.

   **Both Linux artifacts store settings in `~/.local/share/mechvibes/data`.** Neither writes beside its binary: the AppImage mount is read-only and temporary, and the `.deb`'s app root is `/usr/bin` (root-owned), which used to make `{app_root}/data` resolve to `/usr/bin/data` and silently discard every setting because `AppConfig::save()` fails soft. Fixed 260804. A config left at the old location by anyone who ran the app as root is copied across once on first launch, never overwriting a newer one; the original is left in place.

   **The `.deb` ships `assets/` to `/usr/lib/mechvibes/assets`**, which is not a free choice: `dioxus-asset-resolver` computes `current_exe()/../../lib` (so `/usr/lib` for `/usr/bin/mechvibes`) and scans it for the first subdirectory containing an `assets/` folder. Before 260804 the package shipped no `assets/` at all, so every `asset!()` font fell back to a system font. The AppImage uses the same relative location.

   **Neither Linux artifact configures input permissions.** `maintainer-scripts` was deliberately removed from `[package.metadata.deb]`, so installing the package never modifies the user's groups, and an AppImage has no install step that could do so even in principle. Linux users must run `sudo usermod -a -G input $USER` and start a new session themselves, or the app runs silently. The generated release notes say this for both assets; keep that wording if you touch them. (The portable tarball was dropped by user decision 260803; the AppImage now covers the non-Debian users that decision left building from source.)

   **The AppImage's AppDir mirrors the `.deb` layout** (`usr/bin`, `usr/share/mechvibes/soundpacks`) so a single binary serves both packages. Two placements are load-bearing, not cosmetic:

   - `usr/lib/mechvibes/assets` — `dioxus-asset-resolver`'s Linux branch scans `<exe>/../../lib/<dir>/` for a directory containing `assets/`. Put the fonts anywhere else and every `asset!()` font silently 404s.
   - The `.desktop` file and icon are duplicated at the **AppDir root**, which `appimagetool` requires; the copies under `usr/share` are what a desktop-integration helper installs later.

   `src/state/paths.rs` resolves soundpacks **relative to the mount point** when running from an AppImage, and that ordering matters: the absolute `/usr/share/mechvibes/soundpacks` fallback is a real directory on a machine that also has the `.deb` installed, so checking it first would make a running AppImage silently load the other installation's soundpacks. Writable state goes to `~/.local/share/mechvibes/data` for the same reason it does on macOS — the image is mounted read-only, and its mount point is a fresh temporary directory on every launch.

   The job asserts bundled soundpack audio, `config.json` and font counts **match the source tree** (same rationale as macOS: the mouse packs are `.mp3`, so an `.ogg`-only check would pass while dropping all four), unpacks the finished image with `--appimage-extract` and checks the ELF magic byte-wise. `appimagetool` runs with `APPIMAGE_EXTRACT_AND_RUN=1` because GitHub runners have no FUSE, so it cannot mount itself.

   **The macOS asset is not notarized.** It is ad-hoc signed only, so Gatekeeper still blocks a plain double-click (right-click → Open clears it, once). The release notes say so. It is no longer labelled experimental, in the filename or the notes.

   **The macOS asset is a DMG containing a hand-assembled `.app`.** `dx bundle` is not used — DioxusLabs/dioxus#5723 makes the 0.7.x resource copier fail on every directory in `Dioxus.toml`'s `resources`, and it exits non-zero having already written a `.app` with an *empty* `Contents/Resources`, so "use the .app if one exists" would ship a silent app. `scripts/build-macos-app.sh` assembles the bundle explicitly (`Info.plist`, binary, `Resources/` with soundpacks + assets + a generated `.icns`), ad-hoc signs it (`codesign --force --deep -s -`), and packages it with `hdiutil`. Revisit once the upstream fix ships (present in 0.8.0-alpha.1).

   The job asserts the bundled soundpack audio and `config.json` counts **match the source tree** rather than merely being non-zero — the mouse packs are `.mp3`, so an `.ogg`-only check would pass while silently dropping all four. `spctl` is run for information but deliberately **not** gated on: it fails without notarization, which is the expected state.

   **Two path rules the bundle depends on** (`src/state/paths.rs`): resources resolve to `Contents/Resources` when the executable sits in `<name>.app/Contents/MacOS/`, which is also where `dioxus-asset-resolver` hardcodes the asset root on macOS — hence `assets/` is copied into `Resources/` alongside `soundpacks/`, or every `asset!()` font 404s. And writable state (`config.json`, `themes.json`, the cache) moves to `~/Library/Application Support/Mechvibes/data` inside a bundle, because an app in `/Applications` is not user-writable and settings would otherwise silently never persist.

7. **Publish**

   Click "Publish release" on the draft. The in-app auto-updater (`src/utils/auto_updater.rs`) reads published (non-draft) releases from the GitHub API, so existing installs will detect the update after this step - not before.

   Publishing **automatically announces** the release to the Mechvibes Discord's release channel (verified live with v0.7.0) — a Mantine-style message (greeting + release notes + links) that pings the opt-in **Release Updates** role, not `@everyone`. Members pick that role under Channels & Roles. Nothing to run for a normal release.

   Manual runs of `announce-release.yml` are for exceptional cases only, and **publishing already posts on its own — a manual run about the same release creates a second message** (this happened on v0.7.0; one had to be deleted by hand):

   ```powershell
   gh workflow run announce-release.yml -f tag=vX.Y.Z      # re-announce an already-published release (recovery)
   gh workflow run announce-release.yml -f message="..."   # hand-written announcement, posted verbatim with a real ping
   gh workflow run announce-release.yml                    # ping-free test message
   ```

   The `message` mode adds nothing around your text and fails (rather than truncates) past Discord's 2000-character cap. A historical note so nobody re-learns this the hard way: v0.6.3 was not announced automatically simply because this workflow **did not exist yet** when that release was published — not because of any `GITHUB_TOKEN` event rule; GitHub's anti-recursion rule only suppresses events *performed by* the token, and clicking Publish is performed by you. One-time setup: create a webhook in the channel (Channel settings → Integrations → Webhooks), then `gh secret set DISCORD_RELEASE_WEBHOOK`, and `gh variable set DISCORD_RELEASE_ROLE_ID` with the role's ID (unset means the message pings nobody). No bot or bot token involved.

## Why a draft first?

Draft releases don't appear in the public `GET /releases` API response that the app's auto-updater polls, so a bad build never reaches existing users automatically. Publishing is a deliberate, separate action.

## Troubleshooting

**Workflow fails at "Gate - tag must match Cargo.toml version"**
The tag (e.g. `v0.6.0`) doesn't match the `version` in `Cargo.toml`. Either you tagged before bumping the version, or the bump didn't get committed before tagging. Delete the tag (`git tag -d v0.6.0 && git push origin :refs/tags/v0.6.0`), fix the version, re-tag.

**Workflow fails at "Gate - extract changelog notes"**
`CHANGELOG.md` has no `## [<version>]` section, or the section still contains the word "Unreleased" (a placeholder, not a real release date). Run `bump-version.ps1` again or edit the CHANGELOG heading by hand to `## [X.Y.Z] - YYYY-MM-DD`.

**`cargo test` or `cargo check` fails in the smoke test step**
The workflow does not build an installer or create a release if this fails. Fix the underlying test/build failure - do not bypass this gate.

**"No installer asset found" / asset name doesn't match the auto-updater filter**
The auto-updater only recognizes Windows assets whose filename contains `x64` and ends in `.exe`. If you renamed the `.iss` script's `OutputBaseFilename`, keep that constraint (see `src/utils/auto_updater.rs`'s `find_download_url`).

Note the same filter constrains the *other* platforms' asset names in reverse: no Linux or macOS asset may contain `x64` and end in `.exe`, or it could be served to Windows users as an update. The Linux and macOS packaging scripts assert this on their output (`assert_no_updater_collision` in `scripts/lib/common.sh`), and the names use `x86_64`/`amd64`/`arm64` (none of which contain the substring `x64`) with non-`.exe` extensions. If the filter in `find_download_url` ever loosens, revisit those assertions.

**The Linux leg fails on a missing `-dev` package**
The system-dependency list lives in one place, `.github/actions/linux-system-deps/action.yml`, which the `build` job in `ci.yml` uses. Add a new dependency there, and to the Linux list in `README.md`.

**The macOS leg is red**
It blocks the release like every other leg. It is built on every pull request and push to `main` by the `macOS` entry in `ci.yml`'s matrix, so a break should show up there long before a tag.

**I pushed a tag but no release appeared**
The tag only matters when the run's `Find release tag` job looks at the remote, after the builds. Check that job's log:
- *"No v* tag points at <sha> on the remote"* — the tag was pushed after the job ran, or it points at a different commit than the one that was built (a squash merge, a branch commit). Push the tag to the built commit, then re-run `Find release tag` from the run's page. Nothing is rebuilt.
- *"Commit ... has the tag(s) vX but Cargo.toml says version Y"* — the tag does not match `Cargo.toml`. Delete the tag, fix whichever is wrong, and tag again.

**Workflow fails at "Check that no release exists for this tag"**
A release (draft or published) for that tag already exists. The workflow stops rather than add assets to it or replace them. Look at the release it links to; if you really mean to redo it, delete that release in GitHub and re-run the `Draft release` job. (The check runs right before the release is created, so two runs for one tag cannot both publish.)

**A leg fails with "Rust ... is active but Cargo.toml requires rust-version ... or newer"**
The Rust in use is older than the `rust-version` in `Cargo.toml` (or there is no `rustc` at all). Locally, run `rustup update stable`, or look for an exported `RUSTUP_TOOLCHAIN` pointing at an old toolchain. In CI the job uses the runner's preinstalled Rust, so this means the runner image is older than the floor. A Rust *newer* than the floor is not an error. To raise the minimum, change `rust-version` in `Cargo.toml`.

**Local installer build fails with "Inno Setup not found"**
Install Inno Setup 6 from https://jrsoftware.org/isinfo.php, or if already installed via winget, confirm it's at one of the paths `scripts/build-windows-installer.ps1` checks (`Program Files\Inno Setup 6`, or `%LOCALAPPDATA%\Programs\Inno Setup 6` for a per-user winget install).

## Where writable state lives

One rule across all three platforms: **writable state never goes next to the executable unless that location is genuinely the user's own.** Getting this wrong does not raise an error, because `AppConfig::save()` fails soft — it produces an app that silently discards every setting and resets on each launch. All three platforms shipped that bug at some point.

| Build | Writable data (`config.json`, `themes.json`, `manifest.json`, `music.json`, cache) | Decided by |
|---|---|---|
| Windows installed (admin **or** per-user) | `%APPDATA%\Mechvibes\data` | `unins000.exe` beside the exe |
| Windows portable / `cargo run` | `{app_root}\data` | no uninstaller present |
| macOS `.app` | `~/Library/Application Support/Mechvibes/data` | `<name>.app/Contents/MacOS/` layout |
| Linux `.deb` | `~/.local/share/mechvibes/data` | exe in `/usr/bin` or `/usr/local/bin` |
| Linux AppImage | `~/.local/share/mechvibes/data` | `<mount>/usr/bin/` layout |
| Linux portable / dev | `{app_root}/data` | none of the above |

**Why Windows keys off the uninstaller rather than "is the directory writable".** A per-user install (`%LOCALAPPDATA%\Programs\MechvibesDX`) *is* writable, so a write-probe would send admin installs to `%APPDATA%` and per-user installs to the app folder — two installs of the same version disagreeing about where settings live. Worse, a probe result can change between launches (an admin repair, an antivirus lock, a policy change), silently relocating a user's config. Inno writes `unins000.exe` into the install directory and a portable copy never has one, so the answer is stable for the life of the install. `installer/windows/mechvibes-setup.iss` uses `DefaultDirName={autopf}` with `PrivilegesRequired=lowest`, which is exactly why both install shapes exist in the wild.

**Migration.** When the location moves, the *entire* old `data/` directory is copied once (files only, no recursion), never overwriting anything already at the destination, and the originals are left in place so a downgrade still finds them. This matters most on Windows: per-user installs have fully populated, working data directories right now, so without the copy the fix for admin installs would reset everyone else. The copy is not a curated file list on purpose — a real installed machine held `music.json` (37 KB, the largest file there), which no accessor in `paths.rs` names and any hand-maintained list would have dropped.

## Scope note

Windows is the only fully supported platform: installer, upgrade path, and in-app auto-update.

Linux gets a working `.deb` **and** an AppImage, but no automated input-group setup and no auto-update on either. The AppImage is built by `scripts/build-linux-appimage.sh`, written fresh for this pipeline; the legacy `scripts/build-linux-installer.sh` referenced by older docs no longer exists and was never verified.

## How in-app update install works (Windows)

Windows is the only platform with an unattended upgrade path. **Nothing is ever downloaded automatically** — a version check only notifies, and the ~50 MB transfer starts solely when the user clicks "Download & install" in Settings. There is no auto-download setting, because downloading is already an explicit act.

1. A check (at startup, or on the 24h tick) finds a newer non-prerelease tag and shows a badge in the title bar. The badge routes to Settings; it does **not** open the raw `.exe` in a browser, which would bypass verification.
2. The user clicks **"Download & install vX.Y.Z"**. The button then carries the whole state machine: `Downloading new version...` → `Restart to finish update` / `Later` → `Installing...`.
3. The app downloads `SHA256SUMS.txt` **first**. No checksums, no download — this costs one small request instead of ~50 MB that would have to be discarded.
4. The installer streams to `%TEMP%\mechvibes-updates\<name>.exe.partial`, is verified against its digest, and only then renamed to its final name. An interrupted download can never be mistaken for a finished one.
5. **Restart to finish update** re-verifies once more, clears the staged entry from config, spawns the installer detached with `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART`, and closes the app. **Later** keeps the verified file, so returning to the button — or restarting the app — offers the install again without downloading a second time (the hash is re-checked then, so a file altered in between is discarded rather than run).

Two details in `installer/windows/mechvibes-setup.iss` that the update flow depends on — do not remove them without reading this:

- `CloseApplications=yes` lets Setup close the running app that holds `mechvibes.exe`. The `[InstallDelete]` entry for `mechvibes-dx.exe` matters too: Restart Manager covers files that Setup deletes, so an upgrade from MechvibesDX (0.8.x) also closes the old exe, which `[Files]` no longer names.

**The rename from MechvibesDX (3.0.0).** The app, its exe and its packages became `Mechvibes` / `mechvibes`, and the version jumped from 0.8.x to 3.0.0 to continue after the original Electron Mechvibes, whose last release was 2.3.4. Never release below 3.0.0 again: the Electron app's own update check reads this repository's latest release, and the Linux package would read as a downgrade. These keep upgrades working:

- **Windows**: the installer keeps the same `AppId`, so it upgrades in place, and new installs still go to `{autopf}\MechvibesDX`, because the original Electron Mechvibes installs per-user into `%LOCALAPPDATA%\Programs\Mechvibes`. `[InstallDelete]` removes the old exe, Start Menu folder, desktop shortcut and WebView2 cache. The app moves a start-at-login entry still named `MechvibesDX` to the new name on startup (`auto_startup::migrate_legacy_run_entry`). An interactive install that finds the Electron app (an uninstall entry with `DisplayVersion` 2.x and an `Uninstall Mechvibes.exe` uninstaller) offers to run its silent uninstaller first, since both apps running means every key plays twice; auto-updates never ask.
- **Linux**: the `.deb` declares `Conflicts`/`Replaces: mechvibes-dx`, so dpkg removes the old package. The Electron app's package was also called `mechvibes`, at 2.3.4, so 3.0.0 installs as an upgrade of it.
- **macOS**: the bundle identifier stays `com.hainguyents13.mechvibesdx`, so permissions and the login item stay filed under the same app. `Mechvibes.app` replaces the Electron app of the same name when dragged to Applications. An existing `MechvibesDX.app` stays behind, so from 3.0.1 the app offers once, when it runs from an Applications folder, to move it to the Trash (`legacy_migration::offer_to_trash_mechvibesdx_app`).
- **Packs from the Electron app**: from 3.0.1 the first start copies the packs in the Electron app's `custom` folder (`Mechvibes/custom` under `%APPDATA%`, `~/Library/Application Support` or `~/.config`) into the keyboard packs, through the same checks as an import (`legacy_migration::import_mechvibes2_packs`). A marker file in the data folder makes it run once.
- The second `[Run]` entry (`skipifnotsilent`, no `Check:`) is the **only** thing that relaunches the app after a silent install. Do not add a `Check: not RmSessionStarted` guard to it — that was tried and it broke auto-update entirely (app closed, never returned). `CloseApplications=yes` uses Restart Manager to close us, so a session always exists and that Check is always false; meanwhile RM itself only revives processes that called `RegisterApplicationRestart`, which this app never does. A real `/LOG` trace shows RM logging "Attempting to restart applications" and nothing coming back. Double-launch is not a concern: the app's single-instance mutex makes a second copy exit immediately.

Everything in this flow fails soft. A network error, a missing `SHA256SUMS.txt`, a hash mismatch, or a blocked installer all leave the app running; the button shows a short reason and an "Open download page" link, which is the pre-Phase-6 behavior.

**Library links (`mechvibes://install/<id>` and `mechvibes://theme/<id>`).** The "Install in Mechvibes" buttons of the sound pack and theme libraries open this scheme. Who registers it:
- **Windows**: the installer, in `[Registry]` under `HKCU\Software\Classes\mechvibes`. It has to be HKCU because Mechvibes 2 registered the same scheme there, and HKCU wins over HKLM. A silent auto-update writes it too, so upgraded machines get it without a fresh install.
- **Linux**: the `.deb` through `MimeType=x-scheme-handler/mechvibes;` and `Exec=mechvibes %u` in `mechvibes.desktop`. An AppImage writes a hidden `mechvibes-url-handler.desktop` for the user on start (`protocol::register_protocol`), pointing at `$APPIMAGE`.
- **macOS**: not registered yet.

A link that arrives while the app runs starts a second copy. That copy writes the link to `pending-deep-link` in the data folder, wakes the running copy through the single-instance channel (a named event on Windows, a Unix socket on Linux), and exits. The app only accepts `mechvibes://install/mv-<32 hex>` and `mechvibes://theme/th-<32 hex>` (a query or fragment, such as `utm_` tags, is dropped unread), looks the pack or theme up at the compiled-in `LIBRARY_BASE_URL`, and installs after the user confirms, with the file checked against the library's checksum (`utils/deep_link.rs`, `utils/library_client.rs`). A theme then goes through the same checks as a file picked with Import. Keep the website's install link for themes hidden until a release with theme links is out.

macOS is experimental: ad-hoc signed but not notarized, and never run on real hardware by the maintainer. It ships as a `.dmg` so users can drag-install and report back.
