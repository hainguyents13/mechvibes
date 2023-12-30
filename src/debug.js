const { shell, remote, ipcRenderer } = require('electron');
const fs = require('fs');
const path = require('path');

let debug = undefined;
let websocket = null;

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