# Theme file format (`.mvtheme`)

A `.mvtheme` file holds a whole look: the theme colors, the logo style and the background, with their images. The app writes it with **Share** and reads it with **Import**, both in the current theme row at the top of **Customize**. Source: `src/utils/theme_share.rs`.

Share opens the system save dialog straight away, and the theme in the file takes the name the user gives the file. Once saved, a toast offers **Show file**.

Format 1 is frozen. Its exact layout lives in `src/utils/theme_share/wire_v1.rs`, apart from the app's own settings, so changing a setting can never change the file by accident: a new field in the app's logo or background settings does not compile until someone decides whether it goes into the file. Files written by Mechvibes 3.3.0 sit in `src/utils/theme_share/fixtures/v1/`, and tests check that every build reads, applies and writes them again as 3.3 did. A change of shape is a new format number.

## Layout

A ZIP with `theme.json` at the root and the images it names, side by side:

```
night-drive.mvtheme
├── theme.json
├── logo.png          (optional)
├── logo-muted.png    (optional)
└── background.webp   (optional)
```

Image names are whatever `theme.json` says; the app writes `logo`, `logo-muted` and `background` with the extension that matches the image.

## `theme.json`

```json
{
  "format": 1,
  "name": "Night drive",
  "colors": { "custom": { "css": "--color-primary: oklch(55% 0.3 240);\n..." } },
  "logo": {
    "border_color": "#ff4fa3",
    "text_color": "var(--color-base-content)",
    "shadow_color": "var(--color-base-content)",
    "background_color": "var(--color-base-200)",
    "background_image": "logo.png",
    "use_background_image": true,
    "muted_background": "var(--color-base-300)",
    "muted_background_image": null,
    "use_muted_background_image": false,
    "dimmed_when_muted": false
  },
  "background": {
    "background_color": "var(--color-base-100)",
    "background_image": "background.webp",
    "use_image": true
  }
}
```

| Field | Meaning |
|---|---|
| `format` | Layout version, currently `1`. The app refuses a higher number and asks the user to update. |
| `name` | Shown in the import dialog and used as the custom theme's name. Trimmed, at most 64 characters. Share writes the file name the user picked. |
| `colors` | Either `{ "built_in": "<Name>" }` with a built-in theme name as the app lists it (`Dark`, `Synthwave`, ...), or `{ "custom": { "css": "..." } }` with the declarations of a custom theme. |
| `logo` | Same fields as the app's logo settings. Image fields name a file in the ZIP. Left out when the sharer has logo customization switched off. |
| `background` | Same fields as the app's background settings. Left out when switched off. |

## What the app accepts

Everything in a file is untrusted. On import the app:

- Keeps from `colors.custom.css` only the declarations of the variables in the table below, each with a value that fits, and drops everything else: other properties and variables, comments (each reads as a space, as in CSS), rules and at-rules. A dropped line never refuses the whole file, but a custom theme with nothing left is refused. The Colors editor runs the same filter when it saves.
- Accepts a color only if it has no `; { } [ ] < > " ' \ @`, backtick or comment, its parentheses are balanced, and it calls no function other than `var`, `calc`, `min`, `max`, `clamp`, `rgb(a)`, `hsl(a)`, `hwb`, `lab`, `lch`, `oklab`, `oklch`, `color` and `color-mix`. So `url()` and anything else that loads a resource is refused. A color must also be one color, with no comma or space outside a function's brackets, so a list such as `red, 0 0 0 100vmax black` cannot add a shadow of its own. A logo or background color that breaks these rules refuses the file. An empty color means the default.
- Accepts an image only if it is a PNG, JPEG, GIF, WebP or BMP by its first bytes (no SVG), at most 10 MB, and named by a plain file name inside the ZIP.
- Refuses a file over 30 MB or a `theme.json` over 256 KB. A folder or a pipe is refused before it is opened, and a file is opened once and read only up to that limit, so a file that keeps growing is refused rather than read without end.
- Saves images in the app's images folder under a name taken from their content, `img-<first 32 hex characters of the SHA-256>.<extension of the image type>`, so no part of a name comes from the file, an import never replaces another image, and the same image imported twice is one file. If the import fails after that, only the images this import created are removed again, never one that was already there.
- Reuses a custom theme with the same name and CSS instead of adding a copy; a different theme with a name in use gets a number, as "Night drive (2)".
- Switches logo or background customization off when the file leaves that part out and the user keeps that part ticked (see below).

| Variable in `colors.custom.css` | Accepted value |
|---|---|
| `--color-<name>` (letters, digits and `-`) | A color as above that draws something: not `transparent`, not a hex color with a zero alpha (`#0000`, `#ff000000`), and not a color function whose alpha is zero or less (`oklch(50% 0.1 30 / 0)`, `rgba(0, 0, 0, 0)`). The whole value is searched, so such a color cannot hide inside `var()` or `color-mix()`. |
| `color-scheme` | `light` or `dark` |
| `--radius-selector`, `--radius-field`, `--radius-box` | `0` to `2rem` (`32px`) |
| `--size-selector`, `--size-field` | `0.125rem` (`2px`) to `0.5rem` (`8px`) |
| `--border` | `0.5px` to `2px` |
| `--depth`, `--noise` | `0` or `1` |

A value with `!` is dropped. Lengths are `0`, or a plain number in `px` or `rem`. So a theme can change how the app looks, but cannot hide its text or blow up its controls.

## Putting a theme on

