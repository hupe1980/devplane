const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=["Answer.js","api.js","resource.svelte.js","frame.js","cursor.js","write.svelte.js","Failed.js","Failed.css","unread.svelte.js","unread.css","actions.js","href.js","Item.js","Qualifier.js","Qualifier.css","Item.css","Answer.css","Board.js","Grid.js","Grid.css","Empty.js","Empty.css","Props.js","Props.css","offered.svelte.js","Board.css","List.js","ChangeState.js","ChangeState.css","List.css","Doc.js","Tabs.js","Tabs.css","Edits.js","Edits.css","Doc.css","Github.js","Github.css","List2.js","Snoozed.js","Snoozed.css","List2.css","Inbox.js","Inbox.css","New.js","New.css","Palette.js","Palette.css","List3.js","store.svelte.js","List3.css","Doc2.js","Doc2.css","Quit.js","Quit.css","Search.js","Search.css","Setup.js","Setup.css","Why.js","Why.css"])))=>i.map(i=>d[i]);
import { $ as user_effect, B as append, F as index, G as delegate, H as from_html, J as get, K as delegated, L as if_block, M as component, N as snippet, O as set_style, P as each, Q as template_effect, R as mount, U as from_svg, V as comment, W as text, X as untrack, _ as rest_props, a as claimToken, at as proxy, c as bindList, ct as user_derived, d as onAction, dt as pop, f as run, ft as push, g as prop, gt as noop, h as spell, ht as reset, i as api, it as sibling, j as action, k as set_class, l as dispatch, m as sheet, mt as next, n as Unauthorised, nt as first_child, ot as set, pt as snapshot, q as event, rt as only_child, s as bind, st as state, tt as child, v as spread_props, w as set_attribute, z as set_text } from "./api.js";
import { i as same$1 } from "./resource.svelte.js";
//#region \0vite/modulepreload-polyfill.js
(function polyfill() {
	const relList = document.createElement("link").relList;
	if (relList && relList.supports && relList.supports("modulepreload")) return;
	for (const link of document.querySelectorAll("link[rel=\"modulepreload\"]")) processPreload(link);
	new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			if (mutation.type !== "childList") continue;
			for (const node of mutation.addedNodes) if (node.tagName === "LINK" && node.rel === "modulepreload") processPreload(node);
		}
	}).observe(document, {
		childList: true,
		subtree: true
	});
	function getFetchOpts(link) {
		const fetchOpts = {};
		if (link.integrity) fetchOpts.integrity = link.integrity;
		if (link.referrerPolicy) fetchOpts.referrerPolicy = link.referrerPolicy;
		if (link.crossOrigin === "use-credentials") fetchOpts.credentials = "include";
		else if (link.crossOrigin === "anonymous") fetchOpts.credentials = "omit";
		else fetchOpts.credentials = "same-origin";
		return fetchOpts;
	}
	function processPreload(link) {
		if (link.ep) return;
		link.ep = true;
		const fetchOpts = getFetchOpts(link);
		fetch(link.href, fetchOpts);
	}
})();
//#endregion
//#region src/lib/surfaces.ts
function phase(feed) {
	return {
		loaded: feed.loaded ?? false,
		stale_since: feed.stale_since ?? null,
		error: feed.error ?? null
	};
}
var BANDS = [
	"attention",
	"happening",
	"steering",
	"project"
];
var BAND_LABELS = {
	attention: "What needs you",
	happening: "See what is happening",
	steering: "Start and steer work",
	project: "Set up a project"
};
var registry = /* @__PURE__ */ new Map();
function register(surface) {
	if (registry.has(surface.id)) throw new Error(`two surfaces claim the id "${surface.id}"`);
	registry.set(surface.id, surface);
}
function listed$1() {
	return surfaces().filter((s) => s.nav !== false);
}
function surfaces() {
	return [...registry.values()].sort((a, b) => BANDS.indexOf(a.band) - BANDS.indexOf(b.band) || a.order - b.order || a.id.localeCompare(b.id));
}
function landing() {
	return surfaces()[0];
}
async function opening(ms = 1500) {
	for (const s of surfaces()) {
		if (!s.opensFirst) continue;
		if (await Promise.race([s.opensFirst().catch(() => false), new Promise((r) => setTimeout(() => r(false), ms))])) return s;
	}
	return landing();
}
//#endregion
//#region src/lib/sight.ts
function unseen(board) {
	const b = board;
	if (!b || typeof b !== "object") return "";
	if ((b.runs?.length ?? 0) > 0 || (b.summary?.runs ?? 0) > 0) return "";
	const only = b.watching?.driven_only ?? [];
	const unproved = b.watching?.unproved ?? [];
	return [
		"No session has reported to Devplane on this machine yet, so this cannot say what your agents are doing.",
		only.length ? `${only.join(", ")} ${only.length === 1 ? "appears" : "appear"} only when Devplane starts ${only.length === 1 ? "it" : "them"};` : "",
		unproved.length ? `${unproved.join(", ")} ${unproved.length === 1 ? "is" : "are"} read once connected.` : "",
		"`devplane doctor` says what is watched here."
	].filter(Boolean).join(" ");
}
//#endregion
//#region src/surfaces/inbox/keys.ts
var ITEM_KEYS = [
	...[
		1,
		2,
		3,
		4,
		5,
		6,
		7,
		8,
		9
	].map((n) => ({
		combo: String(n),
		action: "item-option",
		label: "choose the option with that number"
	})),
	{
		combo: "t",
		action: "item-type",
		label: "type an answer in your own words"
	},
	{
		combo: "Mod+Enter",
		action: "item-send",
		label: "send the typed answer (from inside the field)"
	},
	{
		combo: "a",
		action: "item-allow",
		label: "allow the permission, once"
	},
	{
		combo: "d",
		action: "item-deny",
		label: "deny the permission"
	},
	{
		combo: "w",
		action: "item-session",
		label: "reach the session the vendor owns: open its link, or copy its resume command"
	},
	{
		combo: "s",
		action: "item-snooze",
		label: "snooze the item until there is new activity (an hour for a pull request or an issue)"
	},
	{
		combo: "o",
		action: "item-open",
		label: "open the change or session the item is about"
	},
	{
		combo: "r",
		action: "item-review",
		label: "open the change's review"
	},
	{
		combo: "z",
		action: "item-undo",
		label: "undo: take back the answer before it is sent, or put a snoozed item back"
	}
];
function bindItemKeys(surface) {
	for (const k of ITEM_KEYS) bind({
		surface,
		...k,
		item: true
	});
}
function owner(item) {
	return /^(.+?) owns this dialog/.exec(item.answer_in ?? "")?.[1] ?? "the agent's own window";
}
function optionCount(item, picked = /* @__PURE__ */ new Set()) {
	const has = (a) => (item.actions ?? []).includes(a);
	if (!has("choose") && !(item.form ?? []).length) return 0;
	const field = (item.form ?? []).find((q) => !picked.has(q.field));
	if (item.form?.length) return field?.options.length ?? 0;
	return (item.options ?? []).filter((o) => o.id).length;
}
function whyNot(item, action, n = 0) {
	const acts = item.actions ?? [];
	const has = (a) => acts.includes(a);
	const vendor = !!item.answer_in;
	const reach = item.session_link || item.resume_command ? " — w reaches its session" : "";
	const kind = item.kind.replace(/_/g, " ");
	switch (action) {
		case "item-option": {
			const count = optionCount(item);
			if (vendor) return `${owner(item)} owns this question: answer it there${reach}`;
			if (count === 0) return `this ${kind} has no options to choose`;
			if (n > count) return `there ${count === 1 ? "is 1 option" : `are ${count} options`}; ${n} picks nothing`;
			return "";
		}
		case "item-type":
		case "item-send":
			if (has("reply") || (item.form ?? []).some((q) => q.custom_field)) return "";
			return vendor ? `${owner(item)} owns this question: answer it there${reach}` : `this ${kind} takes no answer in words`;
		case "item-allow":
		case "item-deny":
			if (has(action === "item-allow" ? "allow" : "deny")) return "";
			if (vendor) return `${owner(item)} owns this permission: Devplane cannot answer it${reach}`;
			return `${action === "item-allow" ? "a" : "d"} answers a permission; this is a ${kind}`;
		case "item-session":
			if (vendor && (item.session_link || item.resume_command)) return "";
			if (vendor) return `${owner(item)} documents no way to reach this session from here`;
			return "w is for a dialog the vendor owns; this one is answered here";
		case "item-snooze":
			if (has("snooze")) return "";
			if (item.kind === "permission" || item.kind.startsWith("question")) return "a question or a permission is never snoozed: it waits for an answer";
			return `this ${kind} cannot be snoozed`;
		case "item-open": return (has("open") || has("review")) && (item.change_id || item.run_id) ? "" : `this ${kind} is about nothing that opens`;
		case "item-review": return has("review") && item.change_id ? "" : "r opens the review of a change ready to decide";
		default: return "";
	}
}
function answerItemKeys(scope, root, item, say) {
	const control = (k) => root()?.querySelector(`[data-key="${k}"]`) ?? null;
	const press = (action, run) => (surface, combo) => {
		if (surface !== scope) return false;
		const el = control(combo);
		if (el && (el.disabled || el.closest("fieldset:disabled"))) return true;
		if (el) (run ?? ((x) => x.click()))(el);
		else say(whyNot(item(), action, Number(combo) || 0) || `${spell(combo)} does nothing on this item right now`);
		return true;
	};
	const offs = [
		onAction("item-option", press("item-option")),
		onAction("item-type", press("item-type", (el) => el.focus())),
		onAction("item-send", (surface) => {
			if (surface !== scope) return false;
			const field = document.activeElement;
			const send = field && root()?.contains(field) ? field.closest(".reply")?.querySelector("[data-send]") : null;
			if (send) send.click();
			else say(whyNot(item(), "item-send") || "this sends from inside the answer field — t puts you there");
			return true;
		}),
		onAction("item-allow", press("item-allow")),
		onAction("item-deny", press("item-deny")),
		onAction("item-session", press("item-session")),
		onAction("item-snooze", press("item-snooze")),
		onAction("item-open", press("item-open")),
		onAction("item-review", press("item-review"))
	];
	return () => offs.forEach((off) => off());
}
//#endregion
//#region \0vite/preload-helper.js
var scriptRel = "modulepreload";
var assetsURL = function(dep) {
	return "/" + dep;
};
var seen = {};
var __vitePreload = function preload(baseModule, deps, importerUrl) {
	let promise = Promise.resolve();
	if (deps && deps.length > 0) {
		const links = document.getElementsByTagName("link");
		const cspNonceMeta = document.querySelector("meta[property=csp-nonce]");
		const cspNonce = cspNonceMeta?.nonce || cspNonceMeta?.getAttribute("nonce");
		function allSettled(promises) {
			return Promise.all(promises.map((p) => Promise.resolve(p).then((value) => ({
				status: "fulfilled",
				value
			}), (reason) => ({
				status: "rejected",
				reason
			}))));
		}
		function importMetaResolve(specifier) {
			if (import.meta.resolve) return import.meta.resolve(specifier);
			return new URL(
				specifier,
				/** #__KEEP__ */
				import.meta.url
			).href;
		}
		promise = allSettled(deps.map((dep) => {
			dep = assetsURL(dep, importerUrl);
			dep = importMetaResolve(dep);
			if (dep in seen) return;
			seen[dep] = true;
			const isCss = dep.endsWith(".css");
			for (let i = links.length - 1; i >= 0; i--) {
				const link = links[i];
				if (link.href === dep && (!isCss || link.rel === "stylesheet")) return;
			}
			const link = document.createElement("link");
			link.rel = isCss ? "stylesheet" : scriptRel;
			if (!isCss) link.as = "script";
			link.crossOrigin = "";
			link.href = dep;
			if (cspNonce) link.setAttribute("nonce", cspNonce);
			document.head.appendChild(link);
			if (isCss) return new Promise((res, rej) => {
				link.addEventListener("load", res);
				link.addEventListener("error", () => rej(/* @__PURE__ */ new Error(`Unable to preload CSS for ${dep}`)));
			});
		}).filter((p) => p !== void 0));
	}
	function handlePreloadError(err) {
		const e = new Event("vite:preloadError", { cancelable: true });
		e.payload = err;
		window.dispatchEvent(e);
		if (!e.defaultPrevented) throw err;
	}
	return promise.then((res) => {
		for (const item of res || []) {
			if (item.status !== "rejected") continue;
			handlePreloadError(item.reason);
		}
		return baseModule().catch(handlePreloadError);
	});
};
//#endregion
//#region src/surfaces/answer/index.ts
bindList("answer", "move into the item (Tab then reaches its controls)");
bindItemKeys("answer");
register({
	id: "answer",
	title: "Answer",
	heading: "The one thing that needs you",
	band: "attention",
	order: 9,
	nav: false,
	bare: true,
	reads: ["/api/inbox"],
	select: (feed, focus) => ({
		blind: unseen(feed.board),
		ask: focus
	}),
	page: () => __vitePreload(() => import("./Answer.js"), __vite__mapDeps([0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16]))
});
//#endregion
//#region src/lib/route.ts
function go(hash) {
	history.replaceState({}, "", hash || "#");
	dispatchEvent(new HashChangeEvent("hashchange"));
}
//#endregion
//#region src/surfaces/board/index.ts
bind({
	surface: "global",
	combo: "g b",
	action: "go-board",
	label: "go to the sessions"
});
bindList("board", "open the session's decisions in the ledger");
onAction("go-board", () => {
	go("#board");
	return true;
});
register({
	id: "board",
	icon: "sessions",
	title: "Sessions",
	heading: "Sessions",
	band: "happening",
	order: 0,
	holds: "run",
	status: (feed) => {
		const s = feed.board?.summary;
		if (!s) return [];
		return [{
			n: s.working ?? 0,
			word: "working",
			icon: "sessions"
		}, ...s.failed ? [{
			n: s.failed,
			word: "failed",
			icon: "alert",
			tone: "fail"
		}] : []];
	},
	count: (feed) => {
		return feed.board?.runs?.length ?? null;
	},
	select: (feed, focus) => {
		const b = feed.board;
		return {
			runs: b?.runs ?? [],
			summary: b?.summary,
			coverage: b?.coverage ?? null,
			thresholds: b?.thresholds ?? null,
			watching: b?.watching ?? null,
			focus,
			...phase(feed)
		};
	},
	page: () => __vitePreload(() => import("./Board.js"), __vite__mapDeps([17,1,2,4,18,19,20,21,22,23,24,25]))
});
//#endregion
//#region src/lib/State.svelte
var STATES = {
	working: {
		glyph: "●",
		word: "working",
		tone: "t-work"
	},
	"needs you": {
		glyph: "◆",
		word: "needs you",
		tone: "t-wait"
	},
	failed: {
		glyph: "✕",
		word: "failed",
		tone: "t-fail"
	},
	verified: {
		glyph: "✓",
		word: "verified",
		tone: "t-done"
	},
	idle: {
		glyph: "○",
		word: "idle",
		tone: "t-dim"
	},
	drafted: {
		glyph: "·",
		word: "drafted",
		tone: "t-dim"
	},
	isolated: {
		glyph: "⎇",
		word: "isolated",
		tone: "t-ink"
	},
	"in flight": {
		glyph: "▶",
		word: "in flight",
		tone: "t-work"
	},
	offered: {
		glyph: "↗",
		word: "offered",
		tone: "t-ink"
	},
	archived: {
		glyph: "▣",
		word: "archived",
		tone: "t-dim"
	},
	"no gates declared": {
		glyph: "∅",
		word: "no gates declared",
		tone: "t-wait"
	},
	stale: {
		glyph: "≠",
		word: "stale",
		tone: "t-wait"
	}
};
var LIFE = [
	"drafted",
	"isolated",
	"in flight",
	"verified",
	"offered",
	"archived"
];
var isArchived = (state) => state === STATES.archived.word;
var root$14 = from_html(`<span class="sr-only"> </span>`);
var root_1$12 = from_html(`<span><span class="glyph svelte-10029po" aria-hidden="true"> </span><!></span>`);
function State($$anchor, $$props) {
	let quiet = prop($$props, "quiet", 3, false);
	const row = user_derived(() => STATES[$$props.state]);
	const said = user_derived(() => $$props.word ?? get(row).word);
	var span = root_1$12();
	var span_1 = child(span);
	var text$2 = only_child(span_1, true);
	var node = sibling(span_1);
	var consequent = ($$anchor) => {};
	var consequent_1 = ($$anchor) => {
		var span_2 = root$14();
		var text_1 = only_child(span_2, true);
		template_effect(() => set_text(text_1, get(said)));
		append($$anchor, span_2);
	};
	var alternate = ($$anchor) => {
		var text_2 = text();
		template_effect(() => set_text(text_2, get(said)));
		append($$anchor, text_2);
	};
	if_block(node, ($$render) => {
		if (!get(said)) $$render(consequent);
		else if (quiet()) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	reset(span);
	template_effect(() => {
		set_class(span, 1, `state ${get(row).tone ?? ""}`, "svelte-10029po");
		set_text(text$2, get(row).glyph);
	});
	append($$anchor, span);
}
//#endregion
//#region src/surfaces/change/index.ts
bind({
	surface: "global",
	combo: "g c",
	action: "go-changes",
	label: "go to the changes"
});
bind({
	surface: "change",
	combo: "r",
	action: "review",
	label: "review the change"
});
bindList("change", "open the change as a tab of its own");
[
	"overview",
	"tasks",
	"review",
	"gates",
	"ledger",
	"agent"
].forEach((v, i) => bind({
	surface: "change",
	combo: `Alt+${i + 1}`,
	action: `view-${v}`,
	label: `show the ${v}`
}));
onAction("go-changes", () => {
	go("#change");
	return true;
});
for (const [combo, action, label] of [
	[
		"j",
		"hunk-next",
		"next hunk, across files"
	],
	[
		"k",
		"hunk-prev",
		"previous hunk, across files"
	],
	[
		"n",
		"file-next",
		"next file"
	],
	[
		"p",
		"file-prev",
		"previous file"
	],
	[
		"s",
		"hunk-seen",
		"mark this hunk seen — kept by Devplane until the hunk changes"
	],
	[
		"f",
		"hunk-fix",
		"request a fix: start a comment on this hunk, for the pending review"
	],
	[
		"Mod+Enter",
		"review-send",
		"send the pending review: every comment, as one turn to the agent"
	],
	[
		"x",
		"hunk-expand",
		"expand this file's collapsed formatter-only hunks"
	],
	[
		"v",
		"diff-mode",
		"switch between unified and side-by-side"
	],
	[
		"1",
		"tab-risk",
		"read by risk"
	],
	[
		"2",
		"tab-intent",
		"read by intent"
	]
]) bind({
	surface: "change:review",
	combo,
	action,
	label
});
var briefs = (feed) => feed.board?.changes ?? [];
register({
	id: "change",
	icon: "change",
	title: "Changes",
	heading: "Changes",
	band: "attention",
	order: 1,
	link: "change",
	count: (feed) => {
		const b = feed.board;
		return b?.changes ? b.changes.filter((c) => !isArchived(c.state)).length : null;
	},
	tab: (feed, focus) => briefs(feed).find((c) => c.id === focus)?.title ?? "Change",
	select: (feed, focus) => {
		const all = briefs(feed);
		return {
			id: focus,
			all,
			brief: all.find((c) => c.id === focus) ?? null,
			loaded: phase(feed).loaded
		};
	},
	side: () => __vitePreload(() => import("./List.js"), __vite__mapDeps([26,1,2,4,6,7,27,13,14,28,29])),
	document: true,
	page: () => __vitePreload(() => import("./Doc.js"), __vite__mapDeps([30,1,2,5,6,7,8,9,18,19,20,21,22,23,24,27,13,14,28,31,32,33,34,35]))
});
//#endregion
//#region src/surfaces/github/index.ts
bind({
	surface: "global",
	combo: "g f",
	action: "go-forge",
	label: "go to the forge"
});
bindList("github", "open the issue or pull request on GitHub");
onAction("go-forge", () => {
	go("#github");
	return true;
});
register({
	id: "github",
	icon: "forge",
	title: "Forge",
	heading: "Issues and pull requests",
	band: "steering",
	order: 1,
	count: (feed) => {
		const per = feed.board?.forge;
		if (!per) return null;
		return Object.values(per).reduce((n, f) => n + (f.needs_you ?? 0), 0);
	},
	reads: ["/api/forge"],
	select: (feed) => {
		const b = feed.board;
		if (!b?.projects) return {};
		return { coverage: {
			projects: b.projects.length,
			configured: Object.keys(b.forge ?? {}).length
		} };
	},
	page: () => __vitePreload(() => import("./Github.js"), __vite__mapDeps([36,1,2,4,6,7,11,18,19,20,21,31,32,37]))
});
//#endregion
//#region src/surfaces/inbox/place.svelte.ts
function address(focus, items) {
	if (focus.startsWith("item=")) return {
		item: focus.slice(5),
		wanted: "",
		project: ""
	};
	if (focus.startsWith("ask=")) return {
		item: "",
		wanted: focus.slice(4),
		project: ""
	};
	if (focus && items.some((i) => i.id === focus)) return {
		item: focus,
		wanted: "",
		project: ""
	};
	return {
		item: "",
		wanted: "",
		project: focus
	};
}
var itemFocus = (id) => `item=${id}`;
var last = proxy({
	id: "",
	index: 0
});
function remember(id, index) {
	if (last.id !== id || last.index !== index) {
		last.id = id;
		last.index = index;
	}
}
function place(shown, a) {
	if (a.item) {
		const i = shown.findIndex((x) => x.id === a.item);
		if (i !== -1) return {
			current: shown[i],
			gone: ""
		};
		const at = last.id === a.item ? Math.min(last.index, shown.length - 1) : 0;
		return {
			current: shown[Math.max(0, at)] ?? null,
			gone: "item"
		};
	}
	if (a.wanted) {
		const x = shown.find((y) => y.ask === a.wanted || y.request_id === a.wanted);
		return x ? {
			current: x,
			gone: ""
		} : {
			current: shown[0] ?? null,
			gone: "ask"
		};
	}
	return {
		current: shown[0] ?? null,
		gone: ""
	};
}
//#endregion
//#region src/surfaces/inbox/index.ts
function inboxCount(feed) {
	const i = feed.inbox;
	if (!i?.items) return null;
	return typeof i.count === "number" ? i.count : i.items.length;
}
var COUNT_WORD = "need you";
bindList("inbox", "move into the item (Tab then reaches its controls)");
bindItemKeys("inbox");
bind({
	surface: "global",
	combo: "g i",
	action: "go-inbox",
	label: "go to the inbox"
});
onAction("go-inbox", () => {
	go("#inbox");
	return true;
});
register({
	id: "inbox",
	icon: "inbox",
	title: "Inbox",
	heading: "What needs you",
	band: "attention",
	order: 0,
	marksLook: true,
	status: (feed) => {
		const n = inboxCount(feed);
		if (n === null) return [];
		const s = feed.board?.summary;
		return [{
			n,
			word: COUNT_WORD,
			icon: "inbox",
			tone: "wait",
			end: true
		}, ...s?.asks_waiting ? [{
			n: s.asks_waiting,
			word: "asks that outlived their sessions",
			icon: "question",
			tone: "wait",
			end: true
		}] : []];
	},
	count: inboxCount,
	countWord: COUNT_WORD,
	link: "ask",
	linkFocus: (id) => `ask=${id}`,
	select: (feed, focus) => {
		const b = feed.inbox;
		const items = b?.items ?? [];
		const { item, wanted, project } = address(focus ?? "", items);
		const watching = feed.board?.watching ?? null;
		return {
			items,
			folded: b?.folded ?? [],
			inhibited: b?.inhibited ?? [],
			close: b?.close ?? null,
			held: b?.held ?? null,
			landed: b?.landed ?? null,
			snoozed: b?.snoozed ?? 0,
			snoozed_rows: b?.snoozed_rows ?? [],
			project,
			item,
			wanted,
			watching,
			blind: unseen(feed.board),
			...phase(feed)
		};
	},
	tab: (feed, focus) => {
		const items = feed.inbox?.items ?? [];
		const { item, project } = address(focus, items);
		return items.find((i) => i.id === item)?.title ?? (project ? `Inbox: ${project}` : "Inbox");
	},
	side: () => __vitePreload(() => import("./List2.js"), __vite__mapDeps([38,1,39,2,10,11,40,41])),
	page: () => __vitePreload(() => import("./Inbox.js"), __vite__mapDeps([42,1,4,5,6,7,8,2,9,10,11,12,13,14,15,24,39,40,43]))
});
//#endregion
//#region src/surfaces/new/index.ts
onAction("new-change", () => {
	if (location.hash.startsWith("#new")) return true;
	try {
		sessionStorage.setItem("vp-before-new", location.hash);
	} catch {}
	go("#new");
	return true;
});
onAction("leave", (surface) => {
	if (surface !== "new") return false;
	let back = "";
	try {
		back = sessionStorage.getItem("vp-before-new") ?? "";
	} catch {}
	go(back);
	return true;
});
register({
	id: "new",
	title: "New change",
	heading: "New change",
	band: "steering",
	order: 0,
	nav: false,
	transient: true,
	reads: [
		"/api/projects",
		"/api/agents",
		"/api/specs",
		"/api/changes/preflight"
	],
	select: () => ({ changeSurface: surfaces().find((s) => s.link === "change")?.id ?? "" }),
	page: () => __vitePreload(() => import("./New.js"), __vite__mapDeps([44,1,2,5,6,7,45]))
});
//#endregion
//#region src/surfaces/palette/index.ts
var from = "";
onAction("open-palette", () => {
	if (location.hash.startsWith("#palette")) return true;
	from = location.hash;
	go("#palette");
	return true;
});
onAction("leave", (surface) => {
	if (surface !== "palette") return false;
	go(from || "");
	from = "";
	return true;
});
register({
	id: "palette",
	title: "Palette",
	heading: "Everything, by name",
	band: "happening",
	order: 9,
	nav: false,
	transient: true,
	reads: ["/api/changes", "/api/projects"],
	select: () => ({ from }),
	page: () => __vitePreload(() => import("./Palette.js"), __vite__mapDeps([46,1,2,6,7,24,27,13,14,28,47]))
});
//#endregion
//#region src/surfaces/plan/index.ts
bind({
	surface: "global",
	combo: "g s",
	action: "go-specs",
	label: "go to the specifications"
});
bindList("plan", "open the specification as a tab of its own");
onAction("go-specs", () => {
	go("#plan");
	return true;
});
register({
	id: "plan",
	icon: "spec",
	title: "Specifications",
	heading: "Specifications",
	band: "happening",
	order: 3,
	reads: ["/api/specs"],
	tab: (_feed, focus) => focus.split("/").filter(Boolean).pop() ?? "Specification",
	select: () => ({}),
	side: () => __vitePreload(() => import("./List3.js"), __vite__mapDeps([48,1,4,27,13,14,28,49,50])),
	document: true,
	page: () => __vitePreload(() => import("./Doc2.js"), __vite__mapDeps([51,1,20,21,27,13,14,28,49,52]))
});
//#endregion
//#region src/surfaces/quit/index.ts
register({
	id: "quit",
	title: "Quit",
	heading: "Quit Devplane?",
	band: "project",
	order: 9,
	nav: false,
	bare: true,
	reads: ["/api/quitting"],
	select: () => ({}),
	page: () => __vitePreload(() => import("./Quit.js"), __vite__mapDeps([53,1,2,3,54]))
});
//#endregion
//#region src/surfaces/search/index.ts
bindList("search", "open the session the match is from");
register({
	id: "search",
	icon: "search",
	title: "Search",
	heading: "Search every session",
	band: "happening",
	order: 2,
	nav: false,
	takesQuery: true,
	reads: ["/api/search"],
	select: (feed, focus) => ({
		q: focus,
		blind: unseen(feed.board)
	}),
	page: () => __vitePreload(() => import("./Search.js"), __vite__mapDeps([55,1,2,4,18,19,20,21,56]))
});
//#endregion
//#region src/surfaces/setup/checklist.ts
var KEY$2 = "first-run:skipped";
function skippedSteps() {
	try {
		const v = JSON.parse(localStorage.getItem(KEY$2) ?? "[]");
		return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
	} catch {
		return [];
	}
}
function keepSkipped(ids) {
	try {
		localStorage.setItem(KEY$2, JSON.stringify(ids));
	} catch {}
}
function incomplete(steps, skipped) {
	return steps.some((s) => s.state !== "done" && !skipped.includes(s.id));
}
var TITLES = {
	sessions: "Sessions already running",
	updates: "Update checks",
	connect: "Connect an agent",
	trust: "Trust a repository",
	github: "Sign in to GitHub",
	declare: "Declare a check",
	dialect: "Spec dialect",
	first_change: "A first verified change"
};
//#endregion
//#region src/surfaces/setup/index.ts
bindList("setup", "open the inbox narrowed to the project");
bind({
	surface: "global",
	combo: "g ,",
	action: "go-setup",
	label: "go to the setup"
});
onAction("go-setup", () => {
	go("#setup");
	return true;
});
register({
	id: "setup",
	icon: "settings",
	title: "Setup",
	heading: "Setup",
	band: "project",
	order: 0,
	holds: "gates",
	reads: [
		"/api/setup",
		"/api/github",
		"/api/first-run",
		"/api/projects",
		"/api/projects/scan",
		"/api/projects/trust"
	],
	opensFirst: async () => {
		try {
			const r = await api("/api/first-run");
			return Array.isArray(r?.steps) && incomplete(r.steps, skippedSteps());
		} catch {
			return false;
		}
	},
	select: () => ({ changeSurface: surfaces().find((s) => s.link === "change")?.id ?? "" }),
	page: () => __vitePreload(() => import("./Setup.js"), __vite__mapDeps([57,1,2,4,6,7,11,18,19,22,23,58]))
});
//#endregion
//#region src/surfaces/why/index.ts
bind({
	surface: "global",
	combo: "g l",
	action: "go-ledger",
	label: "go to the ledger"
});
bindList("why", "open the change the decision is about");
onAction("go-ledger", () => {
	go("#why");
	return true;
});
register({
	id: "why",
	icon: "ledger",
	title: "Ledger",
	heading: "Ledger",
	band: "happening",
	order: 4,
	link: "run",
	select: (feed, focus) => ({
		about: focus,
		blind: unseen(feed.board)
	}),
	page: () => __vitePreload(() => import("./Why.js"), __vite__mapDeps([59,1,2,4,6,7,18,19,20,21,33,34,60]))
});
//#endregion
//#region src/lib/live.svelte.ts
var EVERY_MS = 2e3;
var unread = proxy({ failed: false });
var poke = null;
function reread() {
	poke?.();
}
function live(looking) {
	const state = proxy({
		board: null,
		inbox: null,
		loaded: false,
		stale_since: null,
		error: null,
		unauthorised: false
	});
	let inflight = false;
	let queued = null;
	let lastGood = null;
	let timer = null;
	async function tick(read) {
		if (state.unauthorised) return;
		if (inflight) {
			queued = (queued ?? false) || read;
			return;
		}
		inflight = true;
		try {
			const [board, inbox] = await Promise.all([api("/api/board"), api(read ? "/api/inbox?read=true" : "/api/inbox")]);
			if (!same$1(snapshot(state.board), board)) state.board = board;
			if (!same$1(snapshot(state.inbox), inbox)) state.inbox = inbox;
			state.loaded = true;
			unread.failed = false;
			state.error = null;
			state.stale_since = null;
			lastGood = (/* @__PURE__ */ new Date()).toISOString();
		} catch (e) {
			if (e instanceof Unauthorised) {
				state.unauthorised = true;
				state.error = "This tab has no token. Run `devplane up` again for a fresh link.";
				unread.failed = !state.loaded;
				if (timer !== null) clearInterval(timer);
				timer = null;
				return;
			}
			state.error = e instanceof Error ? e.message : String(e);
			state.stale_since = lastGood;
			if (!state.loaded) unread.failed = true;
		} finally {
			inflight = false;
			if (queued !== null && !state.unauthorised) {
				const again = queued;
				queued = null;
				tick(again);
			}
		}
	}
	function look() {
		tick(looking());
	}
	poke = () => void tick(false);
	function start() {
		look();
		timer = setInterval(() => {
			if (!document.hidden) tick(false);
		}, EVERY_MS);
		const onVisible = () => {
			if (document.visibilityState === "visible") look();
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => {
			if (timer !== null) clearInterval(timer);
			timer = null;
			document.removeEventListener("visibilitychange", onVisible);
		};
	}
	return {
		state,
		start,
		look
	};
}
//#endregion
//#region src/lib/theme.svelte.ts
var KEY$1 = "devplane_theme";
function read() {
	try {
		const v = localStorage.getItem(KEY$1);
		return v === "light" || v === "dark" ? v : "system";
	} catch {
		return "system";
	}
}
function theme() {
	const state = proxy({ choice: read() });
	function apply(next) {
		state.choice = next;
		if (next === "system") document.documentElement.removeAttribute("data-theme");
		else document.documentElement.setAttribute("data-theme", next);
		try {
			if (next === "system") localStorage.removeItem(KEY$1);
			else localStorage.setItem(KEY$1, next);
		} catch {}
	}
	return {
		state,
		restore: () => apply(state.choice),
		cycle: () => apply(state.choice === "system" ? "light" : state.choice === "light" ? "dark" : "system")
	};
}
//#endregion
//#region src/lib/text.ts
function clip(value, max) {
	const chars = [...value];
	return chars.length <= max ? value : chars.slice(0, Math.max(0, max - 1)).join("") + "…";
}
function ago(seconds) {
	if (!Number.isFinite(seconds)) return "";
	const s = Math.max(0, Math.floor(seconds));
	if (s < 60) return `${s}s`;
	if (s < 3600) return `${Math.floor(s / 60)}m`;
	if (s < 86400) return `${Math.floor(s / 3600)}h`;
	return `${Math.floor(s / 86400)}d`;
}
function plural(n, one, many) {
	return n === 1 ? one : many;
}
function shape(says) {
	if (!says) return "";
	return says.split(" · ").map((part) => {
		const lines = /^(\d+)\+ (\d+)− lines$/.exec(part);
		if (lines) return `+${lines[1]} −${lines[2]} lines`;
		const role = /^(\d+) (shared|security)$/.exec(part);
		if (role) return role[1] === "0" ? "" : `${role[1]} ${role[2]} ${role[1] === "1" ? "file" : "files"}`;
		return part;
	}).filter(Boolean).join(" · ");
}
function listed(xs) {
	if (xs.length <= 1) return xs[0] ?? "";
	return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
}
//#endregion
//#region src/lib/md.ts
function inline(text) {
	if (!text) return [];
	const out = [];
	for (const p of text.split(/(`[^`\n]+`)/g)) {
		if (!p) continue;
		if (p.length > 2 && p.startsWith("`") && p.endsWith("`")) {
			out.push({
				k: "code",
				s: p.slice(1, -1)
			});
			continue;
		}
		for (const q of p.split(/(\*\*[^*\n]+\*\*)/g)) {
			if (!q) continue;
			if (q.length > 4 && q.startsWith("**") && q.endsWith("**")) out.push({
				k: "strong",
				s: q.slice(2, -2)
			});
			else out.push({
				k: "t",
				s: q
			});
		}
	}
	return out;
}
function blocks(text) {
	const out = [];
	let fence = null;
	let para = [];
	const flush = () => {
		if (para.length) out.push({
			kind: "p",
			segs: inline(para.join(" "))
		});
		para = [];
	};
	for (const line of (text ?? "").split("\n")) {
		if (line.trimStart().startsWith("```")) {
			if (fence) {
				out.push({
					kind: "pre",
					text: fence.join("\n")
				});
				fence = null;
			} else {
				flush();
				fence = [];
			}
			continue;
		}
		if (fence) {
			fence.push(line);
			continue;
		}
		const t = line.trim();
		if (!t) flush();
		else if (/^#{1,6}\s/.test(t)) {
			flush();
			out.push({
				kind: "h",
				segs: inline(t.replace(/^#+\s*/, ""))
			});
		} else if (/^[-*]\s/.test(t)) {
			flush();
			out.push({
				kind: "li",
				segs: inline(t.slice(2))
			});
		} else if (t.startsWith("|")) {
			flush();
			if (!/^\|[\s|:-]+\|$/.test(t)) {
				const cells = t.replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
				out.push({
					kind: "row",
					segs: inline(cells.join("  ·  "))
				});
			}
		} else para.push(t);
	}
	if (fence) out.push({
		kind: "pre",
		text: fence.join("\n")
	});
	flush();
	return out;
}
//#endregion
//#region src/lib/ui/Inline.svelte
var root$13 = from_html(`<code> </code>`);
var root_1$11 = from_html(`<strong> </strong>`);
function Inline($$anchor, $$props) {
	push($$props, true);
	let text$1 = prop($$props, "text", 3, null), segs = prop($$props, "segs", 3, null);
	const parts = user_derived(() => segs() ?? inline(text$1()));
	var fragment = comment();
	var node = first_child(fragment);
	each(node, 17, () => get(parts), index, ($$anchor, p) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		var consequent = ($$anchor) => {
			var code = root$13();
			var text_1 = only_child(code, true);
			template_effect(() => set_text(text_1, get(p).s));
			append($$anchor, code);
		};
		var consequent_1 = ($$anchor) => {
			var strong = root_1$11();
			var text_2 = only_child(strong, true);
			template_effect(() => set_text(text_2, get(p).s));
			append($$anchor, strong);
		};
		var alternate = ($$anchor) => {
			var text_3 = text();
			template_effect(() => set_text(text_3, get(p).s));
			append($$anchor, text_3);
		};
		if_block(node_1, ($$render) => {
			if (get(p).k === "code") $$render(consequent);
			else if (get(p).k === "strong") $$render(consequent_1, 1);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/lib/Stale.svelte
var root$12 = from_html(`<span class="age svelte-uf40wk"> </span>`);
var root_1$10 = from_html(`<span class="age svelte-uf40wk">· nothing has been read yet</span>`);
var root_2$5 = from_html(`<p class="stale svelte-uf40wk"><span role="status"><!></span> <!></p>`);
function Stale($$anchor, $$props) {
	push($$props, true);
	let stale_since = prop($$props, "stale_since", 3, null);
	let now = state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 1e3);
		return () => clearInterval(id);
	});
	const age = user_derived(() => stale_since() ? ago(Math.max(0, (get(now) - Date.parse(stale_since())) / 1e3)) : "");
	var p = root_2$5();
	var span = child(p);
	Inline(child(span), { get text() {
		return $$props.error;
	} });
	reset(span);
	var node_1 = sibling(span, 2);
	var consequent = ($$anchor) => {
		var span_1 = root$12();
		var text = only_child(span_1);
		template_effect(() => set_text(text, `· showing what was read ${get(age) ?? ""} ago`));
		append($$anchor, span_1);
	};
	var alternate = ($$anchor) => {
		var span_2 = root_1$10();
		append($$anchor, span_2);
	};
	if_block(node_1, ($$render) => {
		if (get(age)) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(p);
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/lib/Skeleton.svelte
var root$11 = from_html(`<p class="unread svelte-1xuy3ds" role="status">Not read — nothing here is current.</p>`);
var root_1$9 = from_html(`<div class="row svelte-1xuy3ds"><span class="bar svelte-1xuy3ds"></span></div>`);
var root_2$4 = from_html(`<div class="skeleton svelte-1xuy3ds" aria-busy="true" aria-label="reading"></div>`);
function Skeleton($$anchor, $$props) {
	push($$props, true);
	let rows = prop($$props, "rows", 3, 3), failed = prop($$props, "failed", 3, false);
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var p = root$11();
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var div = root_2$4();
		each(div, 21, () => ({ length: rows() }), index, ($$anchor, _, i) => {
			var div_1 = root_1$9();
			var span = child(div_1);
			set_style(span, "", {}, { width: `${40 + i * 23 % 40}%` });
			reset(div_1);
			append($$anchor, div_1);
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (failed() || unread.failed) $$render(consequent);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/lib/Lazy.svelte
var loaded = /* @__PURE__ */ new Map();
var rest_excludes = /* @__PURE__ */ new Set([
	"$$slots",
	"$$events",
	"$$legacy",
	"load",
	"component"
]);
var root$10 = from_html(`<p class="failed svelte-1gb5hex" role="alert"> </p>`);
var root_1$8 = from_html(`<div class="wait svelte-1gb5hex"><!></div>`);
function Lazy($$anchor, $$props) {
	push($$props, true);
	let load = prop($$props, "load", 3, void 0), component$1 = prop($$props, "component", 3, void 0), rest = rest_props($$props, rest_excludes);
	let fetched = state(proxy(untrack(() => load() ? loaded.get(load()) ?? null : null)));
	let failed = state("");
	user_effect(() => {
		const want = load();
		set(failed, "");
		if (!want) return;
		const have = loaded.get(want);
		if (have) {
			set(fetched, have, true);
			return;
		}
		set(fetched, null);
		want().then((m) => {
			loaded.set(want, m.default);
			if (load() === want) set(fetched, m.default, true);
		}).catch((e) => {
			if (load() === want) set(failed, e instanceof Error ? e.message : String(e), true);
		});
	});
	const Page = user_derived(() => component$1() ?? get(fetched));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		component(node_1, () => get(Page), ($$anchor, Page_1) => {
			Page_1($$anchor, spread_props(() => rest));
		});
		append($$anchor, fragment_1);
	};
	var consequent_1 = ($$anchor) => {
		var p = root$10();
		var text = only_child(p);
		template_effect(() => set_text(text, `This page did not load: ${get(failed) ?? ""}. Reload the window.`));
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var div = root_1$8();
		Skeleton(child(div), {});
		reset(div);
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (get(Page)) $$render(consequent);
		else if (get(failed)) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/lib/trap.ts
var FOCUSABLE = "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex=\"-1\"])";
function trap(node) {
	const before = document.activeElement;
	const inside = () => [...node.querySelectorAll(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
	queueMicrotask(() => {
		if (node.contains(document.activeElement)) return;
		const first = inside()[0];
		if (first) first.focus();
		else {
			if (!node.hasAttribute("tabindex")) node.setAttribute("tabindex", "-1");
			node.focus();
		}
	});
	const onKey = (e) => {
		if (e.key !== "Tab") return;
		const all = inside();
		if (all.length === 0) {
			e.preventDefault();
			return;
		}
		const first = all[0];
		const last = all[all.length - 1];
		const at = document.activeElement;
		if (e.shiftKey && (at === first || !node.contains(at))) {
			e.preventDefault();
			last.focus();
		} else if (!e.shiftKey && (at === last || !node.contains(at))) {
			e.preventDefault();
			first.focus();
		}
	};
	node.addEventListener("keydown", onKey);
	return { destroy() {
		node.removeEventListener("keydown", onKey);
		if (before && before.isConnected && typeof before.focus === "function") queueMicrotask(() => before.focus());
	} };
}
//#endregion
//#region src/lib/ui/Split.svelte
var root$9 = from_html(`<div class="pane svelte-l5jdby"><!></div>  <div class="handle svelte-l5jdby" role="separator" tabindex="0" aria-label="resize"></div>`, 1);
var root_1$7 = from_html(`<div><!> <div class="rest svelte-l5jdby"><!></div></div>`);
function Split($$anchor, $$props) {
	let axis = prop($$props, "axis", 3, "x"), initial = prop($$props, "size", 3, 280), min = prop($$props, "min", 3, 160), max = prop($$props, "max", 3, 640), side = prop($$props, "side", 3, "start"), collapsed = prop($$props, "collapsed", 3, false);
	const store = () => `vp-split:${$$props.id}`;
	function remembered() {
		try {
			const v = Number(localStorage.getItem(store()));
			return Number.isFinite(v) && v >= min() && v <= max() ? v : initial();
		} catch {
			return initial();
		}
	}
	let size = state(proxy(remembered()));
	let dragging = state(false);
	function keep() {
		try {
			localStorage.setItem(store(), String(Math.round(get(size))));
		} catch {}
	}
	let start = 0;
	let from = 0;
	function down(e) {
		set(dragging, true);
		start = axis() === "x" ? e.clientX : e.clientY;
		from = get(size);
		e.currentTarget.setPointerCapture(e.pointerId);
	}
	function move(e) {
		if (!get(dragging)) return;
		const delta = (axis() === "x" ? e.clientX : e.clientY) - start;
		set(size, Math.min(max(), Math.max(min(), from + (side() === "start" ? delta : -delta))), true);
	}
	function up() {
		if (!get(dragging)) return;
		set(dragging, false);
		keep();
	}
	function key(e) {
		const i = (axis() === "x" ? ["ArrowRight", "ArrowLeft"] : ["ArrowDown", "ArrowUp"]).indexOf(e.key);
		if (i === -1) return;
		e.preventDefault();
		const step = e.shiftKey ? 48 : 16;
		const sign = (i === 0 ? 1 : -1) * (side() === "start" ? 1 : -1);
		set(size, Math.min(max(), Math.max(min(), get(size) + sign * step)), true);
		keep();
	}
	var div = root_1$7();
	let classes;
	var node = child(div);
	var consequent = ($$anchor) => {
		var fragment = root$9();
		var div_1 = first_child(fragment);
		var node_1 = child(div_1);
		snippet(node_1, () => $$props.pane);
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		template_effect(($0) => {
			set_style(div_1, `${axis() === "x" ? "width" : "height"}: ${get(size) ?? ""}px`);
			set_attribute(div_2, "aria-orientation", axis() === "x" ? "vertical" : "horizontal");
			set_attribute(div_2, "aria-valuenow", $0);
			set_attribute(div_2, "aria-valuemin", min());
			set_attribute(div_2, "aria-valuemax", max());
		}, [() => Math.round(get(size))]);
		delegated("pointerdown", div_2, down);
		delegated("pointermove", div_2, move);
		delegated("pointerup", div_2, up);
		event("pointercancel", div_2, up);
		delegated("keydown", div_2, key);
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (!collapsed()) $$render(consequent);
	});
	var div_3 = sibling(node, 2);
	var node_2 = child(div_3);
	snippet(node_2, () => $$props.children);
	reset(div_3);
	reset(div);
	template_effect(() => classes = set_class(div, 1, `split ${axis() ?? ""}`, "svelte-l5jdby", classes, {
		dragging: get(dragging),
		end: side() === "end"
	}));
	append($$anchor, div);
}
delegate([
	"pointerdown",
	"pointermove",
	"pointerup",
	"keydown"
]);
//#endregion
//#region src/lib/ui/Icon.svelte
var PATHS = {
	inbox: "M3 13h5l1.5 3h5L16 13h5M5.5 5h13L21 13v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6z",
	change: "M6 3v12M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 9a9 9 0 0 1-9 9",
	sessions: "M3 12h4l3-8 4 16 3-8h4",
	spec: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5M9 13h6M9 17h6M9 9h2",
	ledger: "M12 3v18M5 7h14M5 7l-2.5 6a3 3 0 0 0 5 0zM19 7l-2.5 6a3 3 0 0 0 5 0zM8 21h8",
	forge: "M6 3v12M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 15V9a3 3 0 0 0-3-3h-4M13 3l-2 3 2 3",
	settings: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3.9 14H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
	search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3",
	plus: "M12 5v14M5 12h14",
	play: "M7 4l13 8-13 8z",
	stop: "M6 6h12v12H6z",
	check: "M4 12.5l5 5L20 6.5",
	decide: "M12 3v18M12 6h7l2 2.5-2 2.5h-7M12 13H5l-2 2.5L5 18h7",
	x: "M6 6l12 12M18 6L6 18",
	alert: "M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
	question: "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3M12 17h.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z",
	clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
	right: "M9 6l6 6-6 6",
	down: "M6 9l6 6 6-6",
	left: "M15 6l-6 6 6 6",
	dot: "M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
	sidebar: "M4 4h16v16H4zM9 4v16",
	panel: "M4 4h16v16H4zM4 15h16",
	split: "M4 4h16v16H4zM12 4v16",
	unified: "M4 4h16v16H4zM4 12h16",
	filter: "M3 5h18l-7 8v6l-4 2v-8z",
	refresh: "M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5",
	external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
	terminal: "M4 17l6-5-6-5M12 19h8",
	file: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5",
	folder: "M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
	person: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0",
	agent: "M12 8V4M8 4h8M6 8h12a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2zM9 13v1M15 13v1",
	shield: "M12 21s8-4 8-10V5l-8-3-8 3v6c0 6 8 10 8 10z",
	rule: "M4 6h16M4 12h10M4 18h7",
	gate: "M4 21V8l8-5 8 5v13M9 21v-6h6v6",
	timeline: "M3 12h18M7 8v8M12 5v14M17 9v6",
	keyboard: "M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10",
	sun: "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4",
	moon: "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z",
	more: "M5 12h.01M12 12h.01M19 12h.01",
	pin: "M12 17v5M9 3h6l-1 6 3 3H7l3-3z",
	eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
	skip: "M5 4l10 8-10 8zM19 5v14"
};
var root$8 = from_svg(`<svg class="icon svelte-jf3o41" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"><path></path></svg>`);
function Icon($$anchor, $$props) {
	let size = prop($$props, "size", 3, 16), label = prop($$props, "label", 3, ""), stroke = prop($$props, "stroke", 3, 1.75);
	const d = user_derived(() => PATHS[$$props.name] ?? PATHS.dot);
	var svg = root$8();
	var path = only_child(svg);
	template_effect(() => {
		set_attribute(svg, "width", size());
		set_attribute(svg, "height", size());
		set_attribute(svg, "stroke-width", stroke());
		set_attribute(svg, "role", label() ? "img" : void 0);
		set_attribute(svg, "aria-label", label() || void 0);
		set_attribute(svg, "aria-hidden", label() ? void 0 : "true");
		set_attribute(path, "d", get(d));
	});
	append($$anchor, svg);
}
//#endregion
//#region src/shell/TitleBar.svelte
var root$7 = from_html(`<span><!><span class="word svelte-whzgc7"> </span></span>`);
var root_1$6 = from_html(`<header class="title svelte-whzgc7"><div class="brand svelte-whzgc7"><svg class="mark svelte-whzgc7" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><rect x="2" y="2" width="20" height="20" rx="5" fill="var(--accent)"></rect><path d="M8 7h4.5a5 5 0 0 1 0 10H8z" fill="none" stroke="var(--chrome)" stroke-width="2.2"></path></svg> <b class="name svelte-whzgc7">Devplane</b> <span class="crumbs svelte-whzgc7"></span></div> <button class="find svelte-whzgc7" aria-label="find anything — changes, sessions, commands"><!> <span class="svelte-whzgc7">Find a change, a session, a command…</span> <kbd class="svelte-whzgc7"> </kbd></button> <div class="acts svelte-whzgc7"><button class="primary svelte-whzgc7" aria-label="New change"><!> <span class="label svelte-whzgc7">New change</span></button> <button aria-label="toggle the sidebar"><!></button> <button aria-label="toggle the activity panel"><!></button></div></header>`);
function TitleBar($$anchor, $$props) {
	push($$props, true);
	var header = root_1$6();
	var div = child(header);
	var span = sibling(child(div), 4);
	each(span, 21, () => $$props.crumbs, index, ($$anchor, c, i) => {
		var span_1 = root$7();
		let classes;
		let styles;
		var node = child(span_1);
		Icon(node, {
			name: "right",
			size: 12
		});
		var span_2 = sibling(node);
		var text = only_child(span_2, true);
		reset(span_1);
		template_effect(() => {
			classes = set_class(span_1, 1, "crumb svelte-whzgc7", null, classes, { last: i === $$props.crumbs.length - 1 });
			styles = set_style(span_1, "", styles, { order: $$props.crumbs.length - 1 - i });
			set_text(text, get(c));
		});
		append($$anchor, span_1);
	});
	reset(span);
	reset(div);
	var button = sibling(div, 2);
	var node_1 = child(button);
	Icon(node_1, {
		name: "search",
		size: 14
	});
	var kbd = sibling(node_1, 4);
	var text_1 = only_child(kbd, true);
	reset(button);
	var div_1 = sibling(button, 2);
	var button_1 = child(div_1);
	Icon(child(button_1), {
		name: "plus",
		size: 14
	});
	next(2);
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	let classes_1;
	Icon(child(button_2), {
		name: "sidebar",
		size: 16
	});
	reset(button_2);
	var button_3 = sibling(button_2, 2);
	let classes_2;
	Icon(child(button_3), {
		name: "panel",
		size: 16
	});
	reset(button_3);
	reset(div_1);
	reset(header);
	template_effect(($0, $1, $2, $3, $4) => {
		set_attribute(div, "title", $0);
		set_text(text_1, $1);
		set_attribute(button_1, "title", `Start a new change (${$2 ?? ""})`);
		classes_1 = set_class(button_2, 1, "icon svelte-whzgc7", null, classes_1, { on: $$props.sideOpen });
		set_attribute(button_2, "title", `Sidebar (${$3 ?? ""})`);
		set_attribute(button_2, "aria-pressed", $$props.sideOpen);
		classes_2 = set_class(button_3, 1, "icon svelte-whzgc7", null, classes_2, { on: $$props.panelOpen });
		set_attribute(button_3, "title", `Activity panel (${$4 ?? ""})`);
		set_attribute(button_3, "aria-pressed", $$props.panelOpen);
	}, [
		() => $$props.crumbs.join(" › "),
		() => spell("Mod+k"),
		() => spell("Alt+n"),
		() => spell("Mod+b"),
		() => spell("Mod+j")
	]);
	delegated("click", button, function(...$$args) {
		$$props.find?.apply(this, $$args);
	});
	delegated("click", button_1, function(...$$args) {
		$$props.create?.apply(this, $$args);
	});
	delegated("click", button_2, function(...$$args) {
		$$props.toggleSide?.apply(this, $$args);
	});
	delegated("click", button_3, function(...$$args) {
		$$props.togglePanel?.apply(this, $$args);
	});
	append($$anchor, header);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/ActivityBar.svelte
var root$6 = from_html(`<b> </b>`);
var root_1$5 = from_html(`<span> </span>`);
var root_2$3 = from_html(`<button><!> <!></button>`);
var root_3$2 = from_html(`<div class="band svelte-w2jox2" role="group"></div>`);
var root_4$2 = from_html(`<nav class="bar svelte-w2jox2" aria-label="surfaces"><!> <span class="gap svelte-w2jox2"></span> <!></nav>`);
function ActivityBar($$anchor, $$props) {
	push($$props, true);
	const item = ($$anchor, s = noop) => {
		const n = user_derived(() => s().count?.($$props.feed) ?? null);
		var button = root_2$3();
		let classes;
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return s().icon;
				},
				size: 20,
				stroke: 1.6
			});
		};
		var alternate = ($$anchor) => {
			var b_1 = root$6();
			var text = only_child(b_1, true);
			template_effect(($0) => set_text(text, $0), [() => s().title.slice(0, 1)]);
			append($$anchor, b_1);
		};
		if_block(node, ($$render) => {
			if (s().icon) $$render(consequent);
			else $$render(alternate, -1);
		});
		var node_1 = sibling(node, 2);
		var consequent_1 = ($$anchor) => {
			var span = root_1$5();
			let classes_1;
			var text_1 = only_child(span, true);
			template_effect(() => {
				classes_1 = set_class(span, 1, "badge svelte-w2jox2", null, classes_1, { hot: s().band === "attention" });
				set_text(text_1, get(n) > 99 ? "99+" : get(n));
			});
			append($$anchor, span);
		};
		if_block(node_1, ($$render) => {
			if (get(n)) $$render(consequent_1);
		});
		reset(button);
		template_effect(() => {
			classes = set_class(button, 1, "act svelte-w2jox2", null, classes, { on: s().id === $$props.current });
			set_attribute(button, "title", get(n) ? `${s().title} — ${get(n)}${s().countWord ? ` ${s().countWord}` : ""}` : s().title);
			set_attribute(button, "aria-label", get(n) ? `${s().title}, ${get(n)}${s().countWord ? ` ${s().countWord}` : ""}` : s().title);
			set_attribute(button, "aria-current", s().id === $$props.current ? "page" : void 0);
		});
		delegated("click", button, () => $$props.pick(s().id));
		append($$anchor, button);
	};
	const group = ($$anchor, g = noop) => {
		var div = root_3$2();
		each(div, 21, () => g().items, (s) => s.id, ($$anchor, s) => {
			item($$anchor, () => get(s));
		});
		reset(div);
		template_effect(() => set_attribute(div, "aria-label", BAND_LABELS[g().band]));
		append($$anchor, div);
	};
	const bands = user_derived(() => BANDS.map((b) => ({
		band: b,
		items: $$props.items.filter((s) => s.band === b)
	})).filter((g) => g.items.length > 0));
	const top = user_derived(() => get(bands).slice(0, -1));
	const bottom = user_derived(() => get(bands).slice(-1));
	var nav = root_4$2();
	var node_2 = child(nav);
	each(node_2, 17, () => get(top), (g) => g.band, ($$anchor, g) => {
		group($$anchor, () => get(g));
	});
	var node_3 = sibling(node_2, 4);
	each(node_3, 17, () => get(bottom), (g) => g.band, ($$anchor, g) => {
		group($$anchor, () => get(g));
	});
	reset(nav);
	append($$anchor, nav);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/EditorTabs.svelte
var root$5 = from_html(`<div role="tab"><!> <span class="name svelte-1h63k4l"> </span> <button class="x svelte-1h63k4l"><!></button></div>`);
var root_1$4 = from_html(`<div class="strip svelte-1h63k4l" role="tablist" aria-label="open tabs"></div>`);
function EditorTabs($$anchor, $$props) {
	push($$props, true);
	var div = root_1$4();
	each(div, 23, () => $$props.list, (t) => t.surface + "/" + t.focus, ($$anchor, t, i) => {
		var div_1 = root$5();
		let classes;
		var node = child(div_1);
		var consequent = ($$anchor) => {
			{
				let $0 = user_derived(() => $$props.icon(get(t)));
				Icon($$anchor, {
					get name() {
						return get($0);
					},
					size: 14
				});
			}
		};
		var d = user_derived(() => $$props.icon(get(t)));
		if_block(node, ($$render) => {
			if (get(d)) $$render(consequent);
		});
		var span = sibling(node, 2);
		var text = only_child(span, true);
		var button = sibling(span, 2);
		Icon(child(button), {
			name: "x",
			size: 12
		});
		reset(button);
		reset(div_1);
		template_effect(($0, $1, $2) => {
			classes = set_class(div_1, 1, "tab svelte-1h63k4l", null, classes, {
				on: get(i) === $$props.active,
				preview: !get(t).pinned && !!get(t).focus
			});
			set_attribute(div_1, "tabindex", get(i) === $$props.active ? 0 : -1);
			set_attribute(div_1, "aria-selected", get(i) === $$props.active);
			set_attribute(div_1, "title", $0);
			set_text(text, $1);
			set_attribute(button, "aria-label", `close ${$2 ?? ""}`);
		}, [
			() => $$props.title(get(t)),
			() => $$props.title(get(t)),
			() => $$props.title(get(t))
		]);
		delegated("click", div_1, () => $$props.pick(get(i)));
		delegated("dblclick", div_1, () => $$props.pin(get(i)));
		event("auxclick", div_1, (e) => e.button === 1 && $$props.close(get(i)));
		delegated("keydown", div_1, (e) => e.key === "Enter" && $$props.pick(get(i)));
		delegated("click", button, (e) => {
			e.stopPropagation();
			$$props.close(get(i));
		});
		append($$anchor, div_1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate([
	"click",
	"dblclick",
	"keydown"
]);
//#endregion
//#region src/shell/StatusBar.svelte
var root$4 = from_html(`<button><!> </button>`);
var root_1$3 = from_html(`<span class="item svelte-1xut7e1"> </span>`);
var root_2$2 = from_html(`<footer class="status svelte-1xut7e1" aria-label="status"><span role="status"><span class="dot svelte-1xut7e1"></span> </span> <!> <!> <span class="gap svelte-1xut7e1"></span> <!> <button class="item svelte-1xut7e1" title="theme"><!> </button> <button class="item svelte-1xut7e1" aria-label="keys bound here"><!></button></footer>`);
function StatusBar($$anchor, $$props) {
	push($$props, true);
	const fact = ($$anchor, i = noop) => {
		var button = root$4();
		let classes;
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return i().icon;
				},
				size: 13
			});
		};
		if_block(node, ($$render) => {
			if (i().icon) $$render(consequent);
		});
		var text = sibling(node);
		reset(button);
		template_effect(() => {
			classes = set_class(button, 1, `item ${i().tone ?? "" ?? ""}`, "svelte-1xut7e1", classes, { hot: i().tone === "wait" && i().n > 0 });
			set_attribute(button, "title", `open ${i().title ?? ""}`);
			set_text(text, `${i().n ?? ""} ${i().word ?? ""}`);
		});
		delegated("click", button, () => $$props.go(`#${i().surface}`));
		append($$anchor, button);
	};
	let projects = prop($$props, "projects", 3, null);
	var footer = root_2$2();
	var span = child(footer);
	let classes_1;
	var text_1 = sibling(child(span), 1, true);
	reset(span);
	var node_1 = sibling(span, 2);
	each(node_1, 17, () => $$props.items.filter((i) => !i.end), (i) => i.surface + i.word, ($$anchor, i) => {
		fact($$anchor, () => get(i));
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1$3();
		var text_2 = only_child(span_1);
		template_effect(() => set_text(text_2, `${projects() ?? ""} ${projects() === 1 ? "project" : "projects"}`));
		append($$anchor, span_1);
	};
	if_block(node_2, ($$render) => {
		if (projects() != null) $$render(consequent_1);
	});
	var node_3 = sibling(node_2, 4);
	each(node_3, 17, () => $$props.items.filter((i) => i.end), (i) => i.surface + i.word, ($$anchor, i) => {
		fact($$anchor, () => get(i));
	});
	var button_1 = sibling(node_3, 2);
	var node_4 = child(button_1);
	{
		let $0 = user_derived(() => $$props.theme === "light" ? "sun" : "moon");
		Icon(node_4, {
			get name() {
				return get($0);
			},
			size: 13
		});
	}
	var text_3 = sibling(node_4, 1, true);
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	Icon(child(button_2), {
		name: "keyboard",
		size: 13
	});
	reset(button_2);
	reset(footer);
	template_effect(() => {
		classes_1 = set_class(span, 1, "item pulse svelte-1xut7e1", null, classes_1, { bad: $$props.bad });
		set_text(text_1, $$props.pulse);
		set_text(text_3, $$props.theme);
	});
	delegated("click", button_1, function(...$$args) {
		$$props.cycleTheme?.apply(this, $$args);
	});
	delegated("click", button_2, function(...$$args) {
		$$props.help?.apply(this, $$args);
	});
	append($$anchor, footer);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/ui/Pill.svelte
var TONES = [
	["fail", /fail|stopped|broken|denied|deny|refused|error|lost|abandon/i],
	["wait", /need|wait|ask|question|permission|held|stale|drift|blocked|review/i],
	["done", /^(verified|passed)$/i],
	["work", /work|flight|running|isolated|driving|live|active|in progress/i]
];
function tone(word) {
	if (!word) return "none";
	for (const [t, re] of TONES) if (re.test(word)) return t;
	return "none";
}
var root$3 = from_html(`<span class="dot svelte-ueztnz" aria-hidden="true"></span>`);
var root_1$2 = from_html(`<span><!> </span>`);
function Pill($$anchor, $$props) {
	let as = prop($$props, "as", 3, void 0), dot = prop($$props, "dot", 3, true), title = prop($$props, "title", 3, void 0);
	const t = user_derived(() => as() ?? tone($$props.word));
	var span = root_1$2();
	var node = child(span);
	var consequent = ($$anchor) => {
		var span_1 = root$3();
		append($$anchor, span_1);
	};
	if_block(node, ($$render) => {
		if (dot()) $$render(consequent);
	});
	var text = sibling(node, 1, true);
	reset(span);
	template_effect(() => {
		set_class(span, 1, `pill ${get(t) ?? ""}`, "svelte-ueztnz");
		set_attribute(span, "title", title());
		set_text(text, $$props.word);
	});
	append($$anchor, span);
}
//#endregion
//#region src/shell/Panel.svelte
var root$2 = from_html(`<p class="quiet fail svelte-9fbpju"> </p>`);
var root_1$1 = from_html(`<p class="quiet svelte-9fbpju">No session has reported anything yet.</p>`);
var root_2$1 = from_html(`<span class="ctx svelte-9fbpju"> </span>`);
var root_3$1 = from_html(`<li><button class="svelte-9fbpju"><time class="svelte-9fbpju"> </time> <span class="proj svelte-9fbpju"> </span> <span class="agent svelte-9fbpju"> </span> <!> <span class="what svelte-9fbpju"> </span> <!></button></li>`);
var root_4$1 = from_html(`<ol class="log svelte-9fbpju"></ol>`);
var root_5$1 = from_html(`<dt class="svelte-9fbpju"><!> Projects whose configuration cannot be read</dt> <dd class="svelte-9fbpju"> </dd>`, 1);
var root_6$1 = from_html(`<dl class="sight svelte-9fbpju"><dt class="svelte-9fbpju"><!> Watched end to end</dt> <dd class="svelte-9fbpju"> </dd> <dt class="svelte-9fbpju"><!> Read, not yet proved against a live session</dt> <dd class="svelte-9fbpju"> </dd> <dt class="svelte-9fbpju"><!> Seen only when Devplane starts them</dt> <dd class="svelte-9fbpju"> </dd> <!></dl> <p class="quiet svelte-9fbpju">A session in a tool that is not watched here is not on the board — the board is the limit of this machine's sight, not a report that nothing is running.</p>`, 1);
var root_7$1 = from_html(`<section class="panel svelte-9fbpju" aria-label="activity panel"><div class="head svelte-9fbpju" role="tablist" aria-label="panel views"><button role="tab" class="svelte-9fbpju">Activity <span class="n svelte-9fbpju"> </span></button> <button role="tab" class="svelte-9fbpju">Sight <span class="n svelte-9fbpju"> </span></button></div> <div class="body svelte-9fbpju"><!></div></section>`);
function Panel($$anchor, $$props) {
	push($$props, true);
	let error = prop($$props, "error", 3, null);
	let view = state("activity");
	const runs = user_derived(() => [...$$props.board?.runs ?? []].sort((a, b) => (b.last_event_at ?? "").localeCompare(a.last_event_at ?? "")));
	const w = user_derived(() => $$props.board?.watching ?? {});
	function clock(at) {
		if (!at) return "--:--:--";
		const d = new Date(at);
		return Number.isNaN(d.getTime()) ? "--:--:--" : d.toTimeString().slice(0, 8);
	}
	var section = root_7$1();
	var div = child(section);
	var button = child(div);
	var span = sibling(child(button));
	var text = only_child(span, true);
	reset(button);
	var button_1 = sibling(button, 2);
	var span_1 = sibling(child(button_1));
	var text_1 = only_child(span_1, true);
	reset(button_1);
	reset(div);
	var div_1 = sibling(div, 2);
	var node = child(div_1);
	var consequent_4 = ($$anchor) => {
		var fragment = comment();
		var node_1 = first_child(fragment);
		var consequent = ($$anchor) => {
			var p = root$2();
			var text_2 = only_child(p);
			template_effect(() => set_text(text_2, `The sessions could not be read: ${error() ?? ""}`));
			append($$anchor, p);
		};
		var consequent_1 = ($$anchor) => {
			Skeleton($$anchor, { rows: 2 });
		};
		var consequent_2 = ($$anchor) => {
			var p_1 = root_1$1();
			append($$anchor, p_1);
		};
		var alternate = ($$anchor) => {
			var ol = root_4$1();
			each(ol, 21, () => get(runs), (r) => r.id, ($$anchor, r) => {
				var li = root_3$1();
				var button_2 = child(li);
				var time = child(button_2);
				var text_3 = only_child(time, true);
				var span_2 = sibling(time, 2);
				var text_4 = only_child(span_2, true);
				var span_3 = sibling(span_2, 2);
				var text_5 = only_child(span_3);
				var node_2 = sibling(span_3, 2);
				{
					let $0 = user_derived(() => get(r).state ?? "unknown");
					Pill(node_2, { get word() {
						return get($0);
					} });
				}
				var span_4 = sibling(node_2, 2);
				var text_6 = only_child(span_4, true);
				var node_3 = sibling(span_4, 2);
				var consequent_3 = ($$anchor) => {
					var span_5 = root_2$1();
					var text_7 = only_child(span_5);
					template_effect(($0) => set_text(text_7, `${$0 ?? ""}% context`), [() => Math.round(get(r).context_percent)]);
					append($$anchor, span_5);
				};
				if_block(node_3, ($$render) => {
					if (get(r).context_percent != null) $$render(consequent_3);
				});
				reset(button_2);
				reset(li);
				template_effect(($0) => {
					button_2.disabled = !$$props.open;
					set_text(text_3, $0);
					set_text(text_4, get(r).project_name ?? "—");
					set_text(text_5, `${get(r).agent ?? "agent" ?? ""}${get(r).mode === "driven" ? " · driven" : ""}`);
					set_text(text_6, get(r).summary ?? "");
				}, [() => clock(get(r).last_event_at)]);
				delegated("click", button_2, () => $$props.open?.(get(r).id));
				append($$anchor, li);
			});
			reset(ol);
			append($$anchor, ol);
		};
		if_block(node_1, ($$render) => {
			if (!$$props.board && error()) $$render(consequent);
			else if (!$$props.board) $$render(consequent_1, 1);
			else if (get(runs).length === 0) $$render(consequent_2, 2);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_2 = root_6$1();
		var dl = first_child(fragment_2);
		var dt = child(dl);
		Icon(child(dt), {
			name: "eye",
			size: 13
		});
		next();
		reset(dt);
		var dd = sibling(dt, 2);
		var text_8 = only_child(dd, true);
		var dt_1 = sibling(dd, 2);
		Icon(child(dt_1), {
			name: "alert",
			size: 13
		});
		next();
		reset(dt_1);
		var dd_1 = sibling(dt_1, 2);
		var text_9 = only_child(dd_1, true);
		var dt_2 = sibling(dd_1, 2);
		Icon(child(dt_2), {
			name: "agent",
			size: 13
		});
		next();
		reset(dt_2);
		var dd_2 = sibling(dt_2, 2);
		var text_10 = only_child(dd_2, true);
		var node_7 = sibling(dd_2, 2);
		var consequent_5 = ($$anchor) => {
			var fragment_3 = root_5$1();
			var dt_3 = first_child(fragment_3);
			Icon(child(dt_3), {
				name: "x",
				size: 13
			});
			next();
			reset(dt_3);
			var dd_3 = sibling(dt_3, 2);
			var text_11 = only_child(dd_3, true);
			template_effect(($0) => set_text(text_11, $0), [() => $$props.board.coverage.unreadable.join(", ")]);
			append($$anchor, fragment_3);
		};
		if_block(node_7, ($$render) => {
			if ($$props.board?.coverage?.unreadable?.length) $$render(consequent_5);
		});
		reset(dl);
		next(2);
		template_effect(($0, $1, $2) => {
			set_text(text_8, $0);
			set_text(text_9, $1);
			set_text(text_10, $2);
		}, [
			() => get(w).watched?.join(", ") || "none",
			() => get(w).unproved?.join(", ") || "none",
			() => get(w).driven_only?.join(", ") || "none"
		]);
		append($$anchor, fragment_2);
	};
	if_block(node, ($$render) => {
		if (get(view) === "activity") $$render(consequent_4);
		else $$render(alternate_1, -1);
	});
	reset(div_1);
	reset(section);
	template_effect(() => {
		set_attribute(button, "aria-selected", get(view) === "activity");
		set_text(text, $$props.board ? get(runs).length : "");
		set_attribute(button_1, "aria-selected", get(view) === "sight");
		set_text(text_1, $$props.board ? (get(w).unproved?.length ?? 0) + (get(w).driven_only?.length ?? 0) : "");
	});
	delegated("click", button, () => set(view, "activity"));
	delegated("click", button_1, () => set(view, "sight"));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/shell/tabs.svelte.ts
var KEY = "vp-tabs";
function load() {
	try {
		const v = JSON.parse(sessionStorage.getItem(KEY) ?? "[]");
		return Array.isArray(v) ? v.filter((t) => typeof t?.surface === "string") : [];
	} catch {
		return [];
	}
}
var tabs = proxy({
	list: load(),
	active: -1
});
function keep() {
	try {
		sessionStorage.setItem(KEY, JSON.stringify(tabs.list));
	} catch {}
}
var same = (t, surface, focus) => t.surface === surface && t.focus === focus;
function show(surface, focus, pin = false, view = "") {
	const at = tabs.list.findIndex((t) => same(t, surface, focus));
	if (at !== -1) {
		if (pin) tabs.list[at].pinned = true;
		tabs.list[at].view = view;
		tabs.active = at;
		keep();
		return;
	}
	const preview = tabs.list.findIndex((t) => !t.pinned);
	const tab = {
		surface,
		focus,
		pinned: pin,
		view
	};
	if (preview !== -1) {
		tabs.list[preview] = tab;
		tabs.active = preview;
	} else {
		tabs.list.push(tab);
		tabs.active = tabs.list.length - 1;
	}
	keep();
}
function pin() {
	const t = tabs.list[tabs.active];
	if (t && !t.pinned) {
		t.pinned = true;
		keep();
	}
}
function close(i) {
	if (i < 0 || i >= tabs.list.length) return null;
	tabs.list.splice(i, 1);
	if (tabs.list.length === 0) {
		tabs.active = -1;
		keep();
		return null;
	}
	if (tabs.active >= i) tabs.active = Math.max(0, tabs.active - 1);
	keep();
	return tabs.list[tabs.active];
}
function hashOf(t) {
	const v = t.focus && t.view ? `/${encodeURIComponent(t.view)}` : "";
	return `#${t.surface}${t.focus ? `/${encodeURIComponent(t.focus)}` : ""}${v}`;
}
//#endregion
//#region src/shell/keys.ts
bind({
	surface: "global",
	combo: "Mod+b",
	action: "toggle-side",
	label: "show or hide the sidebar"
});
bind({
	surface: "global",
	combo: "Mod+j",
	action: "toggle-panel",
	label: "show or hide the activity panel"
});
bind({
	surface: "global",
	combo: "Alt+w",
	action: "close-tab",
	label: "close the tab"
});
bind({
	surface: "global",
	combo: "Alt+]",
	action: "next-tab",
	label: "next tab"
});
bind({
	surface: "global",
	combo: "Alt+[",
	action: "prev-tab",
	label: "previous tab"
});
bind({
	surface: "global",
	combo: "Alt+n",
	action: "new-change",
	label: "start a new change"
});
//#endregion
//#region src/App.svelte
var root$1 = from_html(`<p class="problem svelte-1n46o8q" role="alert"><!></p>`);
var root_1 = from_html(`<main id="surface" class="bare svelte-1n46o8q"><!> <!></main>`);
var root_2 = from_html(`<aside class="side svelte-1n46o8q"><!></aside>`);
var root_3 = from_html(`<p class="notice svelte-1n46o8q" role="status"> </p>`);
var root_4 = from_html(`<p>No surface is registered.</p>`);
var root_5 = from_html(`<!> <main id="surface" class="editor svelte-1n46o8q" tabindex="-1"><!> <!> <!></main>`, 1);
var root_6 = from_html(`<div class="scrim svelte-1n46o8q" role="presentation"></div> <div class="overlay svelte-1n46o8q" role="dialog" aria-modal="true"><!></div>`, 1);
var root_7 = from_html(`<!><kbd class="svelte-1n46o8q"> </kbd>`, 1);
var root_8 = from_html(`<dt></dt> <dd class="svelte-1n46o8q"> </dd>`, 1);
var root_9 = from_html(`<h3 class="svelte-1n46o8q"> </h3> <dl class="svelte-1n46o8q"></dl>`, 1);
var root_10 = from_html(`<div class="scrim svelte-1n46o8q" role="presentation"></div> <div class="help svelte-1n46o8q" role="dialog" aria-modal="true" aria-label="keys bound here"><header class="svelte-1n46o8q"><!> <h2 class="svelte-1n46o8q">Keys bound here</h2></header> <!></div>`, 1);
var root_11 = from_html(`<button class="skip svelte-1n46o8q">Skip to content</button> <div class="wb svelte-1n46o8q"><!> <div class="mid svelte-1n46o8q"><!> <!></div> <!></div> <!> <!>`, 1);
function App($$anchor, $$props) {
	push($$props, true);
	let current = state(proxy(landing()?.id ?? ""));
	let focus = state("");
	let view = state("");
	let notice = state("");
	let helpOpen = state(false);
	const showing = user_derived(() => surfaces().find((s) => s.id === get(current)));
	const looking = () => get(showing)?.marksLook === true && document.visibilityState === "visible";
	const { state: feed, start, look } = live(looking);
	user_effect(() => untrack(start));
	user_effect(() => {
		if (get(showing)?.marksLook) untrack(look);
	});
	const { state: themeState, restore, cycle } = theme();
	user_effect(restore);
	function flag(key, dflt) {
		try {
			const v = localStorage.getItem(key);
			return v === null ? dflt : v === "1";
		} catch {
			return dflt;
		}
	}
	function keepFlag(key, v) {
		try {
			localStorage.setItem(key, v ? "1" : "0");
		} catch {}
	}
	let sideOpen = state(proxy(flag("vp-side", true)));
	let panelOpen = state(proxy(flag("vp-panel", false)));
	user_effect(() => keepFlag("vp-side", get(sideOpen)));
	user_effect(() => keepFlag("vp-panel", get(panelOpen)));
	const NARROW_PX = 1100;
	let wide = state(proxy(typeof innerWidth === "number" ? innerWidth >= NARROW_PX : true));
	user_effect(() => {
		const onResize = () => set(wide, innerWidth >= NARROW_PX);
		addEventListener("resize", onResize);
		return () => removeEventListener("resize", onResize);
	});
	let narrowSide = state(false);
	const folds = user_derived(() => {
		if (get(wide)) return false;
		const t = tabs.list[tabs.active];
		return !!t?.focus && !!surfaces().find((s) => s.id === t.surface)?.document;
	});
	const sideShown = user_derived(() => get(folds) ? get(narrowSide) : get(sideOpen));
	user_effect(() => {
		get(focus);
		untrack(() => set(narrowSide, false));
	});
	function toggleSide() {
		if (get(folds)) set(narrowSide, !get(narrowSide));
		else set(sideOpen, !get(sideOpen));
	}
	function read() {
		const raw = location.hash.slice(1);
		const cut = raw.indexOf("/");
		const id = cut === -1 ? raw : raw.slice(0, cut);
		if (!surfaces().some((s) => s.id === id)) {
			if (!raw) opening().then((s) => {
				const home = s?.id ?? landing()?.id;
				if (home && !location.hash.slice(1)) go(`#${home}`);
			});
			return;
		}
		set(current, id, true);
		const rest = cut === -1 ? "" : raw.slice(cut + 1);
		const vcut = rest.indexOf("/");
		set(focus, decodeURIComponent(vcut === -1 ? rest : rest.slice(0, vcut)), true);
		set(view, vcut === -1 ? "" : decodeURIComponent(rest.slice(vcut + 1)), true);
		set(helpOpen, false);
		const s = surfaces().find((x) => x.id === id);
		if (s && !s.transient && !s.bare) show(id, get(focus), false, get(view));
	}
	function showView(v) {
		if (!get(page) || !get(under).f) return;
		go(`#${get(page).id}/${encodeURIComponent(get(under).f)}${v ? `/${encodeURIComponent(v)}` : ""}`);
	}
	function readQuery() {
		claimToken();
		const url = new URL(location.href);
		const q = url.searchParams;
		if ([...q.keys()].length === 0) return;
		let id = "";
		let f = "";
		const want = q.get("surface");
		const named = !!want && surfaces().some((s) => s.id === want);
		if (named && want) id = want;
		for (const s of surfaces()) {
			const v = s.link ? q.get(s.link) : null;
			if (s.link && v) {
				if (named) {
					f ||= v;
					continue;
				}
				id = s.id;
				f = s.linkFocus ? s.linkFocus(v) : v;
			}
		}
		const link = q.get("link");
		if (link) {
			set(notice, `${link} names nothing this app opens; links are change, review, ask, run, inbox`);
			id = landing()?.id ?? "";
			f = "";
		}
		const hash = id ? `#${id}${f ? `/${encodeURIComponent(f)}` : ""}` : url.hash;
		history.replaceState({}, "", `${url.pathname}${hash}`);
	}
	user_effect(() => {
		readQuery();
		read();
		addEventListener("hashchange", read);
		return () => removeEventListener("hashchange", read);
	});
	function pick(id) {
		const last = [...tabs.list].reverse().find((t) => t.surface === id);
		if (id === get(current) && get(sideShown) && surfaces().find((s) => s.id === id)?.side) {
			toggleSide();
			return;
		}
		if (!get(sideShown)) toggleSide();
		go(last ? hashOf(last) : `#${id}`);
	}
	function pickTab(i) {
		const t = tabs.list[i];
		if (t) go(hashOf(t));
	}
	function closeTab(i) {
		const next = close(i);
		go(next ? hashOf(next) : `#${landing()?.id ?? ""}`);
	}
	user_effect(() => {
		const onKey = (e) => {
			dispatch(get(current), e);
		};
		addEventListener("keydown", onKey);
		const offs = [
			onAction("help", () => {
				set(helpOpen, !get(helpOpen));
				return true;
			}),
			onAction("toggle-side", () => {
				toggleSide();
				return true;
			}),
			onAction("toggle-panel", () => {
				set(panelOpen, !get(panelOpen));
				return true;
			}),
			onAction("close-tab", () => {
				if (tabs.active !== -1) closeTab(tabs.active);
				return true;
			}),
			onAction("next-tab", () => {
				if (tabs.list.length) pickTab((tabs.active + 1) % tabs.list.length);
				return true;
			}),
			onAction("prev-tab", () => {
				if (tabs.list.length) pickTab((tabs.active - 1 + tabs.list.length) % tabs.list.length);
				return true;
			}),
			onAction("leave", () => {
				if (get(helpOpen)) {
					set(helpOpen, false);
					return true;
				}
				if (get(notice)) {
					set(notice, "");
					return true;
				}
				return false;
			})
		];
		return () => {
			removeEventListener("keydown", onKey);
			offs.forEach((off) => off());
		};
	});
	const tabTitle = (t) => {
		const s = surfaces().find((x) => x.id === t.surface);
		if (!s) return t.surface;
		return t.focus && s.tab?.(feed, t.focus) || (t.focus ? `${s.title}: ${t.focus}` : s.title);
	};
	const tabIcon = (t) => surfaces().find((x) => x.id === t.surface)?.icon;
	const crumbs = user_derived(() => get(showing) ? get(focus) && tabs.list[tabs.active] ? [get(showing).title, tabTitle(tabs.list[tabs.active])] : [get(showing).title] : []);
	const pulse = user_derived(() => feed.unauthorised ? "no token" : feed.error ? "not answering" : feed.loaded ? "live" : "connecting");
	const projects = user_derived(() => feed.board?.summary?.projects ?? null);
	const status = user_derived(() => surfaces().flatMap((s) => (s.status?.(feed) ?? []).map((i) => ({
		...i,
		surface: s.id,
		title: s.title
	}))));
	const runsAt = user_derived(() => surfaces().find((s) => s.holds === "run")?.id ?? "");
	const groups = user_derived(() => get(helpOpen) ? sheet(get(current)) : []);
	const under = user_derived(() => {
		if (!get(showing)?.transient) return {
			s: get(showing),
			f: get(focus),
			v: get(view)
		};
		const t = tabs.list[tabs.active];
		return {
			s: (t ? surfaces().find((x) => x.id === t.surface) : void 0) ?? landing(),
			f: t?.focus ?? "",
			v: t?.view ?? ""
		};
	});
	const page = user_derived(() => get(under).s);
	const Side = user_derived(() => get(page)?.side);
	function settler() {
		let last = {};
		return (next) => {
			const keys = Object.keys(next);
			let changed = keys.length !== Object.keys(last).length;
			const out = {};
			for (const k of keys) if (k in last && same$1(last[k], next[k])) out[k] = last[k];
			else {
				out[k] = next[k];
				changed = true;
			}
			if (changed) last = out;
			return last;
		};
	}
	const settlePage = settler();
	const settleOverlay = settler();
	const props = user_derived(() => settlePage(get(page) ? get(page).select(feed, get(under).f) : {}));
	const overlayProps = user_derived(() => settleOverlay(get(showing)?.transient ? get(showing).select(feed, get(focus)) : {}));
	function openFocus(f, pinIt = false) {
		if (!get(page)) return;
		go(`#${get(page).id}${f ? `/${encodeURIComponent(f)}` : ""}`);
		if (pinIt) pin();
	}
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var main = root_1();
		var node_1 = child(main);
		var consequent = ($$anchor) => {
			var p = root$1();
			Inline(child(p), { get text() {
				return feed.error;
			} });
			reset(p);
			append($$anchor, p);
		};
		if_block(node_1, ($$render) => {
			if (feed.unauthorised) $$render(consequent);
		});
		Lazy(sibling(node_1, 2), spread_props({
			get load() {
				return get(showing).page;
			},
			get component() {
				return get(showing).component;
			}
		}, () => get(props)));
		reset(main);
		append($$anchor, main);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = root_11();
		var button = first_child(fragment_1);
		var div = sibling(button, 2);
		var node_4 = child(div);
		TitleBar(node_4, {
			get crumbs() {
				return get(crumbs);
			},
			get sideOpen() {
				return get(sideShown);
			},
			get panelOpen() {
				return get(panelOpen);
			},
			find: () => run("open-palette", get(current)),
			create: () => run("new-change", get(current)),
			toggleSide,
			togglePanel: () => set(panelOpen, !get(panelOpen))
		});
		var div_1 = sibling(node_4, 2);
		var node_5 = child(div_1);
		{
			let $0 = user_derived(listed$1);
			ActivityBar(node_5, {
				get items() {
					return get($0);
				},
				get feed() {
					return feed;
				},
				get current() {
					return get(current);
				},
				pick
			});
		}
		var node_6 = sibling(node_5, 2);
		{
			const pane = ($$anchor) => {
				var aside = root_2();
				var node_7 = child(aside);
				var consequent_2 = ($$anchor) => {
					Lazy($$anchor, spread_props({ get load() {
						return get(Side);
					} }, () => get(props), {
						get focus() {
							return get(focus);
						},
						open: openFocus
					}));
				};
				if_block(node_7, ($$render) => {
					if (get(Side)) $$render(consequent_2);
				});
				reset(aside);
				template_effect(() => set_attribute(aside, "aria-label", `${get(showing)?.title ?? "" ?? ""} list`));
				append($$anchor, aside);
			};
			let $0 = user_derived(() => !get(sideShown) || !get(Side));
			Split(node_6, {
				id: "side",
				size: 300,
				min: 200,
				max: 560,
				get collapsed() {
					return get($0);
				},
				pane,
				children: ($$anchor, $$slotProps) => {
					{
						const pane = ($$anchor) => {
							{
								let $0 = user_derived(() => get(runsAt) ? (id) => go(`#${get(runsAt)}/${encodeURIComponent(id)}`) : null);
								Panel($$anchor, {
									get board() {
										return feed.board;
									},
									get error() {
										return feed.error;
									},
									get open() {
										return get($0);
									}
								});
							}
						};
						let $0 = user_derived(() => !get(panelOpen));
						Split($$anchor, {
							id: "panel",
							axis: "y",
							side: "end",
							size: 220,
							min: 120,
							max: 560,
							get collapsed() {
								return get($0);
							},
							pane,
							children: ($$anchor, $$slotProps) => {
								var fragment_5 = root_5();
								var node_8 = first_child(fragment_5);
								var consequent_3 = ($$anchor) => {
									EditorTabs($$anchor, {
										get list() {
											return tabs.list;
										},
										get active() {
											return tabs.active;
										},
										title: tabTitle,
										icon: tabIcon,
										pick: pickTab,
										close: closeTab,
										pin: (i) => {
											pickTab(i);
											pin();
										}
									});
								};
								if_block(node_8, ($$render) => {
									if (tabs.list.length > 0) $$render(consequent_3);
								});
								var main_1 = sibling(node_8, 2);
								var node_9 = child(main_1);
								var consequent_4 = ($$anchor) => {
									var p_1 = root$1();
									Inline(child(p_1), { get text() {
										return feed.error;
									} });
									reset(p_1);
									append($$anchor, p_1);
								};
								var consequent_5 = ($$anchor) => {
									Stale($$anchor, {
										get error() {
											return feed.error;
										},
										get stale_since() {
											return feed.stale_since;
										}
									});
								};
								if_block(node_9, ($$render) => {
									if (feed.unauthorised) $$render(consequent_4);
									else if (feed.error) $$render(consequent_5, 1);
								});
								var node_11 = sibling(node_9, 2);
								var consequent_6 = ($$anchor) => {
									var p_2 = root_3();
									var text = only_child(p_2, true);
									template_effect(() => set_text(text, get(notice)));
									append($$anchor, p_2);
								};
								if_block(node_11, ($$render) => {
									if (get(notice)) $$render(consequent_6);
								});
								var node_12 = sibling(node_11, 2);
								var consequent_7 = ($$anchor) => {
									Lazy($$anchor, spread_props({
										get load() {
											return get(page).page;
										},
										get component() {
											return get(page).component;
										}
									}, () => get(props), {
										get focus() {
											return get(under).f;
										},
										get view() {
											return get(under).v;
										},
										showView,
										open: openFocus
									}));
								};
								var alternate = ($$anchor) => {
									var p_3 = root_4();
									append($$anchor, p_3);
								};
								if_block(node_12, ($$render) => {
									if (get(page)) $$render(consequent_7);
									else $$render(alternate, -1);
								});
								reset(main_1);
								template_effect(() => set_attribute(main_1, "aria-label", get(page)?.heading));
								append($$anchor, fragment_5);
							},
							$$slots: {
								pane: true,
								default: true
							}
						});
					}
				},
				$$slots: {
					pane: true,
					default: true
				}
			});
		}
		reset(div_1);
		var node_13 = sibling(div_1, 2);
		{
			let $0 = user_derived(() => !!feed.error || feed.unauthorised);
			let $1 = user_derived(() => themeState.choice === "system" ? "auto" : themeState.choice);
			StatusBar(node_13, {
				get items() {
					return get(status);
				},
				get projects() {
					return get(projects);
				},
				get pulse() {
					return get(pulse);
				},
				get bad() {
					return get($0);
				},
				get theme() {
					return get($1);
				},
				get cycleTheme() {
					return cycle;
				},
				help: () => set(helpOpen, !get(helpOpen)),
				go: (h) => go(h)
			});
		}
		reset(div);
		var node_14 = sibling(div, 2);
		var consequent_8 = ($$anchor) => {
			var fragment_9 = root_6();
			var div_2 = first_child(fragment_9);
			var div_3 = sibling(div_2, 2);
			Lazy(child(div_3), spread_props({
				get load() {
					return get(showing).page;
				},
				get component() {
					return get(showing).component;
				}
			}, () => get(overlayProps)));
			reset(div_3);
			action(div_3, ($$node) => trap?.($$node));
			template_effect(() => set_attribute(div_3, "aria-label", get(showing).title));
			delegated("click", div_2, () => run("leave", get(current)));
			append($$anchor, fragment_9);
		};
		if_block(node_14, ($$render) => {
			if (get(showing)?.transient) $$render(consequent_8);
		});
		var node_16 = sibling(node_14, 2);
		var consequent_10 = ($$anchor) => {
			var fragment_10 = root_10();
			var div_4 = first_child(fragment_10);
			var div_5 = sibling(div_4, 2);
			var header = child(div_5);
			Icon(child(header), {
				name: "keyboard",
				size: 16
			});
			next(2);
			reset(header);
			var node_18 = sibling(header, 2);
			each(node_18, 17, () => get(groups), (g) => g.title, ($$anchor, g) => {
				const keys = user_derived(() => get(g).keys);
				var fragment_11 = root_9();
				var h3 = first_child(fragment_11);
				var text_1 = only_child(h3, true);
				var dl = sibling(h3, 2);
				each(dl, 21, () => get(keys), (k) => k.surface + k.combo, ($$anchor, k) => {
					var fragment_12 = root_8();
					var dt = first_child(fragment_12);
					each(dt, 22, () => get(k).combos, (c) => c, ($$anchor, c, ci) => {
						var fragment_13 = root_7();
						var node_19 = first_child(fragment_13);
						var consequent_9 = ($$anchor) => {
							var text_2 = text();
							text_2.nodeValue = " ";
							append($$anchor, text_2);
						};
						if_block(node_19, ($$render) => {
							if (get(ci) > 0) $$render(consequent_9);
						});
						var kbd = sibling(node_19);
						var text_3 = only_child(kbd, true);
						template_effect(($0) => set_text(text_3, $0), [() => spell(c)]);
						append($$anchor, fragment_13);
					});
					reset(dt);
					var dd = sibling(dt, 2);
					var text_4 = only_child(dd, true);
					template_effect(() => set_text(text_4, get(k).label));
					append($$anchor, fragment_12);
				});
				reset(dl);
				template_effect(() => set_text(text_1, get(g).title));
				append($$anchor, fragment_11);
			});
			reset(div_5);
			action(div_5, ($$node) => trap?.($$node));
			delegated("click", div_4, () => set(helpOpen, false));
			append($$anchor, fragment_10);
		};
		if_block(node_16, ($$render) => {
			if (get(helpOpen)) $$render(consequent_10);
		});
		delegated("click", button, () => document.getElementById("surface")?.focus());
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (get(showing)?.bare) $$render(consequent_1);
		else $$render(alternate_1, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/Elsewhere.svelte
var root = from_html(`<main class="elsewhere svelte-1pcxrm7"><p class="svelte-1pcxrm7"> </p> <p class="quiet svelte-1pcxrm7">It was started outside this app — <code>devplane up</code> in a terminal, say. Its page has everything this window would.</p> <p class="svelte-1pcxrm7"><a class="open svelte-1pcxrm7">Open that page</a></p></main>`);
function Elsewhere($$anchor, $$props) {
	var main = root();
	var p = child(main);
	var text = only_child(p);
	var p_1 = sibling(p, 4);
	var a = only_child(p_1);
	reset(main);
	template_effect(() => {
		set_text(text, `A host is already serving this home on port ${$$props.port ?? ""}.`);
		set_attribute(a, "href", `http://127.0.0.1:${$$props.port ?? ""}/`);
	});
	append($$anchor, main);
}
//#endregion
//#region src/main.ts
var target = document.getElementById("app");
if (!target) throw new Error("no #app to mount into");
var elsewhere = Number(new URLSearchParams(location.search).get("elsewhere"));
Number.isInteger(elsewhere) && elsewhere > 0 ? mount(Elsewhere, {
	target,
	props: { port: elsewhere }
}) : mount(App, { target });
//#endregion
export { State as C, listed$1 as D, landing as E, surfaces as O, STATES as S, answerItemKeys as T, address as _, Inline as a, remember as b, clip as c, shape as d, reread as f, skippedSteps as g, keepSkipped as h, Skeleton as i, listed as l, incomplete as m, Icon as n, blocks as o, TITLES as p, Split as r, ago as s, Pill as t, plural as u, itemFocus as v, go as w, LIFE as x, place as y };

//# sourceMappingURL=app.js.map