import { A as append, D as if_block, F as delegate, G as child, H as template_effect, I as delegated, J as sibling, M as from_html, R as get, X as set, Y as proxy, Z as state, at as reset, i as api, k as set_text, nt as push, q as only_child, tt as pop } from "./api.js";
import { p as copyText } from "./app.js";
//#region src/lib/Commands.svelte
var root = from_html(`<span class="note svelte-1hip6z1" role="status">Copied.</span>`);
var root_1 = from_html(`<span class="note svelte-1hip6z1" role="status">Nothing was copied — this page has no clipboard; select the text.</span>`);
var root_2 = from_html(`<div class="commands svelte-1hip6z1"><pre class="svelte-1hip6z1"> </pre> <button type="button">Copy</button> <!> <!></div>`);
function Commands($$anchor, $$props) {
	push($$props, true);
	let copied = state("");
	async function copy() {
		set(copied, await copyText($$props.commands.join("\n")) ? "yes" : "no", true);
	}
	var div = root_2();
	var pre = child(div);
	var text = only_child(pre, true);
	var button = sibling(pre, 2);
	var node = sibling(button, 2);
	var consequent = ($$anchor) => {
		var span = root();
		append($$anchor, span);
	};
	if_block(node, ($$render) => {
		if (get(copied) === "yes") $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1();
		append($$anchor, span_1);
	};
	if_block(node_1, ($$render) => {
		if (get(copied) === "no") $$render(consequent_1);
	});
	reset(div);
	template_effect(($0) => set_text(text, $0), [() => $$props.commands.join("\n")]);
	delegated("click", button, copy);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/unread.svelte.ts
var posted = proxy({ keys: /* @__PURE__ */ new Set() });
var keyOf = (change, path, digest) => `${change}\u0000${path}\u0000${digest}`;
function isSeen(change, path, h) {
	return !!h.seen || !!h.digest && posted.keys.has(keyOf(change, path, h.digest));
}
function unitSeen(change, f) {
	return !!f.unit_seen || posted.keys.has(keyOf(change, f.path, "*"));
}
function unreadIn(change, f) {
	if (!f.hunks.length) return f.unit_seen === false && !unitSeen(change, f) ? 1 : 0;
	let n = 0;
	for (const h of f.hunks) if (!isSeen(change, f.path, h)) n += 1;
	return n;
}
async function markSeen(change, path, digest) {
	const k = keyOf(change, path, digest ?? "*");
	posted.keys = /* @__PURE__ */ new Set([...posted.keys, k]);
	try {
		await api(`/api/changes/${encodeURIComponent(change)}/review/seen`, {
			method: "POST",
			body: JSON.stringify(digest ? {
				path,
				digest
			} : { path })
		});
	} catch (e) {
		const next = new Set(posted.keys);
		next.delete(k);
		posted.keys = next;
		throw e;
	}
}
function unread(r, fallback = 0) {
	posted.keys;
	if (!r) return {
		hunks: null,
		weakened: fallback
	};
	let hunks = 0;
	for (const g of r.groups) for (const f of g.files) hunks += unreadIn(r.change, f);
	const weakened = r.groups.flatMap((g) => g.weakened ?? []).filter((w) => !w.seen).length;
	return {
		hunks,
		weakened
	};
}
function unreadSays(u) {
	const parts = [...u.hunks ? [`${u.hunks} ${u.hunks === 1 ? "hunk" : "hunks"} unread`] : [], ...u.weakened ? [`${u.weakened} weakened`] : []];
	return parts.length ? parts.join(" · ") : u.hunks === null ? "" : "0 unread";
}
//#endregion
export { unreadIn as a, unread as i, markSeen as n, unreadSays as o, unitSeen as r, Commands as s, isSeen as t };

//# sourceMappingURL=unread.svelte.js.map