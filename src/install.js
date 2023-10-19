const { shell, remote, ipcRenderer } = require('electron');
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
	
	fetch("https://mechvibes.lunarwebsite.ca/").then(() => {
		logo.innerText = "Sound Pack";
		packageNameHolder.innerText = "CherryMX Red - ABS keycaps";
		packageNameSection.style.display = "block";
		askPrompt.style.display = "block";
		resizeWindow();
	})

	const yesBtn = document.getElementById("answer-yes");
	const noBtn = document.getElementById("answer-no");
	yesBtn.onclick = () => {
		const progSection = document.getElementById("prog");
		const progBar = document.getElementById("prog-bar");
		askPrompt.style.display = "none";
		
		setTimeout(() => {
			progSection.style.display = "block";
			resizeWindow();
			let progress = 0;
			setInterval(() => {
				progress += 1;
				progBar.style.width = `${progress}%`;
			}, 100)
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