import { $ as push, B as user_effect, C as each, E as if_block, F as delegated, G as sibling, H as child, I as event, J as state, L as get, O as set_text, P as delegate, Q as pop, U as first_child, W as only_child, X as autofocus, Y as user_derived, f as bind_value, j as from_html, k as append, m as set_attribute, nt as reset, o as prop, p as remove_input_defaults, q as set, tt as next, v as set_class, w as index, z as template_effect } from "./api.js";
import { A as spell, E as help, M as listed, N as surfaces, O as run, T as all, h as resource, j as landing, n as Icon, w as go } from "./app.js";
import { t as Failed } from "./Failed.js";
import { t as ChangeState } from "./ChangeState.js";
import { t as here } from "./offered.svelte.js";
//#region src/surfaces/palette/Palette.svelte
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
	const LIST_ACTIONS = /* @__PURE__ */ new Set([
		"next",
		"prev",
		"first",
		"last",
		"open"
	]);
	const over = user_derived(() => from().replace(/^#/, "").split("/")[0] || landing()?.id || "");
	const opensChange = user_derived(() => surfaces().find((s) => s.link === "change"));
	const entries = user_derived(() => {
		const out = [];
		for (const c of here.list) out.push({
			kind: "here",
			icon: c.icon,
			label: c.label,
			hint: c.hint,
			go: () => {
				go(from() || "");
				setTimeout(c.run, 0);
			}
		});
		for (const c of get(fetchedChanges) ?? changes() ?? []) {
			const to = get(opensChange)?.id;
			if (!to) continue;
			out.push({
				kind: "change",
				icon: "change",
				label: c.title || c.id,
				hint: c.state,
				qualifier: c.qualifier,
				go: () => go(`#${to}/${encodeURIComponent(c.id)}`)
			});
		}
		for (const s of listed()) out.push({
			kind: "surface",
			icon: s.icon ?? "right",
			label: `Go to ${s.title}`,
			go: () => go(`#${s.id}`)
		});
		for (const b of help(get(over))) {
			if (b.action === "open-palette" || b.action === "leave" || LIST_ACTIONS.has(b.action)) continue;
			out.push({
				kind: "action",
				icon: "keyboard",
				label: b.label.charAt(0).toUpperCase() + b.label.slice(1),
				hint: spell(b.combo),
				go: () => {
					go(from() || "");
					setTimeout(() => run(b.action, get(over)), 0);
				}
			});
		}
		for (const p of get(fetchedProjects) ?? projects() ?? []) {
			const to = landing()?.id;
			if (!to) continue;
			out.push({
				kind: "project",
				icon: "folder",
				label: p.name,
				hint: "narrow to this project",
				go: () => go(`#${to}/${encodeURIComponent(p.name)}`)
			});
		}
		return out;
	});
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
	var node = child(label);
	Icon(node, {
		name: "search",
		size: 16
	});
	var input = sibling(node, 2);
	remove_input_defaults(input);
	autofocus(input, true);
	reset(label);
	var node_1 = sibling(label, 2);
	var consequent = ($$anchor) => {
		var div = root();
		var node_2 = child(div);
		Failed(node_2, {
			what: "the changes, so none are listed",
			get failure() {
				return changesRead.failure;
			}
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node_1, ($$render) => {
		if (changesRead.failure && !changesRead.data) $$render(consequent);
	});
	var node_3 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var div_1 = root();
		var node_4 = child(div_1);
		Failed(node_4, {
			what: "the projects, so none are listed",
			get failure() {
				return projectsRead.failure;
			}
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_3, ($$render) => {
		if (projectsRead.failure && !projectsRead.data) $$render(consequent_1);
	});
	var ul = sibling(node_3, 2);
	var node_5 = child(ul);
	each(node_5, 17, () => get(shown), index, ($$anchor, e, i) => {
		var fragment = root_3();
		var node_6 = first_child(fragment);
		var consequent_2 = ($$anchor) => {
			var li = root_1();
			var text_1 = only_child(li, true);
			template_effect(() => set_text(text_1, get(TITLES)[get(e).kind]));
			append($$anchor, li);
		};
		if_block(node_6, ($$render) => {
			if (i === 0 || get(shown)[i - 1].kind !== get(e).kind) $$render(consequent_2);
		});
		var li_1 = sibling(node_6, 2);
		set_attribute(li_1, "id", `palette-opt-${i}`);
		var button = child(li_1);
		var node_7 = child(button);
		Icon(node_7, {
			get name() {
				return get(e).icon;
			},
			size: 14
		});
		var span = sibling(node_7, 2);
		var text_2 = only_child(span, true);
		var node_8 = sibling(span, 2);
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
				classes = set_class(span_1, 1, "hint svelte-d7fgwz", null, classes, { kbd: get(e).kind === "action" });
				set_text(text_3, get(e).hint);
			});
			append($$anchor, span_1);
		};
		if_block(node_8, ($$render) => {
			if (get(e).kind === "change" && get(e).hint) $$render(consequent_3);
			else if (get(e).hint) $$render(consequent_4, 1);
		});
		reset(button);
		reset(li_1);
		template_effect(() => {
			set_attribute(li_1, "aria-selected", i === get(at));
			set_text(text_2, get(e).label);
		});
		delegated("click", button, function(...$$args) {
			get(e).go?.apply(this, $$args);
		});
		event("mouseenter", button, () => set(at, i, true));
		append($$anchor, fragment);
	});
	var node_9 = sibling(node_5, 2);
	var consequent_5 = ($$anchor) => {
		var li_2 = root_4();
		var text_4 = only_child(li_2);
		template_effect(() => set_text(text_4, `Nothing by that name — ${get(entries).length ?? ""} things and ${get(bound) ?? ""} keys are here.`));
		append($$anchor, li_2);
	};
	if_block(node_9, ($$render) => {
		if (get(shown).length === 0) $$render(consequent_5);
	});
	reset(ul);
	next(2);
	reset(section);
	template_effect(() => set_attribute(input, "aria-activedescendant", get(shown)[get(at)] ? `palette-opt-${get(at)}` : void 0));
	delegated("keydown", input, typed);
	bind_value(input, () => get(query), ($$value) => set(query, $$value));
	append($$anchor, section);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
export { ORDER, Palette as default, rank, score };

//# sourceMappingURL=Palette.js.map