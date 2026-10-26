# MechvibesDX Soundpack Config.json V2 Format

This guide describes the V2 soundpack format used by MechvibesDX. Use this to create, modify, or understand soundpack configurations.

## Overview

A soundpack is a folder containing audio files and a configuration file that defines which sounds play for each keyboard key or mouse button. MechvibesDX ships with curated bundled soundpacks (Cherry MX Black, Topre Purple, etc.) and supports importing custom packs.

### Folder Layout

```
my-soundpack/
├── config.json          (required)  Metadata and key-to-sound mappings
├── sound.ogg           (required)  Audio file (OGG, WAV, MP3, or FLAC)
├── icon.png            (optional)  Display thumbnail for the UI
└── README.md           (optional)  Author notes
```

### Where Custom Packs Live

Custom soundpacks are stored in the system application data directory:

- **Windows:** `%APPDATA%\Mechvibes\soundpacks\`
- **Linux:** `~/.local/share/mechvibes/soundpacks/`
- **macOS:** `~/Library/Application Support/Mechvibes/soundpacks/`

Within that directory, packs are organized by type:
```
soundpacks/
├── keyboard/
│   ├── my-pack-1/
│   └── my-pack-2/
└── mouse/
    └── my-mouse-pack/
```

---

## Full Field Reference

### Required Fields

These three fields are what the validator actually checks. A config missing any of them is rejected with a "Missing required V2 fields" error.

#### `name` (string)
Human-readable name displayed in the UI and settings.
- Example: `"Cherry MX Black - ABS keycaps"`

#### `author` (string)
Pack creator name. Required: validation fails without it (the legacy spelling `m_author` is also accepted).
- Example: `"Mechvibes"`, `"John Smith"`

#### `definitions` or `defs` (object, required)
A mapping of key names to their timing information. Both `"definitions"` and `"defs"` are accepted; use either.

Each key maps to an object with timing data:
```json
{
  "KeyA": { "timing": [[0, 50], [50, 100]] },
  "KeyB": { "timing": [[100, 200]] }
}
```

For the `"single"` method, `timing` is an array of `[start_ms, end_ms]` pairs within the audio file:
- Example: `[[45750.0, 45832.0], [45832.0, 45914.0]]` means two sound segments (for multiple keystroke variations)
- Milliseconds can be floats (e.g., `45750.0`)
- The app picks a random segment on each keypress if multiple are provided

### Playback Fields

Not required by the validator, but a pack does not make sound without them.

#### `audio_file` (string)
Path to the main audio file (for the `"single"` method). Can be relative to the soundpack directory.
- Example: `"sound.ogg"`, `"./audio/click.wav"`
- Supported formats: OGG, WAV, MP3, FLAC
- Needed when `definition_method` is `"single"` (the normal case)

#### `definition_method` (string)
How sounds are mapped to keys. In practice always `"single"`: all keys play segments from one main audio file, specified in `audio_file`.
- If a pack uses `"multi"` (one file per key), the app automatically converts it to `"single"` on first load

### Optional Fields

#### `id` (string)
Unique identifier for the soundpack; alphanumeric characters, hyphens, and underscores.
- Example: `"my-keyboard-pack"`, `"cherrymx-black-abs"`
- If omitted on import, the app generates one automatically (e.g., `"imported-<uuid>"`)

#### `config_version` (string or number)
Configuration format version. Should be `"2"` or `2` (both forms are accepted).
- Default: treated as `2` when omitted and the structure is valid
- A value above `2` makes the app report that the pack needs a newer MechvibesDX

#### `description` (string)
Longer description of the soundpack (UI tooltip or details).
- Example: `"Cherry MX Black switches with ABS keycaps"`

#### `version` (string)
Pack version in any format (e.g., `"1.0.0"`, `"2025-01-15"`).
- Default: `"1.0.0"`

#### `icon` (string)
Filename of an image file to display as the pack thumbnail.
- Example: `"black.jpg"`, `"icon.png"`
- The image should be placed in the soundpack directory
- Formats: JPEG, PNG recommended
- If the file does not exist, the UI shows a placeholder

#### `tags` (array of strings)
Labels for organizing and searching packs (e.g., `["mechanical", "clicky", "cherry"]`).
- Example: `["Cherry MX", "Black", "ABS"]`
- Default: empty array `[]`

#### `created_at` (string)
ISO 8601 timestamp when the pack was created. Informational only; not used for sorting.
- Example: `"2025-06-17T12:23:39.537516300+00:00"`
- Default: not set

#### `soundpack_type` (string)
Explicitly declare the pack type. Normally auto-detected; use only to override.
- Valid values: `"Keyboard"` or `"Mouse"` (with capital first letter when explicit)
- If omitted, the app detects the type based on key names in `definitions`
- Default: auto-detect

#### `options` (object)
Advanced playback options.

**`recommended_volume`** (float, 0.0-2.0)
- Volume level (1.0 is 100%). Used as a starting point in the UI.
- Example: `0.8`
- Default: `1.0`

**`random_pitch`** (boolean)
- If `true`, each keystroke plays at a slightly randomized pitch (+/- up to 10-15% variation)
- If `false`, all keystrokes play at the original pitch
- Example: `true`, `false`
- Default: `false`

Example:
```json
{
  "options": {
    "recommended_volume": 0.8,
    "random_pitch": true
  }
}
```

---

## Key Naming Reference

Use these identifiers in the `definitions` object. The app follows the W3C Web IDL `KeyboardEvent.code` standard with extensions for mouse.

### Keyboard Keys

**Alphanumeric:**
```
KeyA, KeyB, KeyC, ... KeyZ  (letters)
Digit0, Digit1, ... Digit9  (number row)
```

**Function Keys:**
```
F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12
```

**Navigation:**
```
Home, End, PageUp, PageDown, Insert, Delete, Tab
```

**Modifiers:**
```
ShiftLeft, ShiftRight
ControlLeft, ControlRight
AltLeft, AltRight
```

**Special Keys:**
```
Enter, Escape, Backspace, Space, CapsLock, NumLock
PrintScreen, ScrollLock, Pause
```

**Punctuation/Symbols:**
```
Comma, Period, Semicolon, Quote, Backquote, Backslash
BracketLeft, BracketRight, Slash, Minus, Equal
```

**Arrows:**
```
ArrowUp, ArrowDown, ArrowLeft, ArrowRight
```

**Numpad:**
```
Numpad0, Numpad1, ... Numpad9
NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide
NumpadDecimal, NumpadEnter
```

### Mouse Keys

The engine understands these exact code strings, but which of them the app can
actually capture depends on the platform:

| Code | Emitted when | Captured on |
|------|--------------|-------------|
| `MouseLeft` | Left button press/release | Everywhere |
| `MouseRight` | Right button press/release | Everywhere |
| `MouseMiddle` | Middle (wheel) button press/release | Windows, Linux |
| `MouseWheelUp` | Wheel scrolled up | Linux under Wayland |
| `MouseWheelDown` | Wheel scrolled down | Linux under Wayland |
| `Mouse4` | First extra button (usually Back) | Windows, and Linux under Wayland |
| `Mouse5` | Second extra button (usually Forward) | Windows, and Linux under Wayland |
| `Mouse6`, `Mouse7` | Further extra buttons, on mice that have them | Linux under Wayland |
| `Mouse8` | Third extra button; on X11, the Back button | Linux |

Three limits are worth knowing before you build a pack around these codes:

- **macOS reports the left and right buttons only.** The other codes never reach
  the engine there, whatever the pack defines.
- **On X11 the extra buttons do not arrive as `Mouse4` or `Mouse5`.** rdev reads
  X11 buttons 4 to 7 as wheel axes, so the side buttons reach it as unknown
  codes: Back is reported as `Mouse8` and Forward is dropped. The `Mouse4` and
  `Mouse5` pair comes from Wayland only.
- **The wheel is Linux under Wayland only.** The Windows Raw Input listener maps
  buttons only, and on X11 and macOS the rdev branch that would report the wheel
  is commented out, so none of those platforms forward it: on macOS rdev does see
  the wheel, the app just does not act on it. Touchpad scrolling is not covered
  either: two-finger scrolling is synthesized by the compositor and produces no
  `REL_WHEEL` delta, so it makes no sound.

**A mouse code a pack does not define produces no sound.** The app deliberately
does not substitute another sound: a wheel that sounds like the left click is
worse than silence. So a pack that wants wheel, wheel-click or extra-button
sounds has to define them; a pack that defines only `MouseLeft`/`MouseRight`
simply stays silent for the rest.

Define a mouse sound by adding its timing entry to `definitions`:

```json
"definitions": {
  "MouseLeft": { "timing": [[0, 100], [100, 200]] },
  "MouseRight": { "timing": [[200, 300], [300, 400]] },
  "MouseMiddle": { "timing": [[400, 500], [500, 600]] },
  "MouseWheelUp": { "timing": [[600, 700]] }
}
```

Wheel codes are impulses: a scroll has no release event, so a single
`[start, end]` pair is enough. The app rate-limits wheel sounds to one per
120 ms, so a fast scroll cannot machine-gun the sound. Up and down share that
limit.

The app auto-detects mouse packs by checking if key names start with `Mouse`,
`Button`, or `Wheel`.

---

## Templates

### Minimal Keyboard Pack

```json
{
  "config_version": "2",
  "id": "my-first-pack",
  "name": "My First Soundpack",
  "author": "Your Name",
  "definition_method": "single",
  "audio_file": "click.ogg",
  "definitions": {
    "KeyA": { "timing": [[0, 50]] },
    "KeyB": { "timing": [[50, 100]] },
    "Space": { "timing": [[100, 150]] }
  }
}
```

Save this as `config.json` in a folder with `click.ogg`, then import via the app.

### Full-Featured Keyboard Pack

```json
{
  "config_version": "2",
  "id": "cherry-mx-custom",
  "name": "Custom Cherry MX",
  "author": "Your Name",
  "description": "Custom Cherry MX switches with multiple sound variants",
  "version": "1.0.0",
  "definition_method": "single",
  "audio_file": "keyboard.ogg",
  "icon": "icon.png",
  "soundpack_type": "Keyboard",
  "created_at": "2025-08-13T00:00:00Z",
  "tags": ["Cherry MX", "Mechanical", "Custom"],
  "options": {
    "recommended_volume": 0.9,
    "random_pitch": true
  },
  "definitions": {
    "KeyA": { "timing": [[100, 150], [150, 200]] },
    "KeyB": { "timing": [[200, 250], [250, 300]] },
    "Space": { "timing": [[10000, 10100]] }
  }
}
```

### Minimal Mouse Pack

```json
{
  "config_version": "2",
  "id": "mouse-clicks",
  "name": "Simple Mouse Clicks",
  "author": "Your Name",
  "definition_method": "single",
  "audio_file": "mouse.ogg",
  "definitions": {
    "MouseLeft": { "timing": [[0, 100]] },
    "MouseRight": { "timing": [[100, 200]] }
  }
}
```

The app auto-detects this as a mouse pack because the key names start with `Mouse`.

### Explicit Mouse Pack (with Type Declaration)

```json
{
  "config_version": "2",
  "id": "mouse-sounds",
  "name": "Mouse Sounds",
  "author": "Your Name",
  "soundpack_type": "Mouse",
  "definition_method": "single",
  "audio_file": "clicks.ogg",
  "definitions": {
    "MouseLeft": { "timing": [[0, 80]] },
    "MouseRight": { "timing": [[80, 160]] }
  }
}
```

---

## Classic Mechvibes Packs (V1 Format)

### V1 to V2 Conversion

Old Mechvibes soundpacks (V1 format) are auto-converted on import or when the app loads them:

1. The app detects the V1 format based on the presence of `defines` and `sound` fields
2. A backup is created at `config.json.v1.backup` (in the soundpack directory)
3. The `config.json` is automatically converted to V2 format
4. Multi-method V1 packs are further converted to single-method for V2 storage

### What Changed

| Aspect | V1 | V2 |
|--------|----|----|
| **Version field** | `"config_version": 1` | `"config_version": "2"` |
| **Definitions** | `"defines": { "1": [0, 100], "2": [100, 200] }` | `"definitions": { "KeyA": { "timing": [[0, 100]] } }` |
| **Audio reference** | `"sound": "file.ogg"` or multiple files (multi method) | `"audio_file": "file.ogg"` (single method) |
| **Key naming** | IOHook numeric codes | W3C Web `KeyboardEvent.code` strings |

### After-Import Advice

If you reimport a V1 pack after the app already converted it:

1. The app detects it is already V2 and skips re-conversion
2. Backup is not overwritten unless conversion is needed
3. No manual action required; the pack plays the same

If some keys sounded sped up or too high-pitched in a pack you converted before, that converter bug was fixed (see issue #63). Re-import the original classic pack to get the corrected sound; the `.v1.backup` file keeps your original config either way.

---

## Troubleshooting

### Common Validation Errors

#### "Missing required fields: name"
The `name` field is not in the config. Add it:
```json
{
  "name": "My Pack",
  ...
}
```

#### "Missing required fields: author"
The `author` field is missing. Add it:
```json
{
  "author": "Your Name",
  ...
}
```

#### "Missing required fields: definitions"
The `definitions` (or `defs`) field is empty or missing. Ensure at least one key is defined:
```json
{
  "definitions": {
    "KeyA": { "timing": [[0, 50]] }
  }
}
```

#### "Invalid definitions entry for 'KeyX': expected timing array"
The timing format is wrong. Each key must map to `{ "timing": [[start, end], ...] }`:
```json
"KeyA": { "timing": [[0, 50]] }  // Correct
"KeyA": [[0, 50]]                 // Wrong: missing "timing" key
```

#### "Invalid timing array for 'KeyX[0]': expected [start, end]"
A timing pair is malformed. Each pair must be exactly two numbers:
```json
"timing": [[0, 50]]        // Correct
"timing": [[0, 50, 100]]   // Wrong: three values instead of two
```

#### "Invalid JSON format: ..."
The JSON syntax is invalid. Common issues:
- Missing commas between fields: `{ "name": "Pack" "id": "..." }` (missing comma after `"Pack"`)
- Trailing commas: `{ "tags": ["a", "b",] }` (trailing comma in array)
- Unquoted strings: `{ name: "Pack" }` (keys must be quoted)

Use a JSON validator (e.g., [jsonlint.com](https://www.jsonlint.com)) to check syntax.

#### "This soundpack requires a newer version of MechvibesDX"
The `config_version` is higher than the app supports. Update MechvibesDX or downgrade the config version to `2`.

#### "No audio files found in soundpack"
For single-method packs, at least one audio file must be present in the soundpack folder. Check:
- File exists in the pack directory
- Filename is correct in `audio_file` field
- Format is OGG, WAV, MP3, or FLAC

### Pack Won't Import

1. **Ensure `config.json` is valid JSON**
   - Check for syntax errors (unmatched braces, missing commas)
   - Use an online JSON validator

2. **Verify required fields exist**
   - `name`, `author`, `definitions` (or `defs`), `definition_method`, `audio_file`

3. **Confirm audio file exists**
   - File must be in the soundpack folder
   - Path in `audio_file` must match exactly (case-sensitive on Linux/macOS)

4. **Check for ID conflicts**
   - If an ID is already used, the app may reject the pack or auto-generate a new ID
   - Ensure the `id` field is unique

### Timing Values Are Too Large/Small

If timing values seem out of range:

- **Too large:** Check if the audio file duration is actually shorter than the timing values. Use a media player to confirm file duration in milliseconds.
- **Too small:** Timing values can be very precise (fractional milliseconds). This is normal.

The app validates timing during load and logs warnings if a key's timing exceeds the audio file duration.

### No Sound on Certain Keys

1. Check that the key name matches the W3C standard (e.g., `"KeyA"` not `"Key A"`)
2. Ensure timing values are within the audio file duration
3. Verify `audio_file` points to an existing file
4. Check the app's debug log (Settings > Debug) for load errors

---

## Best Practices

### Timing Precision

- Timing values are in milliseconds and can be floats (e.g., `45750.5`)
- Use precise timing for realistic sound playback; avoid rounding to whole numbers unnecessarily
- When creating timing data, measure the audio file's actual sample positions, not just estimates

### Multiple Variants

If you have multiple keystroke sound variations (e.g., a "press" and "release" sound in one file), list both in the timing array:
```json
"KeyA": { "timing": [[0, 50], [50, 100]] }
```
The app randomly picks one on each keystroke.

### Audio File Format

- **OGG Vorbis** recommended for best compression and compatibility
- **WAV** is lossless but larger (suitable for small packs or high-fidelity needs)
- **MP3** supported but older codec; not recommended for new packs
- **FLAC** lossless and smaller than WAV; good alternative

All formats are resampled to the system audio output rate on load for optimal quality.

### Icon Selection

- Create a square image (512x512 or similar) for best scaling
- Keep file size small (< 200 KB) for fast UI rendering
- Use PNG for transparency, JPEG for photographs

### Testing

Before sharing:
1. Import into the app via Settings > Soundpacks > Import
2. Select for keyboard or mouse sounds
3. Test multiple keys to verify timing is correct
4. Check volume level (adjust `recommended_volume` if needed)
5. Confirm no keys are missing or have wrong sounds

---

## Advanced Topics

### The "Multi" to "Single" Conversion

If you have an older V2 pack with `"definition_method": "multi"`:

```json
{
  "definition_method": "multi",
  "definitions": {
    "KeyA": { "audio_file": "a.ogg", "timing": [[0, 50]] },
    "KeyB": { "audio_file": "b.ogg", "timing": [[0, 75]] }
  }
}
```

The app automatically converts this to single-method on first load:
- All audio files are concatenated into one file
- Timing values are adjusted to account for the concatenation offsets
- The config is rewritten to single-method format
- Original files are left untouched

This is transparent; you do not need to do anything.

### Skipped Fields

The following fields are recognized but do not affect playback:
- `description` - UI only
- `tags` - UI organization only
- `created_at` - Informational only
- `license` - Informational only
- `version` - Informational only (not the config version)

### Generated Fields

The app may add or modify these on load:
- `config_version_num` - Internal field, do not set manually
- `soundpack_type` - Auto-set if not explicit; do not rely on manual entry

---

## Spec Compliance

- **Config version:** 2
- **Key naming:** W3C `KeyboardEvent.code` standard
- **Timing unit:** milliseconds (float)
- **Audio formats:** OGG (Vorbis), WAV (PCM), MP3 (MPEG), FLAC (lossless)
- **Character encoding:** UTF-8 for JSON and all strings

---

## See Also

- [MechvibesDX README](../README.md#soundpacks) - Basic soundpack import and usage
- [System Architecture](./system-architecture.md) - Audio engine and soundpack loading details
- [W3C KeyboardEvent.code Spec](https://w3c.github.io/uievents-code/) - Full key naming reference

