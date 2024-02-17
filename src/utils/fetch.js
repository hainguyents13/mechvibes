const { app } = require('electron');
const Fetch = require('electron-fetch').default;

function fetch(serverUrl, requestOptions) {
	if (!requestOptions.headers) {
		requestOptions.headers = {
			'User-Agent': `Mechvibes/${app.getVersion()} (Electron/${process.versions.electron})`
		};
	}

	if (requestOptions.body && typeof requestOptions.body !== 'string') {
		requestOptions.headers['Content-Type'] = 'application/json';
		requestOptions.body = JSON.stringify(requestOptions.body);
	}
	
	return Fetch(serverUrl, requestOptions);
}

module.exports = fetch;