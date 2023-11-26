const fs = require('fs');
const { shell, remote, ipcRenderer } = require('electron');
const BASE_URL = "https://www.mechvibes.com/sound-packs";
const CUSTOM_PACKS_DIR = remote.getGlobal('custom_dir');

const errorTranslation = {
	400: "INVREQ",
	401: "UNAUTH",
	402: "PAYMENT",
	403: "FORBID",
	404: "NOTFOUND",
	405: "BADMETH",
	418: "TEAPOT",
	429: "TOOFAST",
	451: "DMCA",
	500: "SERVERR",
	502: "SERVBAD",
	503: "SERVUNAV",
	504: "SERVSLOW",
	521: "SERVOFF",
	522: "SERVSLOW",
	523: "SERVOFF",
	524: "SERVSLOW",
	525: "SERVSSL",
	526: "SERVSSL"
}

function resizeWindow(){
	// ensure a render tick has occured before we resize,
	// so that we know we're resizing to the right size.
	setTimeout(() => {
		ipcRenderer.send("resize-installer", document.scrollingElement.scrollHeight);
	},5)
}
let lock = false;
ipcRenderer.on("install-pack", (event, packId) => {
	lock = true;
	const logo = document.getElementById("logo");
	const packageNameSection = document.getElementById("package-section");
	const packageNameHolder = document.getElementById("package-name");
	const askPrompt = document.getElementById("ask");

	let installation;
	const PACK_URL = `${BASE_URL}/${packId}/dist`;
	
	fetch(`${PACK_URL}/install.json`).then((response) => {
		console.log(response);
		console.log(response.ok);
		if(response.ok){
			response.json().then((data) => {
				installation = data;
				logo.innerText = "Sound Pack";
				packageNameHolder.innerText = data.name;
				packageNameSection.style.display = "block";
				askPrompt.style.display = "block";
				resizeWindow();
			}).catch((r) => {
				// json parse error
				// NOTE: in theory this shouldn't happen.
				lock = false;
				logo.innerText = `Error (PARSE)`;
			});
		}else{
			lock = false;
			if(errorTranslation[response.status]){
				logo.innerText = `Error (${errorTranslation[response.status]})`;
			}else{
				logo.innerText = `Error (UNKNOWN)`;
			}
		}
	})

	const yesBtn = document.getElementById("answer-yes");
	const noBtn = document.getElementById("answer-no");
	yesBtn.onclick = () => {
		const progStatus = document.getElementById("status-text");
		const progSection = document.getElementById("prog");
		const progBar = document.getElementById("prog-bar");
		askPrompt.style.display = "none";

		const INSTALL_DIR = `${CUSTOM_PACKS_DIR}/${installation.folder}`;
		if(!fs.existsSync(INSTALL_DIR)){
			fs.mkdirSync(INSTALL_DIR);
		}
		
		setTimeout(async () => {
			progSection.style.display = "block";
			resizeWindow();
			let progress = 0;
			let error = null;
			for (const i in installation.files) {
				const file = installation.files[i];
				progStatus.innerText = `Downloading ${file}...`;
				const request = await fetch(`${PACK_URL}/${file}`);
				if(!request.ok){
					error = {
						status:request.status,
						file:file
					};
					break;
				}
				const blob = await request.blob();
				const arrayBuffer = await blob.arrayBuffer();
				const buffer = Buffer.from(arrayBuffer);
				fs.writeFileSync(`${INSTALL_DIR}/${file}`, buffer);

				progress = ((Number(i) + 1) / installation.files.length) * 100;
				progBar.style.width = `${progress}%`;
			}

			if(error !== null){
				lock = false;
				if(errorTranslation[error.status]){
					progStatus.innerText = `Failed to download ${error.file} (${errorTranslation[error.status]})`;
				}else{
					progStatus.innerText = `Failed to download ${error.file} (UNKNOWN)`;
				}
			}else{
				progStatus.innerText = "Installing...";
				ipcRenderer.send("installed", installation.folder);
			}
		},50)
	}
	noBtn.onclick = () => {
		window.close();
	}

	console.log("Attempting to install...")
	console.log(packId);
})

ipcRenderer.on("resize-done", (event) => {
	console.log()
})