//#region src/lib/frame.ts
function hideWindow() {
	if (typeof window === "undefined" || typeof window.__devplane_hide !== "function") return false;
	window.__devplane_hide();
	return true;
}
//#endregion
export { hideWindow as t };

//# sourceMappingURL=frame.js.map