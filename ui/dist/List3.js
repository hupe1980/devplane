import { $ as user_effect, B as append, C as remove_input_defaults, G as delegate, H as from_html, J as get, K as delegated, L as if_block, P as each, Q as template_effect, S as bind_value, V as comment, at as proxy, ct as user_derived, dt as pop, ft as push, g as prop, ht as reset, it as sibling, mt as next, nt as first_child, ot as set, rt as only_child, st as state, tt as child, w as set_attribute, z as set_text } from "./api.js";
import { a as Inline, n as Icon } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as ChangeState } from "./ChangeState.js";
import { n as specs, r as watch, t as key } from "./store.svelte.js";
//#region src/surfaces/plan/List.svelte
var root = from_html(`<span class="n svelte-16yypc0"> </span>`);
var root_1 = from_html(`<p class="quiet fail svelte-16yypc0">Could not read the specifications: <!>. Nothing here is current; <code>devplane doctor</code> tells more.</p>`);
var root_2 = from_html(`<div class="skel svelte-16yypc0"></div>`);
var root_3 = from_html(`<span class="state svelte-16yypc0"><!></span>`);
var root_4 = from_html(`<span class="boxes svelte-16yypc0"> </span>`);
var root_5 = from_html(`<span class="unread svelte-16yypc0">no task file · no progress claimed</span>`);
var root_6 = from_html(`<span class="wait svelte-16yypc0"><!> </span>`);
var root_7 = from_html(`<span class="wait svelte-16yypc0"><!> drifted</span>`);
var root_8 = from_html(`<button class="row svelte-16yypc0"><span class="title svelte-16yypc0"><!> </span> <!> <span class="meta svelte-16yypc0"><!> <!> <!></span></button>`);
var root_9 = from_html(`<button class="group svelte-16yypc0"><!> <!> <span> </span> <span class="n svelte-16yypc0"> </span></button> <!>`, 1);
var root_10 = from_html(`<p class="why svelte-16yypc0"><!></p>`);
var root_11 = from_html(`<div class="away svelte-16yypc0"><p class="group svelte-16yypc0"><!> <span> </span></p> <!></div>`);
var root_12 = from_html(`<details class="bare svelte-16yypc0"><summary class="svelte-16yypc0"><!> </summary> <p class="svelte-16yypc0"> </p> <p class="why svelte-16yypc0"><!></p></details>`);
var root_13 = from_html(`<p class="none svelte-16yypc0"> </p>`);
var root_14 = from_html(`<div class="list svelte-16yypc0"><header class="svelte-16yypc0"><span class="t svelte-16yypc0">Specifications</span><!></header> <label class="filter svelte-16yypc0"><!> <input placeholder="Filter specifications" aria-label="filter the specifications" class="svelte-16yypc0"/></label> <div class="rows svelte-16yypc0"><!> <!> <!> <!> <!></div></div>`);
function List($$anchor, $$props) {
	push($$props, true);
	let focus = prop($$props, "focus", 3, "");
	user_effect(watch);
	let filter = state("");
	let folded = state(proxy({}));
	const name = (path) => path.split("/").filter(Boolean).pop() ?? path;
	const projects = user_derived(() => (specs.projects ?? []).map((p) => ({
		...p,
		plans: p.plans.filter((r) => !get(filter).trim() || `${r.plan.path} ${r.title ?? ""}`.toLowerCase().includes(get(filter).trim().toLowerCase()))
	})).filter((p) => p.plans.length > 0));
	const away = user_derived(() => (specs.projects ?? []).filter((p) => p.plans.length === 0 && (p.layouts ?? []).some((l) => l.elsewhere)));
	const bare = user_derived(() => (specs.projects ?? []).filter((p) => p.plans.length === 0 && !get(away).includes(p)));
	const sentence = user_derived(() => get(bare)[0]?.no_layout ?? "");
	const total = user_derived(() => (specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
	const flat = user_derived(() => get(projects).flatMap((p) => get(folded)[p.project_id] ? [] : p.plans.map((r) => key(p, r))));
	user_effect(() => cursor({
		scope: "plan",
		size: () => get(flat).length,
		at: () => get(flat).indexOf(focus()),
		go: (i) => {
			$$props.open(get(flat)[i]);
			queueMicrotask(() => document.querySelector(`[data-spec="${CSS.escape(get(flat)[i])}"]`)?.scrollIntoView({ block: "nearest" }));
		},
		open: (i) => $$props.open(get(flat)[i], true)
	}));
	var div = root_14();
	var header = child(div);
	var node = sibling(child(header));
	var consequent = ($$anchor) => {
		var span = root();
		var text = only_child(span, true);
		template_effect(() => set_text(text, get(total)));
		append($$anchor, span);
	};
	if_block(node, ($$render) => {
		if (specs.projects !== null) $$render(consequent);
	});
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
	var consequent_1 = ($$anchor) => {
		var p_1 = root_1();
		var node_3 = sibling(child(p_1));
		Inline(node_3, { get text() {
			return specs.error;
		} });
		next(3);
		reset(p_1);
		append($$anchor, p_1);
	};
	var consequent_2 = ($$anchor) => {
		var fragment = comment();
		var node_4 = first_child(fragment);
		each(node_4, 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			var div_2 = root_2();
			append($$anchor, div_2);
		});
		append($$anchor, fragment);
	};
	if_block(node_2, ($$render) => {
		if (specs.error) $$render(consequent_1);
		else if (specs.projects === null) $$render(consequent_2, 1);
	});
	var node_5 = sibling(node_2, 2);
	each(node_5, 17, () => get(projects), (p) => p.project_id, ($$anchor, p) => {
		var fragment_1 = root_9();
		var button = first_child(fragment_1);
		var node_6 = child(button);
		{
			let $0 = user_derived(() => get(folded)[get(p).project_id] ? "right" : "down");
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
		var text_1 = only_child(span_1, true);
		var span_2 = sibling(span_1, 2);
		var text_2 = only_child(span_2, true);
		reset(button);
		var node_8 = sibling(button, 2);
		var consequent_8 = ($$anchor) => {
			var fragment_2 = comment();
			var node_9 = first_child(fragment_2);
			each(node_9, 17, () => get(p).plans, (r) => r.plan.path, ($$anchor, r) => {
				const k = user_derived(() => key(get(p), get(r)));
				var button_1 = root_8();
				var span_3 = child(button_1);
				var node_10 = child(span_3);
				Icon(node_10, {
					name: "spec",
					size: 13
				});
				var text_3 = sibling(node_10);
				reset(span_3);
				var node_11 = sibling(span_3, 2);
				var consequent_3 = ($$anchor) => {
					var span_4 = root_3();
					var node_12 = child(span_4);
					ChangeState(node_12, {
						get state() {
							return get(r).state;
						},
						get q() {
							return get(r).qualifier;
						}
					});
					reset(span_4);
					template_effect(($0) => set_attribute(span_4, "title", $0), [() => [
						get(r).title,
						get(r).state,
						get(r).qualifier?.says
					].filter(Boolean).join(" · ")]);
					append($$anchor, span_4);
				};
				if_block(node_11, ($$render) => {
					if (get(r).state) $$render(consequent_3);
				});
				var span_5 = sibling(node_11, 2);
				var node_13 = child(span_5);
				var consequent_4 = ($$anchor) => {
					var span_6 = root_4();
					var text_4 = only_child(span_6);
					template_effect(() => {
						set_attribute(span_6, "title", `boxes ticked in the task file · ${get(r).plan.progress.total ?? ""} tasks`);
						set_text(text_4, `${get(r).plan.progress.total ?? ""} tasks · ${get(r).plan.progress.done ?? ""} ticked`);
					});
					append($$anchor, span_6);
				};
				var consequent_5 = ($$anchor) => {
					var span_7 = root_5();
					template_effect(() => set_attribute(span_7, "title", get(r).plan.no_progress));
					append($$anchor, span_7);
				};
				if_block(node_13, ($$render) => {
					if (get(r).plan.progress) $$render(consequent_4);
					else if (get(r).plan.no_progress) $$render(consequent_5, 1);
				});
				var node_14 = sibling(node_13, 2);
				var consequent_6 = ($$anchor) => {
					var span_8 = root_6();
					var node_15 = child(span_8);
					Icon(node_15, {
						name: "question",
						size: 11
					});
					var text_5 = sibling(node_15);
					reset(span_8);
					template_effect(() => set_text(text_5, ` ${get(r).plan.open_questions ?? ""}`));
					append($$anchor, span_8);
				};
				if_block(node_14, ($$render) => {
					if (get(r).plan.open_questions > 0) $$render(consequent_6);
				});
				var node_16 = sibling(node_14, 2);
				var consequent_7 = ($$anchor) => {
					var span_9 = root_7();
					var node_17 = child(span_9);
					Icon(node_17, {
						name: "alert",
						size: 11
					});
					next();
					reset(span_9);
					append($$anchor, span_9);
				};
				if_block(node_16, ($$render) => {
					if (get(r).drifted) $$render(consequent_7);
				});
				reset(span_5);
				reset(button_1);
				template_effect(($0) => {
					set_attribute(button_1, "data-spec", get(k));
					set_attribute(button_1, "aria-current", get(k) === focus() ? "true" : void 0);
					set_attribute(span_3, "title", get(r).plan.path);
					set_text(text_3, ` ${$0 ?? ""}`);
				}, [() => name(get(r).plan.path)]);
				delegated("click", button_1, () => $$props.open(get(k)));
				delegated("dblclick", button_1, () => $$props.open(get(k), true));
				append($$anchor, button_1);
			});
			append($$anchor, fragment_2);
		};
		if_block(node_8, ($$render) => {
			if (!get(folded)[get(p).project_id]) $$render(consequent_8);
		});
		template_effect(() => {
			set_attribute(button, "aria-expanded", !get(folded)[get(p).project_id]);
			set_text(text_1, get(p).project);
			set_text(text_2, get(p).plans.length);
		});
		delegated("click", button, () => set(folded, {
			...get(folded),
			[get(p).project_id]: !get(folded)[get(p).project_id]
		}, true));
		append($$anchor, fragment_1);
	});
	var node_18 = sibling(node_5, 2);
	var consequent_9 = ($$anchor) => {
		var fragment_3 = comment();
		var node_19 = first_child(fragment_3);
		each(node_19, 17, () => get(away), (p) => p.project_id, ($$anchor, p) => {
			var div_3 = root_11();
			var p_2 = child(div_3);
			var node_20 = child(p_2);
			Icon(node_20, {
				name: "folder",
				size: 13
			});
			var span_10 = sibling(node_20, 2);
			var text_6 = only_child(span_10, true);
			reset(p_2);
			var node_21 = sibling(p_2, 2);
			each(node_21, 17, () => (get(p).layouts ?? []).filter((l) => l.elsewhere), (l) => l.root, ($$anchor, l) => {
				var p_3 = root_10();
				var node_22 = child(p_3);
				{
					let $0 = user_derived(() => get(l).elsewhere ?? "");
					Inline(node_22, { get text() {
						return get($0);
					} });
				}
				reset(p_3);
				append($$anchor, p_3);
			});
			reset(div_3);
			template_effect(() => set_text(text_6, get(p).project));
			append($$anchor, div_3);
		});
		append($$anchor, fragment_3);
	};
	var d = user_derived(() => !get(filter).trim());
	if_block(node_18, ($$render) => {
		if (get(d)) $$render(consequent_9);
	});
	var node_23 = sibling(node_18, 2);
	var consequent_10 = ($$anchor) => {
		var details = root_12();
		var summary = child(details);
		var node_24 = child(summary);
		Icon(node_24, {
			name: "folder",
			size: 13
		});
		var text_7 = sibling(node_24);
		reset(summary);
		var p_4 = sibling(summary, 2);
		var text_8 = only_child(p_4, true);
		var p_5 = sibling(p_4, 2);
		var node_25 = child(p_5);
		Inline(node_25, { get text() {
			return get(sentence);
		} });
		reset(p_5);
		reset(details);
		template_effect(($0) => {
			set_text(text_7, ` ${get(bare).length ?? ""} ${get(bare).length === 1 ? "project has" : "projects have"} no specification layout`);
			set_text(text_8, $0);
		}, [() => get(bare).map((p) => p.project).join(", ")]);
		append($$anchor, details);
	};
	var d_1 = user_derived(() => get(bare).length > 0 && !get(filter).trim());
	if_block(node_23, ($$render) => {
		if (get(d_1)) $$render(consequent_10);
	});
	var node_26 = sibling(node_23, 2);
	var consequent_11 = ($$anchor) => {
		var p_6 = root_13();
		var text_9 = only_child(p_6);
		template_effect(() => set_text(text_9, `${specs.omitted ?? ""} more not shown.`));
		append($$anchor, p_6);
	};
	if_block(node_26, ($$render) => {
		if (specs.omitted > 0) $$render(consequent_11);
	});
	reset(div_1);
	reset(div);
	bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click", "dblclick"]);
//#endregion
export { List as default };

//# sourceMappingURL=List3.js.map