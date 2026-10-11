import { $ as user_effect, B as append, C as remove_input_defaults, G as delegate, H as from_html, J as get, K as delegated, L as if_block, P as each, Q as template_effect, S as bind_value, V as comment, X as untrack, _t as to_array, at as proxy, ct as user_derived, dt as pop, f as run, ft as push, g as prop, ht as reset, it as sibling, mt as next, nt as first_child, ot as set, rt as only_child, st as state, tt as child, w as set_attribute, z as set_text } from "./api.js";
import { r as resource } from "./resource.svelte.js";
import { n as Icon } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Failed } from "./Failed.js";
import { t as ChangeState } from "./ChangeState.js";
//#region src/surfaces/change/List.svelte
var root = from_html(`<div class="fail"><!></div>`);
var root_1 = from_html(`<div class="skel svelte-3ehcwp"></div>`);
var root_2 = from_html(`<p class="quiet svelte-3ehcwp">No change has been started on this machine. <button class="link svelte-3ehcwp">Start one</button> — an isolated worktree, an agent in it, and the project's own gates.</p>`);
var root_3 = from_html(`<p class="quiet svelte-3ehcwp"> </p>`);
var root_4 = from_html(`<span class="wait svelte-3ehcwp"> </span>`);
var root_5 = from_html(`<span class="dim svelte-3ehcwp">in place</span>`);
var root_6 = from_html(`<span class="claimed svelte-3ehcwp"> </span>`);
var root_7 = from_html(`<span class="branch svelte-3ehcwp"><!> </span>`);
var root_8 = from_html(`<div class="row svelte-3ehcwp" role="option" tabindex="-1"><span class="title svelte-3ehcwp"> </span> <span class="meta svelte-3ehcwp"><!> <!> <!></span> <!> <!></div>`);
var root_9 = from_html(`<button class="group svelte-3ehcwp"><!> <!> <span> </span> <span class="n svelte-3ehcwp"> </span></button> <!>`, 1);
var root_10 = from_html(`<button class="foot svelte-3ehcwp"> </button>`);
var root_11 = from_html(`<div class="list svelte-3ehcwp"><header class="svelte-3ehcwp"><span class="t svelte-3ehcwp">Changes</span> <span class="n svelte-3ehcwp"> </span> <button class="icon svelte-3ehcwp" title="Start a new change (Alt+N)" aria-label="start a new change"><!></button></header> <label class="filter svelte-3ehcwp"><!> <input placeholder="Filter by title, branch, project, state" aria-label="filter the changes" class="svelte-3ehcwp"/></label> <div class="rows svelte-3ehcwp" role="listbox" aria-label="changes" tabindex="0"><!></div> <!></div>`);
function List($$anchor, $$props) {
	push($$props, true);
	let all = prop($$props, "all", 19, () => []), focus = prop($$props, "focus", 3, ""), planted = prop($$props, "rows", 3, null);
	const read = resource(() => "/api/changes", { tell: () => "devplane change list" });
	const rows = user_derived(() => Array.isArray(read.data) ? read.data : planted());
	let filter = state("");
	let showArchived = state(false);
	const signature = user_derived(() => all().map((b) => `${b.id}:${b.state}`).join("|"));
	let lastSignature = null;
	user_effect(() => {
		const sig = get(signature);
		untrack(() => {
			if (lastSignature !== null && sig !== lastSignature) read.reload();
			lastSignature = sig;
		});
	});
	const name = (p) => p.split("/").filter(Boolean).pop() ?? p;
	const visible = user_derived(() => (get(rows) ?? []).filter((r) => (get(showArchived) || !r.archived_at) && (!get(filter).trim() || `${r.title} ${r.branch ?? ""} ${name(r.project_id)} ${r.state}`.toLowerCase().includes(get(filter).trim().toLowerCase()))));
	const groups = user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const r of get(visible)) {
			const k = name(r.project_id);
			m.set(k, [...m.get(k) ?? [], r]);
		}
		return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
	});
	const archived = user_derived(() => (get(rows) ?? []).filter((r) => r.archived_at).length);
	let folded = state(proxy({}));
	const flat = user_derived(() => get(groups).flatMap(([g, rs]) => get(folded)[g] ? [] : rs));
	user_effect(() => cursor({
		scope: "change",
		size: () => get(flat).length,
		at: () => get(flat).findIndex((r) => r.id === focus()),
		go: (i) => {
			$$props.open(get(flat)[i].id);
			queueMicrotask(() => document.getElementById(optionId(get(flat)[i].id))?.scrollIntoView({ block: "nearest" }));
		},
		open: (i) => $$props.open(get(flat)[i].id, true)
	}));
	const optionId = (id) => `change-row-${id.replace(/[^\w-]/g, "_")}`;
	const active = user_derived(() => get(visible).some((r) => r.id === focus()) ? optionId(focus()) : void 0);
	var div = root_11();
	var header = child(div);
	var span = sibling(child(header), 2);
	var text = only_child(span, true);
	var button = sibling(span, 2);
	var node = child(button);
	Icon(node, {
		name: "plus",
		size: 15
	});
	reset(button);
	reset(header);
	var label = sibling(header, 2);
	var node_1 = child(label);
	Icon(node_1, {
		name: "filter",
		size: 13
	});
	var input = sibling(node_1, 2);
	remove_input_defaults(input);
	reset(label);
	var div_1 = sibling(label, 2);
	var node_2 = child(div_1);
	var consequent = ($$anchor) => {
		var div_2 = root();
		var node_3 = child(div_2);
		{
			let $0 = user_derived(() => read.data !== null);
			Failed(node_3, {
				what: "the changes",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
		reset(div_2);
		append($$anchor, div_2);
	};
	var consequent_1 = ($$anchor) => {
		var fragment = comment();
		var node_4 = first_child(fragment);
		each(node_4, 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			var div_3 = root_1();
			append($$anchor, div_3);
		});
		append($$anchor, fragment);
	};
	var consequent_2 = ($$anchor) => {
		var p_1 = root_2();
		var button_1 = sibling(child(p_1));
		next();
		reset(p_1);
		delegated("click", button_1, () => run("new-change", "change"));
		append($$anchor, p_1);
	};
	var consequent_3 = ($$anchor) => {
		var p_2 = root_3();
		var text_1 = only_child(p_2);
		template_effect(() => set_text(text_1, `No change matches “${get(filter) ?? ""}”.`));
		append($$anchor, p_2);
	};
	var alternate = ($$anchor) => {
		var fragment_1 = comment();
		var node_5 = first_child(fragment_1);
		each(node_5, 17, () => get(groups), ([g, rs]) => g, ($$anchor, $$item) => {
			var $$array = user_derived(() => to_array(get($$item), 2));
			let g = () => get($$array)[0];
			let rs = () => get($$array)[1];
			var fragment_2 = root_9();
			var button_2 = first_child(fragment_2);
			var node_6 = child(button_2);
			{
				let $0 = user_derived(() => get(folded)[g()] ? "right" : "down");
				Icon(node_6, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
			var node_7 = sibling(node_6, 2);
			Icon(node_7, {
				name: "folder",
				size: 13
			});
			var span_1 = sibling(node_7, 2);
			var text_2 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var text_3 = only_child(span_2, true);
			reset(button_2);
			var node_8 = sibling(button_2, 2);
			var consequent_8 = ($$anchor) => {
				var fragment_3 = comment();
				var node_9 = first_child(fragment_3);
				each(node_9, 17, rs, (r) => r.id, ($$anchor, r) => {
					var div_4 = root_8();
					var span_3 = child(div_4);
					var text_4 = only_child(span_3, true);
					var span_4 = sibling(span_3, 2);
					var node_10 = child(span_4);
					ChangeState(node_10, {
						get state() {
							return get(r).state;
						},
						get q() {
							return get(r).qualifier;
						}
					});
					var node_11 = sibling(node_10, 2);
					var consequent_4 = ($$anchor) => {
						var span_5 = root_4();
						var text_5 = only_child(span_5, true);
						template_effect(() => set_text(text_5, get(r).waiting_says));
						append($$anchor, span_5);
					};
					if_block(node_11, ($$render) => {
						if (get(r).waiting_says) $$render(consequent_4);
					});
					var node_12 = sibling(node_11, 2);
					var consequent_5 = ($$anchor) => {
						var span_6 = root_5();
						append($$anchor, span_6);
					};
					if_block(node_12, ($$render) => {
						if (get(r).in_place) $$render(consequent_5);
					});
					reset(span_4);
					var node_13 = sibling(span_4, 2);
					var consequent_6 = ($$anchor) => {
						var span_7 = root_6();
						var text_6 = only_child(span_7, true);
						template_effect(() => set_text(text_6, get(r).stop_claim.says));
						append($$anchor, span_7);
					};
					if_block(node_13, ($$render) => {
						if (get(r).stop_claim) $$render(consequent_6);
					});
					var node_14 = sibling(node_13, 2);
					var consequent_7 = ($$anchor) => {
						var span_8 = root_7();
						var node_15 = child(span_8);
						Icon(node_15, {
							name: "change",
							size: 11
						});
						var text_7 = sibling(node_15);
						reset(span_8);
						template_effect(() => set_text(text_7, ` ${get(r).branch ?? ""}`));
						append($$anchor, span_8);
					};
					if_block(node_14, ($$render) => {
						if (get(r).branch) $$render(consequent_7);
					});
					reset(div_4);
					template_effect(($0) => {
						set_attribute(div_4, "id", $0);
						set_attribute(div_4, "aria-selected", get(r).id === focus());
						set_attribute(span_3, "title", get(r).title || get(r).id);
						set_text(text_4, get(r).title || get(r).id);
					}, [() => optionId(get(r).id)]);
					delegated("click", div_4, () => $$props.open(get(r).id));
					delegated("dblclick", div_4, () => $$props.open(get(r).id, true));
					append($$anchor, div_4);
				});
				append($$anchor, fragment_3);
			};
			if_block(node_8, ($$render) => {
				if (!get(folded)[g()]) $$render(consequent_8);
			});
			template_effect(() => {
				set_attribute(button_2, "aria-expanded", !get(folded)[g()]);
				set_text(text_2, g());
				set_text(text_3, rs().length);
			});
			delegated("click", button_2, () => set(folded, {
				...get(folded),
				[g()]: !get(folded)[g()]
			}, true));
			append($$anchor, fragment_2);
		});
		append($$anchor, fragment_1);
	};
	if_block(node_2, ($$render) => {
		if (read.failure) $$render(consequent);
		else if (get(rows) === null) $$render(consequent_1, 1);
		else if (get(rows).length === 0) $$render(consequent_2, 2);
		else if (get(visible).length === 0) $$render(consequent_3, 3);
		else $$render(alternate, -1);
	});
	reset(div_1);
	var node_16 = sibling(div_1, 2);
	var consequent_9 = ($$anchor) => {
		var button_3 = root_10();
		var text_8 = only_child(button_3);
		template_effect(() => set_text(text_8, `${get(showArchived) ? "Hide" : "Show"} ${get(archived) ?? ""} archived`));
		delegated("click", button_3, () => set(showArchived, !get(showArchived)));
		append($$anchor, button_3);
	};
	if_block(node_16, ($$render) => {
		if (get(archived) > 0) $$render(consequent_9);
	});
	reset(div);
	template_effect(() => {
		set_text(text, get(rows) === null ? "" : get(visible).length);
		set_attribute(div_1, "aria-activedescendant", get(active));
	});
	delegated("click", button, () => run("new-change", "change"));
	bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click", "dblclick"]);
//#endregion
export { List as default };

//# sourceMappingURL=List.js.map