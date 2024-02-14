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
	const debug_code = document.getElementById("debug_code");

	enable_toggle_group.addEventListener("click", (e) => {
		debug.enabled = !debug.enabled;
		if(!debug.enabled){
			debug.identifier = undefined;
		}
		setDebugOptions(debug);
		refresh();
	})

	debug_code.addEventListener("focus", (e) => {
		debug_code.select();
	});
	refresh();
}

function refresh(){
	const enable_toggle = document.getElementById("remote_toggle");
	const remote_options_group = document.getElementById("remote_options");
	const debug_code = document.getElementById("debug_code");

	enable_toggle.checked = debug.enabled;
	if(debug.enabled){
		remote_options_group.style.display = "block";
		if(debug.identifier !== undefined){
			debug_code.value = debug.identifier;
		}else{
			debug_code.value = "";
		}
	}else{
		remote_options_group.style.display = "none";
		debug_code.value = "";
	}
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

ipcRenderer.on("debug-update", (event, json) => {
	debug = json;
	refresh();
})

// $("#remote_toggle_group").on("click", () => {
// 	console.log("...");
// })