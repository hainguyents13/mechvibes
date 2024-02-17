const fetch = require("./fetch");

let remoteUrl = undefined;
module.exports = {
	setRemoteUrl: (url) => {
		remoteUrl = url;
	},
	identify: (info) => {
		return new Promise((resolve, reject) => {
			if(remoteUrl === undefined) reject("Remote URL not set");
			fetch(remoteUrl, {
				method: "AUTH",
				body: {
				  type: "identify",
				  userInfo: info
				}
			}).then(r => r.json()).then((json) => {
				resolve(json);
			}).catch((e) => {				
				resolve({
					success: false,
					error: e,
					src: "ipc-identify"
				});
			});
		});
	},
	validate: (identifier, info) => {
		return new Promise((resolve, reject) => {
			if(remoteUrl === undefined) reject("Remote URL not set");
			fetch(remoteUrl, {
				method: "AUTH",
				body: {
				  type: "validate",
				  data: identifier,
				  userInfo: info
				}
			}).then(r => r.json()).then((json) => {
				resolve(json);
			}).catch((e) => {
				resolve({
					success: false,
					error: e,
					src: "ipc-validate"
				});
			});
		});
	}
}