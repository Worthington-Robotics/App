function setError(error) {
	let text = "Error: " + error;
	console.error(text);
	let errorElement = document.getElementById("error");
	errorElement.style.display = "flex";
	errorElement.innerHTML = text;
	setTimeout(() => {
		errorElement.style.display = "none";
	}, 3000);
}