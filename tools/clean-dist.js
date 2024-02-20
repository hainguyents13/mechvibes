const fs = require('fs-extra');
const path = require('path');

// Check if package.json is in the current directory or in the parent directory
const packageJsonPath = path.resolve(process.cwd(), 'package.json');
const parentPackageJsonPath = path.resolve(process.cwd(), '../package.json');

if (fs.existsSync(packageJsonPath)) {
  	console.log('package.json found in the current directory.');
	// Get the dist folder path
	const distPath = path.resolve(process.cwd(), 'dist');
	// Check if the dist folder exists
	if (fs.existsSync(distPath)) {
		// Empty the dist folder
		fs.emptyDirSync(distPath);
		console.log('Dist folder emptied successfully.');
	} else {
		console.log('Dist folder does not exist.');
	}
} else if (fs.existsSync(parentPackageJsonPath)) {
  	console.log('package.json found in the parent directory.');
	// Get the dist folder path
	const distPath = path.resolve(process.cwd(), '../dist');
	// Check if the dist folder exists
	if (fs.existsSync(distPath)) {
		// Empty the dist folder
		fs.emptyDirSync(distPath);
		console.log('Dist folder emptied successfully.');
	} else {
		console.log('Dist folder does not exist.');
	}
} else {
  	console.log('package.json not found.');
}

