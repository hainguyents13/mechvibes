# Theme file format (`.mvtheme`)

A `.mvtheme` file holds a whole look: the theme colors, the logo style and the background, with their images. The app writes it with **Customize > Themes > Share** and reads it with **Import**. Source: `src/utils/theme_share.rs`.

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
| `name` | Shown in the import dialog and used as the custom theme's name. Trimmed, at most 64 characters. |
| `colors` | Either `{ "built_in": "<Name>" }` with a built-in theme name as the app lists it (`Dark`, `Synthwave`, ...), or `{ "custom": { "css": "..." } }` with the declarations of a custom theme. |
| `logo` | Same fields as the app's logo settings. Image fields name a file in the ZIP. Left out when the sharer has logo customization switched off. |
| `background` | Same fields as the app's background settings. Left out when switched off. |

## What the app accepts

Everything in a file is untrusted. On import the app:

- Keeps only CSS declarations of custom properties (`--name: value`) and `color-scheme` from `colors.custom.css`, and drops comments (each reads as a space, as in CSS), rules and at-rules. A custom theme with nothing left is refused.
- Accepts a color only if it has no `; { } [ ] < > " ' \ @`, backtick or comment, its parentheses are balanced, and it calls no function other than `var`, `calc`, `min`, `max`, `clamp`, `rgb(a)`, `hsl(a)`, `hwb`, `lab`, `lch`, `oklab`, `oklch`, `color` and `color-mix`. So `url()` and anything else that loads a resource is refused. An empty color means the default.
- Accepts an image only if it is a PNG, JPEG, GIF, WebP or BMP by its first bytes (no SVG), at most 10 MB, and named by a plain file name inside the ZIP.
- Refuses a file over 30 MB or a `theme.json` over 256 KB.
- Saves images under new names in the app's images folder, so an import never replaces an image already in use. If the import fails after that, the saved images are removed again.
- Reuses a custom theme with the same name and CSS instead of adding a copy; a different theme with a name in use gets a number, as "Night drive (2)".
- Switches logo or background customization off when the file leaves that part out.

## Sharing images from the web

An image the sharer set by web address is downloaded when sharing and stored in the file, so importing never contacts that address.

## Image sizes when sharing

Share cuts images down to what the app can show before writing the file (`shrink_images` in `src/utils/theme_share.rs`):

- The wallpaper is cropped around its center to the widest shape of the app window, 470 x 600 pt, which the app fills with `center / cover`, and sized to at most 1692 x 2160 px (2x a 1080 pt tall window). A taller window only trims more off the sides. A smaller wallpaper is never enlarged.
- Logo images are shrunk to fit 960 x 960 px and never cropped.
- The result is JPEG, or PNG when the image has transparency, and the file name's extension follows.
- A GIF (it may be animated), an image over 40 megapixels or one the app cannot decode, and an image that comes out no smaller, are kept as they are.

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
2. Shows a confirm dialog from `GET https://mechvibes.com/api/themes/<id>` (`{ "theme": { "id", "name", "description", "author", "colorsLabel", "hasLogo", "hasBackground", "size", "checksum", "previewUrl", ... } }`, `404` when the theme is not published).
3. When the user confirms, downloads `GET https://mechvibes.com/api/themes/<id>/download`, compares its SHA-256 with `checksum`, and imports it exactly as a file picked with **Import**, with every check above.

Nothing is installed without the confirm dialog, and the link carries no other data.
