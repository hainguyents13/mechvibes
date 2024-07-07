/**
 * Handles the loading of soundpack files automatically, regardless of format.
 * 
 * This class will load requested files into memory and return an object which can
 * be passed directly to howlerjs for playback.
 * 
 * Supported formats are:
 * - folders
 * - .zip files
 */

const fs = require('fs');
const path = require('path');
const mime = require('mime-types');
const Zip = require('adm-zip');

// HACK: howlerjs is kinda dumb, so we need to modify mime types for some files
mime.types['mp4'] = "audio/mp4";
mime.types['wav'] = "audio/wav";

function IsArchivePath(folder){
	return path.extname(folder) == '.zip';
}

function GetFilesFromArchive(folder){
	const zip = new Zip(folder);
	const zipFiles = zip.getEntries();
	let files = {};
	zipFiles.map((file) => {
		if(file.isDirectory){
			return;
		}
		const fileName = path.basename(file.entryName).toLowerCase();
		if(fileName == 'config.json'){
			files[fileName] = file.getData().toString('utf8');
		}else{
			const mimeType = mime.lookup(fileName);
			files[fileName] = `data:${mimeType};base64,${file.getData().toString('base64')}`;
		}
	});
	return files;
}

function GetFileFromArchive(folder, search){
	const zip = new Zip(folder);
	const zipFiles = zip.getEntries();
	let return_file = null;
	zipFiles.map((file) => {
		if(return_file !== null || file.isDirectory){
			return;
		}
		const fileName = path.basename(file.entryName).toLowerCase();
		if(fileName == search){
			if(fileName == 'config.json'){
				return_file = file.getData().toString('utf8');
			}else{
				const mimeType = mime.lookup(fileName);
				return_file = `data:${mimeType};base64,${file.getData().toString('base64')}`;
			}
		}
	});
	return return_file;
}

function GetFileFromFolder(folder, file){
	const filePath = path.join(folder, file);
	if(!fs.existsSync(filePath)){
		return null;
	}
	const mimeType = mime.lookup(filePath);
	return `data:${mimeType};base64,${fs.readFileSync(filePath, 'base64')}`;
}

function GetSoundpackFile(abs_path, sound){
	if(IsArchivePath(abs_path)){
		return GetFileFromArchive(abs_path, sound);
	}else{
		return GetFileFromFolder(abs_path, sound);
	}
}

module.exports = {
	GetSoundpackFile,
	GetFilesFromArchive,
	GetFileFromArchive,
	GetFileFromFolder
};