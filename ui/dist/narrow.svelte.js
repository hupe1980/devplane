import { Q as user_derived, R as get, U as user_effect, Y as proxy, i as api, rt as snapshot } from "./api.js";
import { g as same, m as failure } from "./app.js";
//#region src/surfaces/inbox/narrow.svelte.ts
var EVERY_MS = 2e3;
var shared = proxy({
	project: "",
	data: null,
	failure: null,
	at: null
});
var inflight = 0;
var seq = 0;
var landed = 0;
var users = 0;
var timer = null;
async function read(project, force = false) {
	if (inflight > 0 && !force) return;
	const n = ++seq;
	inflight += 1;
	try {
		const r = await api(`/api/inbox?project=${encodeURIComponent(project)}`);
		if (shared.project !== project || n < landed) return;
		landed = n;
		if (!same(snapshot(shared.data), r)) shared.data = r;
		shared.failure = null;
		shared.at = Date.now();
	} catch (e) {
		if (shared.project !== project || n < landed) return;
		landed = n;
		shared.failure = failure(e, `devplane inbox --project ${project}`);
	} finally {
		inflight = Math.max(0, inflight - 1);
	}
}
function narrowed(project) {
	const key = user_derived(() => project().trim());
	user_effect(() => {
		const want = get(key);
		if (!want) return;
		if (shared.project !== want) {
			shared.project = want;
			shared.data = null;
			shared.failure = null;
			shared.at = null;
			landed = seq;
		}
		users += 1;
		if (users === 1 || shared.data === null) read(want);
		if (timer === null) timer = setInterval(() => {
			if (!document.hidden && shared.project) read(shared.project);
		}, EVERY_MS);
		return () => {
			users -= 1;
			if (users === 0 && timer !== null) {
				clearInterval(timer);
				timer = null;
			}
		};
	});
	const mine = () => !!get(key) && shared.project === get(key);
	return {
		get on() {
			return !!get(key);
		},
		get data() {
			return mine() ? shared.data : null;
		},
		get failure() {
			return mine() ? shared.failure : null;
		},
		get at() {
			return mine() ? shared.at : null;
		},
		get phase() {
			const d = mine() ? shared.data : null;
			const f = mine() ? shared.failure : null;
			return d ? f ? "stale" : "ok" : f ? "failed" : "loading";
		},
		get missing() {
			return mine() && shared.data?.narrowed?.no_such_project === true;
		},
		reload: () => get(key) ? read(get(key), true) : Promise.resolve()
	};
}
//#endregion
export { narrowed as t };

//# sourceMappingURL=narrow.svelte.js.map