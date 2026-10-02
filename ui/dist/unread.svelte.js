import { $ as push, E as if_block, F as delegated, G as sibling, H as child, J as state, K as proxy, L as get, O as set_text, P as delegate, Q as pop, W as only_child, j as from_html, k as append, nt as reset, q as set, z as template_effect } from "./api.js";
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
//#region src/surfaces/change/review/marks.ts
function digest(text) {
	let h = 2166136261;
	for (let i = 0; i < text.length; i++) {
		h ^= text.charCodeAt(i);
		h = Math.imul(h, 16777619);
	}
	return (h >>> 0).toString(16).padStart(8, "0");
}
var prefix = (change) => `review:${change}:`;
function markKey(change, path, header, lines) {
	const body = lines.map(([kind, text]) => `${kind}${text}`).join("\n");
	return `${prefix(change)}${path}:${digest(`${header}\n${body}`)}`;
}
function readMark(key) {
	try {
		return localStorage.getItem(key) === "seen" ? "seen" : null;
	} catch {
		return null;
	}
}
function writeMark$1(key, mark) {
	try {
		localStorage.setItem(key, mark);
	} catch {}
}
function prune(change, live) {
	try {
		const gone = [];
		for (let i = 0; i < localStorage.length; i++) {
			const k = localStorage.key(i);
			if (k?.startsWith(prefix(change)) && !live.has(k)) gone.push(k);
		}
		gone.forEach((k) => localStorage.removeItem(k));
	} catch {}
}
function markedFor(change) {
	try {
		let n = 0;
		for (let i = 0; i < localStorage.length; i++) {
			const k = localStorage.key(i);
			if (k?.startsWith(prefix(change)) && readMark(k)) n++;
		}
		return n;
	} catch {
		return 0;
	}
}
//#endregion
//#region src/surfaces/change/review/unread.svelte.ts
var marks = proxy({ n: 0 });
function writeMark(key, mark) {
	writeMark$1(key, mark);
	marks.n += 1;
}
function unread(r, fallback = 0) {
	marks.n;
	if (!r) return {
		hunks: null,
		weakened: fallback
	};
	let hunks = 0;
	for (const g of r.groups) for (const f of g.files) for (const h of f.hunks) if (!readMark(markKey(r.change, f.path, h.header, h.lines))) hunks += 1;
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
export { markedFor as a, Commands as c, markKey as i, unreadSays as n, prune as o, writeMark as r, readMark as s, unread as t };

//# sourceMappingURL=unread.svelte.js.map