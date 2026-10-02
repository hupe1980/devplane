//#region src/lib/href.ts
var SCHEMES = /* @__PURE__ */ new Set([
	"http:",
	"https:",
	"devplane:",
	"vscode:",
	"claude-cli:"
]);
function safeHref(url) {
	if (!url) return null;
	const s = url.trim();
	if (s.startsWith("#")) return s;
	const m = /^([a-z][a-z0-9+.-]*:)/i.exec(s);
	if (!m) return null;
	return SCHEMES.has(m[1].toLowerCase()) ? s : null;
}
//#endregion
export { safeHref as t };

//# sourceMappingURL=href.js.map