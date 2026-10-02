import { J as proxy, i as api } from "./api.js";
//#region src/surfaces/plan/store.svelte.ts
var specs = proxy({
	projects: null,
	omitted: 0,
	error: ""
});
var users = 0;
var timer = null;
async function read() {
	try {
		const r = await api("/api/specs");
		specs.projects = r.projects ?? [];
		specs.omitted = r.omitted ?? 0;
		specs.error = "";
	} catch (e) {
		specs.error = e instanceof Error ? e.message : String(e);
	}
}
function watch() {
	users += 1;
	if (users === 1) {
		read();
		timer = setInterval(read, 15e3);
	}
	return () => {
		users -= 1;
		if (users === 0 && timer) {
			clearInterval(timer);
			timer = null;
		}
	};
}
var key = (p, r) => `${p.project}/${r.plan.path}`;
//#endregion
export { specs as n, watch as r, key as t };

//# sourceMappingURL=store.svelte.js.map