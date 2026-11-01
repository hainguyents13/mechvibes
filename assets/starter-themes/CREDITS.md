# Starter themes: credits

The themes in this folder come with Mechvibes: Retro Beige, Midnight Blue, RGB Night and Rainy Cafe. The app builds them in and reads them with the same checks as any theme file (`src/utils/starter_themes.rs`).

## Images

None of the starter themes has an image yet. Each one brings its colors and a logo style only, made by the Mechvibes team. No image or other work by anyone else is in these files.

## Adding an image later

A wallpaper or logo image goes into a starter theme only after the owner has seen it in the app and approved it. Images are made with an AI image tool, never taken from another person or site. For each image, add a row to the table below in the same change that adds the image:

| Theme | Image in the file | Made with (tool and model) | Made on | Prompt | Approved by the owner on |
|---|---|---|---|---|---|

- **Theme** and **Image in the file**: the theme's name and the image's name inside its `.mvtheme`, for example `background.webp`.
- **Made with**: the tool and the model version, as the tool names them.
- **Made on** and **Approved by the owner on**: dates as `YYYY-MM-DD`.
- **Prompt**: the full prompt, word for word, including any negative prompt.

Keep each theme file at 150 KB or less and all of them together at 600 KB or less. A test checks this.