Whatever brings the file, one dialog at the root of the window asks first: **Use this theme?**. It lists three parts, each with a tick, read from the file itself:

- **Colors**, as `Colors: Synthwave` or `Colors: Custom colors`. Unticked, the app keeps its current colors and adds nothing to the colors list.
- **Logo** and **Background**. When the file has the part, the line says what it brings (`Logo with images`, `Logo style`, `Background image`, `Background color`), and adds `Replaces your logo` or `Replaces your background` when the user's own is switched on. When the file leaves the part out, the line says `Not in this theme. Will turn off yours.`: ticked, the user's own part is switched off with its settings kept for later; unticked, it stays as it is.

Every part starts ticked, which gives the same result as an import in 3.3. Only the images of ticked parts are saved, and **Use this theme** is disabled while no part is ticked.

Right before a theme goes on, the app remembers the colors, logo and background settings it is about to change, in memory only. **Undo**, in the dialog once the theme is on, puts them back, as long as nothing else has changed the look since. The **Default** card in **Customize > Themes** offers the same Undo in a toast. Both sit outside the area a theme colors, so they stay readable whatever a theme does.

## Opening a theme file

The same dialog opens for a file picked with **Import**, a file dropped on the window, and a file opened from the system:

- **Windows:** the installer makes Mechvibes the handler for `.mvtheme` (ProgId `Mechvibes.Theme`), so a double-click opens the file in the app.
- **Linux:** the `.deb` installs the type `application/x-mechvibes-theme` (`packaging/linux/mechvibes.xml`) and lists it in `mechvibes.desktop`. The AppImage registers the same type for the current user when it runs.
- **macOS:** the file type is not registered yet, so use **Import**.

The file reaches the app as a launch argument: a path ending in `.mvtheme` (any case), or a `file://` URL with no host or `localhost`. A URL naming another computer and an argument starting with any other scheme are ignored, so a web page cannot make the app open a file. When the app is already running, the new launch hands the path to the running window and quits. Opening a file shows the window even with **Start minimized** on.

When a file is dragged over the window, an overlay says **Drop to use this theme**. Only `.mvtheme` files count, and of several files the first `.mvtheme` one is taken. While the dialog is busy with another theme, a new file is ignored.

## Sharing images from the web

An image the sharer set by web address is downloaded when sharing and stored in the file, so importing never contacts that address.

## Image sizes when sharing

Share cuts images down to what the app can show before writing the file (`shrink_images` in `src/utils/theme_share.rs`):

- The wallpaper is cropped around its center to the widest shape of the app window, 470 x 600 pt, which the app fills with `center / cover`, and sized to at most 1692 x 2160 px (2x a 1080 pt tall window). A taller window only trims more off the sides. A smaller wallpaper is never enlarged.
- Logo images are shrunk to fit 960 x 960 px and never cropped.
- A photo stored sideways with an EXIF orientation is turned upright first, since the copy carries no EXIF.
- The result is JPEG, or PNG when the image has transparency or is a PNG logo, and the file name's extension follows.
- An animation (GIF, animated PNG or animated WebP), an image over 40 megapixels or one the app cannot decode, and an image that comes out no smaller, are kept as they are.

## Files from the theme library

The theme library at [mechvibes.com/themes](https://mechvibes.com/themes) checks an upload with the rules above, then writes the file again so it holds no more than the app can show. A file downloaded from the library is still a format 1 `.mvtheme`, but it is not byte for byte the file that was uploaded:

- The wallpaper is cropped around its center to the widest shape of the app window, 470 x 600 pt, which the app fills with `center / cover`, and sized to at most 1285 x 1640 px (2x the default 820 pt height). A smaller wallpaper is cropped to that shape but never enlarged.
- Logo images are shrunk to fit 960 x 960 px and never cropped, since the logo box follows the font.
- Each image becomes WebP. Animated GIFs stay animated (animated WebP).
- An image is kept unchanged when it is already smaller as it is, or when the website cannot read it for resizing (BMP, or more than 40 megapixels).
- Images are renamed `image-1`, `image-2` and so on with the extension of their type, `theme.json` names them accordingly, and every entry `theme.json` does not name is left out. All other fields of `theme.json` are kept as they were.

A 1 MB upload with a 4K wallpaper comes out at about 22 KB. The rewritten file passes the same checks again before the library publishes it.

### Install links

The library links to `mechvibes://theme/<id>`, which the app handles from 3.3.0 on Windows and Linux. A theme id is `th-` and 32 lowercase hex characters (`^th-[0-9a-f]{32}$`); ids never change and never collide with sound pack ids (`mv-`). For such a link the app:

1. Reads the id from the path and ignores the link unless it matches the pattern.
2. Looks the theme up with `GET https://mechvibes.com/api/themes/<id>` (`{ "theme": { "id", "name", "description", "author", "colorsLabel", "hasLogo", "hasBackground", "size", "checksum", "previewUrl", ... } }`, `404` when the theme is not published).
3. Downloads `GET https://mechvibes.com/api/themes/<id>/download`, compares its SHA-256 with `checksum`, and reads it with every check above, as a file picked with **Import**.
4. Shows the confirm dialog with the name and author from the library and the checklist built from the downloaded file, not from `hasLogo` or `hasBackground`, so what the dialog lists is what goes on. **Use this theme** puts on that same file; nothing is downloaded again.

The dialog shows each step (looking up, downloading) with a **Cancel** button, and a result that arrives after Cancel is dropped. Nothing is put on without the confirm dialog, and the link carries no other data.
