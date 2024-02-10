const { ipcRenderer } = require('electron');

let debug = undefined;

function onReady(){
	console.log(document.readyState);
	if(debug === undefined){
		console.error("debug options aren't set yet?");
		return;
	}else{
		console.log(debug);
	}

	const enable_toggle_group = document.getElementById("remote_toggle_group");

	enable_toggle_group.addEventListener("click", (e) => {
		debug.enabled = !debug.enabled;
		setDebugOptions(debug);
		refresh();
	})
}

function refresh(){
	const enable_toggle = document.getElementById("remote_toggle");
	enable_toggle.checked = debug.enabled;
	const remote_options_group = document.getElementById("remote_options");
	if(debug.enabled){
		remote_options_group.style.display = "block";
	}else{
		remote_options_group.style.display = "none";
	}
	const debug_code = document.getElementById("debug_code");
	debug_code.value = "Example";
}

function getDebugOptions(){
	return new Promise((resolve) => {
		ipcRenderer.once("debug-options", (event, json) => {
			resolve(json);
		})
		ipcRenderer.send("fetch-debug-options");
	})
}
function setDebugOptions(options){
	ipcRenderer.send("set-debug-options", options);
}

ipcRenderer.once("debug-options", (event, json) => {
	debug = json;
	onReady();
})

ipcRenderer.on("debug-update", (event, type, message) => {

})

// $("#remote_toggle_group").on("click", () => {
// 	console.log("...");
// })