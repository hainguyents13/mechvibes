# Format 1 files from Mechvibes 3.3

These `.mvtheme` files and their `.expected.json` snapshots were written by Mechvibes 3.3.0 itself (tag `v3.3.0`, commit `f7a6c3e`). The tests in `src/utils/theme_share.rs` check that the current build reads, applies and writes them again exactly as 3.3 did, so a change to the code can never quietly change format 1.

Never regenerate them with newer code: then they would only prove that the code agrees with itself. Never edit them by hand either; the hashes below make an edit show up in review.

## The files

| Fixture | How 3.3 made it | What it pins |
|---|---|---|
| `custom-full` | Share, from custom colors with comments, a logo with `logo.png` and `logo-muted.jpg`, a background with `background.jpg` | the full read path, both logo images, every logo and background field |
| `built-in-plain` | Share, from `Synthwave` with logo and background off | a part left out is switched off |
| `logo-colors-only` | Share, from `Dark` with a logo and no images | a logo without images (`null` image fields) |
| `background-color-only` | Share, from custom colors with a background color and no image | a background without an image |
| `older-empty-colors` | `theme.json` written by hand: empty and blank colors | an empty color reads as the default |
| `unknown-fields` | `theme.json` written by hand: keys 3.3 does not know at the top level, in `colors`, `logo` and `background`, plus a ZIP entry `theme.json` does not name | unknown keys and entries are ignored |
| `library-rewritten` | `theme.json` written by hand in the shape the theme library writes: compact JSON, images renamed `image-1.webp` and `image-2.webp` | a file downloaded from the library |

The custom colors use only `--color-*`, `color-scheme` and the daisyUI variables (`--radius-*`, `--size-*`, `--border`, `--depth`, `--noise`). The one exception is `--sparkle-size` in `unknown-fields`, a variable that is not a theme color. 3.3 keeps it. Later builds leave it out on purpose, since theme CSS now keeps only the variables the app knows, so the tests take that one line out of the snapshot before they compare. The other is in `applied`: 3.3 switched off a logo or background the file leaves out, while later builds leave it as the user has it, so the tests put back the `start` switch for that part.

Each `<fixture>.expected.json` holds what 3.3 made of the file:

- `read`: the name, colors, logo and background `from_bytes` returned, and the SHA-256 of each image it kept.
- `start` and `applied`: the theme, logo and background settings of a config before and after `apply_to_config` with every part. `start` has its own colors, logo and background, all switched on and mostly away from the defaults, so settings kept for a part that is switched off stay visible. Custom colors are applied as `Custom("imported-colors")`.
- `written_again`: `theme.json` of `to_bytes(from_bytes(file))`.

## How they were made

A throwaway worktree of the tag, with a temporary `#[test] #[ignore] fn write_v1_fixtures()` added to the tests of `src/utils/theme_share.rs` there. It writes the first four files with `save` (the Share button's code), the last three with the tests' `zip_with` helper, and then the snapshots from `from_bytes`, `apply_to_config` and `to_bytes`. The worktree, and the test with it, were removed afterwards.

```powershell
git worktree add "$env:TEMP\mv-v330" v3.3.0
# add write_v1_fixtures to the tests in $env:TEMP\mv-v330\src\utils\theme_share.rs
$env:MV_FIXTURE_OUT = "<an empty folder>"
cargo test --manifest-path "$env:TEMP\mv-v330\Cargo.toml" write_v1_fixtures -- --ignored
git worktree remove --force "$env:TEMP\mv-v330"
```

## SHA-256

```
ae9580339fdf95120fdb07682d1b89658891c82b27c248a9380a05237e4234eb  background-color-only.expected.json
4d8c5eb40c143c329beef7b9a724a41a0a50b30eaa13f33224d0625610089885  background-color-only.mvtheme
b7f4800091761de0d4be2f077cc324e24d3ad060c97139490c143c03b952280a  built-in-plain.expected.json
32ac5cfc967ec69387914613a3fad0d1f9d9fbf1f6b26f48e6e52d9611eeb3c1  built-in-plain.mvtheme
2be85b9fa7dc902b0d6b9450403b95fc1213bc8f29e50c7babc07a2ecb539c9f  custom-full.expected.json
7bff86384bc1c755da4a153fbaa33eacf0e9b47c037ec9ea56ca40a88b68d7bd  custom-full.mvtheme
ce06edab879742385c14f945e604ff71db0194c256a89b9a2762fc44c40d0367  library-rewritten.expected.json
4c8e0fc7d38aeed37c0613b2a3ade326d1bdb0439df29e738c518530a46f8ce8  library-rewritten.mvtheme
0a2dc2eda7957395ec63c0ccd4e85512dd4ff482b9965ea939065e23133f5149  logo-colors-only.expected.json
b7df21da9a6bdd935a75a71d78adbb6c5ee91f454fe92ec85053370835463d32  logo-colors-only.mvtheme
051ce9c4e012b7225c330f14d7401a57c4369c220ece27b1560f9afb12705ba9  older-empty-colors.expected.json
1e57130bbad5d4a2dca52dee086ac140e24d959d89e63742fd89583afe01115e  older-empty-colors.mvtheme
78b30e87ff3c8587556ce573fa7513dfb81414ecb2bf4329c4959d27d961cbf2  unknown-fields.expected.json
4983f21e3f10842134f8abcae16e53b135546b06efc73c8db646c5cddc51bb7c  unknown-fields.mvtheme
```
