import { $ as user_effect, J as get, X as untrack, ct as user_derived, i as api, n as Unauthorised, ot as set, r as Unreachable, st as state, t as Refused } from "./api.js";
//#region src/lib/resource.svelte.ts
function failure(e, tell = "devplane doctor") {
	if (e instanceof Unauthorised) return {
		says: "this tab has no token",
		tell: "devplane up",
		down: true
	};
	if (e instanceof Unreachable) return {
		says: e.message,
		tell: e.kind === "refused" ? "devplane up --no-browser" : "devplane doctor",
		down: true
	};
	if (e instanceof Refused) {
		const body = typeof e.body === "string" ? e.body : e.body === null ? "" : JSON.stringify(e.body);
		return {
			says: e.message,
			tell,
			raw: `${e.path ? `${e.path} → ` : ""}${e.status}${body ? ` ${body}` : ""}`
		};
	}
	return {
		says: e instanceof Error ? e.message : String(e),
		tell
	};
}
function same(a, b) {
	if (a === b) return true;
	try {
		return JSON.stringify(a) === JSON.stringify(b);
	} catch {
		return false;
	}
}
function resource(url, opts = {}) {
	let data = state(null);
	let fail = state(null);
	let at = state(null);
	const key = user_derived(url);
	let gen = 0;
	let seq = 0;
	let landed = 0;
	let pending = 0;
	async function read(want, mine) {
		const n = ++seq;
		pending += 1;
		try {
			const r = await api(want);
			if (mine !== gen || n < landed) return;
			landed = n;
			if (!same(get(data), r)) set(data, r);
			set(fail, null);
			set(at, Date.now(), true);
		} catch (e) {
			if (mine !== gen || n < landed) return;
			landed = n;
			set(fail, {
				...failure(e, opts.tell?.()),
				retry: reload
			});
		} finally {
			if (mine === gen) pending = Math.max(0, pending - 1);
		}
	}
	user_effect(() => {
		const want = get(key);
		const mine = untrack(() => {
			gen += 1;
			pending = 0;
			landed = seq;
			set(data, null);
			set(fail, null);
			set(at, null);
			return gen;
		});
		if (!want) return;
		read(want, mine);
		if (!opts.every) return;
		const t = setInterval(() => {
			if (!document.hidden && pending === 0) read(want, mine);
		}, opts.every);
		return () => clearInterval(t);
	});
	async function reload() {
		const want = untrack(() => get(key));
		if (want) await read(want, gen);
	}
	return {
		get phase() {
			return get(data) !== null ? get(fail) ? "stale" : "ok" : get(fail) ? "failed" : "loading";
		},
		get data() {
			return get(data);
		},
		get failure() {
			return get(fail);
		},
		get at() {
			return get(at);
		},
		reload
	};
}
async function copyText(text) {
	try {
		if (!navigator.clipboard?.writeText) return false;
		await navigator.clipboard.writeText(text);
		return true;
	} catch {
		return false;
	}
}
//#endregion
export { same as i, failure as n, resource as r, copyText as t };

//# sourceMappingURL=resource.svelte.js.map