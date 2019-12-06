# Mechvibes

It's an app that let you define and play mechanical key press sound as you type your current keyboard, although it is membrane or mech key with any kind of switch. Just select switch type, keycaps type and modifications such as lubed, silenced... (comming soon) put on headphone or turn on speakers to enjoy the sound!

- You can update new sound set by cloning and edit set detail in `/src/audio/`.
- Selected set and volume will be saved every time you change them for next use.
- Select set bases on keycap types, lubed or silenced switches are comming soon, I'm working on it.

This app using `gkm` package which requires **Java** to run, `iohook` doesn't require Java but I cannot use it in current Electron version.

### How to use

- Clone the code
- Install `electron-forge`
- run `npm install`
- run `npm start`
- start typing and enjoy!

### Issues

🙉 If you cannot hear any sound, please install Java.  
🤝 Any ideas are welcomed!
