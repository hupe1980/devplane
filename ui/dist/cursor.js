import { D as onAction } from "./app.js";
//#region src/lib/cursor.ts
function cursor(c) {
	const clamp = (i) => Math.min(Math.max(i, 0), c.size() - 1);
	const on = (action, move) => onAction(action, (surface) => {
		if (surface !== c.scope || c.size() === 0) return false;
		const to = move(c.at());
		if (to !== null) c.go(clamp(to));
		return true;
	});
	const offs = [
		on("next", (at) => at + 1),
		on("prev", (at) => at < 0 ? 0 : at - 1),
		on("first", () => 0),
		on("last", () => c.size() - 1),
		on("open", (at) => {
			c.open(clamp(at < 0 ? 0 : at));
			return null;
		})
	];
	return () => offs.forEach((off) => off());
}
//#endregion
export { cursor as t };

//# sourceMappingURL=cursor.js.map