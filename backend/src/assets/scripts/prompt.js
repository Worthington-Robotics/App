let promptContainer, promptConfirm, promptText;

function loadPrompt() {
	promptContainer = document.getElementById("prompt-container");
	promptConfirm = document.getElementById("prompt-confirm");
	promptText = document.getElementById("prompt-text");

	document.getElementById("prompt-cancel").addEventListener("click", closePrompt);
	promptConfirm.addEventListener("click", () => {
		closePrompt();
		confirmAction();
	});
}

document.getElementById("prompt-script").addEventListener("load", () => {
	loadPrompt();
});

let confirmAction = () => { };

// Show a prompt with a message that follows "Are you sure you want to {message}?" and an action on confirm
function showConfirmPrompt(message, onConfirm) {
	promptContainer.style.display = "";
	promptText.innerHTML = `Are you sure you want to ${message}?`;
	promptConfirm.innerHTML = "Confirm";
	confirmAction = onConfirm;
}

/// Closes / cancels the prompt
function closePrompt() {
	promptContainer.style.display = "none";
}
