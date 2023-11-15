const fs = require('fs');
const { shell, remote, ipcRenderer } = require('electron');
const BASE_URL = "https://mechvibes.lunarwebsite.ca/sound-packs";
const CUSTOM_PACKS_DIR = remote.getGlobal('custom_dir');

function resizeWindow(){
	// ensure a render tick has occured before we resize,
	// so that we know we're resizing to the right size.
	setTimeout(() => {
		ipcRenderer.send("resize-installer", document.scrollingElement.scrollHeight);
	},5)
}

ipcRenderer.on("install-pack", (event, packId) => {
	const logo = document.getElementById("logo");
	const packageNameSection = document.getElementById("package-section");
	const packageNameHolder = document.getElementById("package-name");
	const askPrompt = document.getElementById("ask");

	let installation;
	const PACK_URL = `${BASE_URL}/${packId}/dist`;
	
	fetch(`${PACK_URL}/install.json`).then((response) => response.json())
	.then((data) => {
		installation = data;
		logo.innerText = "Sound Pack";
		packageNameHolder.innerText = data.name;
		packageNameSection.style.display = "block";
		askPrompt.style.display = "block";
		resizeWindow();
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
			for (const i in installation.files) {
				const file = installation.files[i];
				progStatus.innerText = `Downloading ${file}...`;
				// console.log(`${PACK_URL}/${file}`);
				const request = await fetch(`${PACK_URL}/${file}`);
				const blob = await request.blob();
				const arrayBuffer = await blob.arrayBuffer();
				const buffer = Buffer.from(arrayBuffer);
				fs.writeFileSync(`${INSTALL_DIR}/${file}`, buffer);

				progress = ((Number(i) + 1) / installation.files.length) * 100;
				progBar.style.width = `${progress}%`;
			}

			progStatus.innerText = "Installing...";
			ipcRenderer.send("installed", packId);
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