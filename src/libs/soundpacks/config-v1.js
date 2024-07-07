/**
 * Soundpack configuration version 1
 * 
 */

const { Howl } = require('howler');
const { keycodesRemap } = require('../keycodes');
const { GetSoundpackFile } = require('./file-manager');

class SoundpackConfig {
	/**
	 * 
	 * @param {object} config the decoded json of the soundpack config.json
	 * @param {object} meta calculated metadata about the soundpack
	 */
	constructor(config, meta) {
		this.name = config.name ?? null;
		this.key_define_type = config.key_define_type ?? null;
		this.includes_numpad = config.includes_numpad ?? null;
		this.sound = config.sound ?? null;
		this.defines = config.defines ?? null;

		this.pack_id = meta.pack_id ?? null;
		this.group = meta.group ?? null;
		this.abs_path = meta.abs_path ?? null;
		this.is_archive = meta.is_archive ?? null;
		this.is_custom = meta.is_custom ?? null;

		// check all the keys for any null values and throw an error if there are any
		for (let key in this) {
			if (this[key] === null) {
				throw new Error(`SoundpackConfig: Missing required property: ${key}`);
			}
		}

		this.version = 1;
	}



	/**
	 * Load the sounds into memory
	 */
	LoadSounds(){
		return new Promise((resolve, reject) => {
			let beforeReject = (e) => {
				// delete this.sound_data;
				if(this.key_define_type == "single"){
					if(this.audio){
						this.audio.unload();
						delete this.audio;
					}
				}else if(this.key_define_type == "multi"){
					if(this.audio){
						Object.keys(this.audio).map((kc) => {
							this.audio[kc].unload();
						});
						delete this.audio;
					}
				}
				reject(e);
			}
			let timeout = setTimeout(() => {
				beforeReject("The soundpack took too long to load.");
			}, 3000);

			let wait = (audio) => {
				return new Promise((res, rej) => {
					if(audio.state() == "loaded"){
						res();
					}else{
						audio.once('load', () => {
							res();
						})
						audio.once('loaderror', (e) => {
							rej(e);
						})
					}
				});
			}

			if(this.key_define_type == "single"){
				// define sound path
				const sound = GetSoundpackFile(this.abs_path, this.sound);
				const sound_data = { src: [sound], sprite: keycodesRemap(this.defines) };

				const audio = new Howl(sound_data);
				wait(audio).then(() => {
					clearTimeout(timeout);
					this.audio = audio;
					resolve();
				}).catch((e) => {
					beforeReject(e);
				})
			}else if(this.key_define_type == "multi"){
				let sound_data = {};
				Object.keys(this.defines).map((kc) => {
					if (this.defines[kc]) {
						// define sound path
						const sound = GetSoundpackFile(this.abs_path, this.defines[kc]);
						sound_data[kc] = { src: [sound] };
					}
				});
				this.audio = {};
				sound_data = keycodesRemap(sound_data);
				Object.keys(sound_data).map((kc) => {
					const audio = new Howl(sound_data[kc]);
					wait(audio).then(() => {
						clearTimeout(timeout);
						this.audio[kc] = audio;
						resolve();
					}).catch((e) => {
						beforeReject(e);
					})
				})
			}else{
				beforeReject("Invalid key_define_type");
			}

		});
	}

	UnloadSounds(){

	}
}

module.exports = SoundpackConfig;


// demo config
let demo_config = {
	// A unique identifier, usually assigned by the server
	"id": "sound-pack-1200000000001",

	// The name of the soundpack
	"name": "CherryMX Black - ABS keycaps",

	// how the key definitions are defined
	"key_define_type": "single" || "multi",

	// if the soundpack includes numpad definitions
	"includes_numpad": false, // unused but required

	// the sound file to use when key_define_type is "single".
	// This property is still required when key_define_type is "multi", but is unused in that mode.
	"sound": "sound.ogg",

	// key definitions
	"defines": {
		// format
		"keyCode": "definition",
		// when key_define_type is "single"
		"1": [
			2894, // start time in milliseconds
			226 // duration in milliseconds
		],
		"2": [
			12946,
			191
		],
		// when key_define_type is "multi"
		"3": "sound.ogg",
		"4": "sound.ogg"
	},

	// though the default assumed version is 1, it's better to define it incase the default ever changes.
	"version": 1
}