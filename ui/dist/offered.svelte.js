import { K as proxy } from "./api.js";
//#region src/lib/offered.svelte.ts
var here = proxy({
	title: "",
	list: [],
	by: 0
});
var offers = 0;
function offerCommands(title, list) {
	const by = ++offers;
	here.title = title;
	here.list = list;
	here.by = by;
	return () => {
		if (here.by === by) {
			here.title = "";
			here.list = [];
			here.by = 0;
		}
	};
}
//#endregion
export { offerCommands as n, here as t };

//# sourceMappingURL=offered.svelte.js.map