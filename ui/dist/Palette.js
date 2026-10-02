import { B as effect, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, I as event, K as only_child, L as get, O as set_text, P as delegate, R as tick, V as template_effect, W as child, X as state, Y as set, Z as user_derived, b as action, et as pop, f as bind_value, it as reset, j as from_html, k as append, m as set_attribute, o as prop, p as remove_input_defaults, q as sibling, rt as next, tt as push, v as set_class } from "./api.js";
import { A as spell, E as help, M as listed, N as surfaces, O as run, T as all, h as resource, j as landing, n as Icon, w as go } from "./app.js";
import { t as Failed } from "./Failed.js";
import { t as ChangeState } from "./ChangeState.js";
import { t as here } from "./offered.svelte.js";
//#region src/surfaces/palette/Palette.svelte
function unique(entries) {
	const kept = /* @__PURE__ */ new Map();
	for (const e of entries) {
		const have = kept.get(e.id);
		if (!have) kept.set(e.id, e);
		else if (!have.hint && e.hint) kept.set(e.id, {
			...have,
			hint: e.hint
		});
	}
	return [...kept.values()];
}
function focusOnMount(node) {
	tick().then(() => node.focus());
}
function score(q, text) {
	const t = text.toLowerCase();
	if (!q) return 0;
	const at = t.indexOf(q);
	if (at === 0) return 0;
	if (at > 0) return /\W/.test(t[at - 1]) ? 1 : 2;
	const starts = (k) => k === 0 || /[^\p{L}\p{N}]/u.test(t[k - 1]);
	let last = -1;
	let jumps = 0;
	for (const ch of q.replace(/\s+/g, "")) {
		if (last !== -1 && t[last + 1] === ch) {
			last += 1;
			continue;
		}
		let j = -1;
		for (let k = last + 1; k < t.length; k++) if (t[k] === ch && starts(k)) {
			j = k;
			break;
		}
		if (j === -1) return null;
		jumps += 1;
		last = j;
	}
	return 3 + jumps;
}
var LIST_ACTIONS = /* @__PURE__ */ new Set([
	"next",
	"prev",
	"first",
	"last",
	"open"
]);
function commands(on) {
	const out = [];
	for (const c of on.here) out.push({
		id: `here:${c.label}`,
		kind: "here",
		icon: c.icon,
		label: c.label,
		hint: c.hint,
		go: () => {
			on.back();
			setTimeout(c.run, 0);
		}
	});
	for (const c of on.changes) {
		const to = on.opensChange;
		if (!to) continue;
		out.push({
			id: `change:${c.id}`,
			kind: "change",
			icon: "change",
			label: c.title || c.id,
			hint: c.state,
			qualifier: c.qualifier,
			go: () => go(`#${to}/${encodeURIComponent(c.id)}`)
		});
	}
	const place = (title) => `go to the ${title.toLowerCase()}`;
	for (const s of on.places) {
		const key = on.keys.find((b) => b.surface === "global" && b.label === place(s.title));
		out.push({
			id: `place:${s.id}`,
			kind: "surface",
			icon: s.icon ?? "right",
			label: `Go to ${s.title}`,
			hint: key ? spell(key.combo) : void 0,
			go: () => go(`#${s.id}`)
		});
	}
	for (const b of on.keys) {
		if (b.action === "open-palette" || b.action === "leave" || LIST_ACTIONS.has(b.action)) continue;
		const to = b.surface === "global" ? on.places.find((s) => b.label === place(s.title)) : void 0;
		out.push({
			id: to ? `place:${to.id}` : `action:${b.action}`,
			kind: "action",
			icon: "keyboard",
			label: b.label.charAt(0).toUpperCase() + b.label.slice(1),
			hint: spell(b.combo),
			go: () => {
				on.back();
				setTimeout(() => run(b.action, on.over), 0);
			}
		});
	}
	for (const p of on.projects) {
		const to = on.landing;
		if (!to) continue;
		out.push({
			id: `project:${p.id}`,
			kind: "project",
			icon: "folder",
			label: p.name,
			hint: "narrow to this project",
			go: () => go(`#${to}/${encodeURIComponent(p.name)}`)
		});
	}
	return unique(out);
}
var ORDER = [
	"here",
	"change",
	"surface",
	"action",
	"project"
];
function rank(entries, raw) {
	const typed = raw.trim().toLowerCase();
	const actions = typed.startsWith(">");
	const q = actions ? typed.slice(1).trim() : typed;
	return entries.filter((e) => !actions || e.kind === "action").map((e) => ({
		e,
		s: score(q, e.label)
	})).filter((x) => x.s !== null).sort((a, b) => ORDER.indexOf(a.e.kind) - ORDER.indexOf(b.e.kind) || a.s - b.s).map((x) => x.e);
}
var root = from_html(`<div class="unread svelte-d7fgwz"><!></div>`);
var root_1 = from_html(`<li class="sec svelte-d7fgwz" role="presentation"> </li>`);
var root_2 = from_html(`<span> </span>`);
var root_3 = from_html(`<!> <li role="option" class="svelte-d7fgwz"><button tabindex="-1" class="svelte-d7fgwz"><!> <span class="label svelte-d7fgwz"> </span> <!></button></li>`, 1);
var root_4 = from_html(`<li class="none svelte-d7fgwz"> </li>`);
var root_5 = from_html(`<section class="palette svelte-d7fgwz" aria-labelledby="palette-head"><h2 id="palette-head" class="sr-only">Everything, by name</h2> <label class="field svelte-d7fgwz"><!> <input placeholder="Type a change, a place, an action — or > for actions only" aria-label="find by name" role="combobox" aria-expanded="true" aria-controls="palette-matches" aria-autocomplete="list" class="svelte-d7fgwz"/></label> <!> <!> <ul role="listbox" aria-label="matches" id="palette-matches" class="svelte-d7fgwz"><!> <!></ul> <footer class="svelte-d7fgwz"><span><kbd class="svelte-d7fgwz">↑</kbd><kbd class="svelte-d7fgwz">↓</kbd> move</span><span><kbd class="svelte-d7fgwz">↵</kbd> open</span><span><kbd class="svelte-d7fgwz">esc</kbd> close</span></footer></section>`);
function Palette($$anchor, $$props) {
	push($$props, true);
	let from = prop($$props, "from", 3, ""), changes = prop($$props, "changes", 3, null), projects = prop($$props, "projects", 3, null);
	let query = state("");
	const changesRead = resource(() => "/api/changes", { tell: () => "devplane change list" });
	const projectsRead = resource(() => "/api/projects", { tell: () => "devplane doctor" });
	const fetchedChanges = user_derived(() => Array.isArray(changesRead.data) ? changesRead.data : null);
	const fetchedProjects = user_derived(() => Array.isArray(projectsRead.data) ? projectsRead.data : null);
	const over = user_derived(() => from().replace(/^#/, "").split("/")[0] || landing()?.id || "");
	const opensChange = user_derived(() => surfaces().find((s) => s.link === "change"));
	const entries = user_derived(() => commands({
		here: here.list,
		changes: get(fetchedChanges) ?? changes() ?? [],
		opensChange: get(opensChange)?.id,
		places: listed(),
		keys: help(get(over)),
		projects: get(fetchedProjects) ?? projects() ?? [],
		landing: landing()?.id,
		over: get(over),
		back: () => go(from() || "")
	}));
	const TITLES = user_derived(() => ({
		here: here.title ? `This change: ${here.title}` : "Here",
		change: "Changes",
		surface: "Go to",
		action: "Actions",
		project: "Projects"
	}));
	const searcher = user_derived(() => surfaces().find((s) => s.takesQuery));
	const shown = user_derived(() => {
		const raw = get(query).trim();
		const actions = raw.startsWith(">");
		return rank(get(entries), raw).concat(raw && !actions && get(searcher) ? [{
			id: "search",
			kind: "surface",
			icon: "search",
			label: `Search every session for “${raw}”`,
			go: () => go(`#${get(searcher).id}/${encodeURIComponent(raw)}`)
		}] : []);
	});
	let at = state(0);
	user_effect(() => {
		get(query);
		set(at, 0);
	});
	function typed(e) {
		if (e.key === "ArrowDown" || e.ctrlKey && e.key === "n") {
			set(at, Math.min(get(shown).length - 1, get(at) + 1), true);
			e.preventDefault();
		} else if (e.key === "ArrowUp" || e.ctrlKey && e.key === "p") {
			set(at, Math.max(0, get(at) - 1), true);
			e.preventDefault();
		} else if (e.key === "Enter") {
			get(shown)[get(at)]?.go();
			e.preventDefault();
		}
		queueMicrotask(() => document.querySelector(".palette li[aria-selected='true']")?.scrollIntoView({ block: "nearest" }));
	}
	const bound = user_derived(() => all().length);
	var section = root_5();
	var label = sibling(child(section), 2);
	var node_1 = child(label);
	Icon(node_1, {
		name: "search",
		size: 16
	});
	var input = sibling(node_1, 2);
	remove_input_defaults(input);
	effect(() => bind_value(input, () => get(query), ($$value) => set(query, $$value)));
	action(input, ($$node) => focusOnMount?.($$node));
	reset(label);
	var node_2 = sibling(label, 2);
	var consequent = ($$anchor) => {
		var div = root();
		var node_3 = child(div);
		Failed(node_3, {
			what: "the changes, so none are listed",
			get failure() {
				return changesRead.failure;
			}
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node_2, ($$render) => {
		if (changesRead.failure && !changesRead.data) $$render(consequent);
	});
	var node_4 = sibling(node_2, 2);
	var consequent_1 = ($$anchor) => {
		var div_1 = root();
		var node_5 = child(div_1);
		Failed(node_5, {
			what: "the projects, so none are listed",
			get failure() {
				return projectsRead.failure;
			}
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_4, ($$render) => {
		if (projectsRead.failure && !projectsRead.data) $$render(consequent_1);
	});
	var ul = sibling(node_4, 2);
	var node_6 = child(ul);
	each(node_6, 19, () => get(shown), (e) => e.id, ($$anchor, e, i) => {
		var fragment = root_3();
		var node_7 = first_child(fragment);
		var consequent_2 = ($$anchor) => {
			var li = root_1();
			var text_1 = only_child(li, true);
			template_effect(() => set_text(text_1, get(TITLES)[get(e).kind]));
			append($$anchor, li);
		};
		if_block(node_7, ($$render) => {
			if (get(i) === 0 || get(shown)[get(i) - 1].kind !== get(e).kind) $$render(consequent_2);
		});
		var li_1 = sibling(node_7, 2);
		var button = child(li_1);
		var node_8 = child(button);
		Icon(node_8, {
			get name() {
				return get(e).icon;
			},
			size: 14
		});
		var span = sibling(node_8, 2);
		var text_2 = only_child(span, true);
		var node_9 = sibling(span, 2);
		var consequent_3 = ($$anchor) => {
			ChangeState($$anchor, {
				get state() {
					return get(e).hint;
				},
				get q() {
					return get(e).qualifier;
				}
			});
		};
		var consequent_4 = ($$anchor) => {
			var span_1 = root_2();
			let classes;
			var text_3 = only_child(span_1, true);
			template_effect(() => {
				classes = set_class(span_1, 1, "hint svelte-d7fgwz", null, classes, { kbd: get(e).kind === "action" || get(e).kind === "surface" });
				set_text(text_3, get(e).hint);
			});
			append($$anchor, span_1);
		};
		if_block(node_9, ($$render) => {
			if (get(e).kind === "change" && get(e).hint) $$render(consequent_3);
			else if (get(e).hint) $$render(consequent_4, 1);
		});
		reset(button);
		reset(li_1);
		template_effect(() => {
			set_attribute(li_1, "id", `palette-opt-${get(i) ?? ""}`);
			set_attribute(li_1, "aria-selected", get(i) === get(at));
			set_text(text_2, get(e).label);
		});
		delegated("click", button, function(...$$args) {
			get(e).go?.apply(this, $$args);
		});
		event("mouseenter", button, () => set(at, get(i), true));
		append($$anchor, fragment);
	});
	var node_10 = sibling(node_6, 2);
	var consequent_5 = ($$anchor) => {
		var li_2 = root_4();
		var text_4 = only_child(li_2);
		template_effect(() => set_text(text_4, `Nothing by that name — ${get(entries).length ?? ""} things and ${get(bound) ?? ""} keys are here.`));
		append($$anchor, li_2);
	};
	if_block(node_10, ($$render) => {
		if (get(shown).length === 0) $$render(consequent_5);
	});
	reset(ul);
	next(2);
	reset(section);
	template_effect(() => set_attribute(input, "aria-activedescendant", get(shown)[get(at)] ? `palette-opt-${get(at)}` : void 0));
	delegated("keydown", input, typed);
	append($$anchor, section);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
export { ORDER, commands, Palette as default, focusOnMount, rank, score, unique };

//# sourceMappingURL=Palette.js.map