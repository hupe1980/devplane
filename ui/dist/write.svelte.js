import { R as get, X as set, Z as state } from "./api.js";
import { s as ago } from "./app.js";
//#region src/lib/write.svelte.ts
function writer() {
	let busy = state("");
	let since = state(0);
	let now = state(0);
	let tick = null;
	return {
		get busy() {
			return get(busy);
		},
		get elapsed() {
			return get(busy) ? ago(Math.max(0, (get(now) - get(since)) / 1e3)) : "";
		},
		async run(what, work) {
			if (get(busy)) return null;
			set(busy, what, true);
			set(since, set(now, Date.now(), true), true);
			tick = setInterval(() => set(now, Date.now(), true), 1e3);
			try {
				return await work();
			} finally {
				if (tick !== null) clearInterval(tick);
				tick = null;
				set(busy, "");
			}
		}
	};
}
//#endregion
export { writer as t };

//# sourceMappingURL=write.svelte.js.map