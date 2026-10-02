import { $ as push, A as comment, B as user_effect, C as each, E as if_block, F as delegated, G as sibling, H as child, J as state, K as proxy, L as get, O as set_text, P as delegate, Q as pop, U as first_child, W as only_child, Y as user_derived, j as from_html, k as append, m as set_attribute, nt as reset, o as prop, q as set, tt as next, v as set_class, z as template_effect } from "./api.js";
import { _ as address, a as Inline, n as Icon, s as ago, v as itemFocus, y as place } from "./app.js";
import { t as narrowed } from "./narrow.svelte.js";
//#region src/surfaces/inbox/List.svelte
var root = from_html(`<p class="since svelte-i436af"><!> </p>`);
var root_1 = from_html(`<button> </button>`);
var root_2 = from_html(`<div class="chips svelte-i436af" role="group" aria-label="narrow to one project"><button>all</button> <!></div>`);
var root_3 = from_html(`<p class="quiet failed svelte-i436af"> <!>.</p>`);
var root_4 = from_html(`<p class="quiet failed svelte-i436af"> </p>`);
var root_5 = from_html(`<div class="skel svelte-i436af" aria-busy="true"></div>`);
var root_6 = from_html(`<p class="quiet svelte-i436af">The last read had nothing for you, and Devplane has not answered since.</p>`);
var root_7 = from_html(`<p class="quiet svelte-i436af"> </p>`);
var root_8 = from_html(`<span class="new svelte-i436af">new</span>`);
var root_9 = from_html(`<span class="weak svelte-i436af"> </span>`);
var root_10 = from_html(`<div role="option" tabindex="-1"><span class="ic svelte-i436af"><!></span> <span class="title svelte-i436af"> </span> <!> <span class="meta svelte-i436af"> <!> </span> <span class="age svelte-i436af"> </span></div>`);
var root_11 = from_html(`<div class="folded svelte-i436af"><!> <span class="svelte-i436af"> </span></div>`);
var root_12 = from_html(`<div class="list svelte-i436af"><header class="svelte-i436af"><span class="t svelte-i436af">What needs you</span> <span class="n svelte-i436af"> </span></header> <!> <!> <div class="rows svelte-i436af" role="listbox" aria-label="what needs you" tabindex="0"><!> <!> <!> <!></div></div>`);
function List($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 19, () => []), folded = prop($$props, "folded", 19, () => []), inhibited = prop($$props, "inhibited", 19, () => []), close = prop($$props, "close", 3, null), project = prop($$props, "project", 3, ""), focus = prop($$props, "focus", 3, ""), loaded = prop($$props, "loaded", 3, false), error = prop($$props, "error", 3, null);
	const narrow = narrowed(() => project());
	const projects = user_derived(() => [...new Set(items().map((i) => i.project_name).filter((p) => !!p))]);
	const NONE = {
		items: [],
		folded: [],
		inhibited: []
	};
	const narrowedFeed = user_derived(() => narrow.on ? narrow.data ?? NONE : null);
	const shown = user_derived(() => get(narrowedFeed)?.items ?? items());
	const shownFolded = user_derived(() => get(narrowedFeed)?.folded ?? folded());
	const shownInhibited = user_derived(() => get(narrowedFeed)?.inhibited ?? inhibited());
	const ready = user_derived(() => narrow.on ? narrow.phase !== "loading" : loaded());
	const nothingRaised = user_derived(() => get(shown).length === 0 && get(shownFolded).length === 0 && get(shownInhibited).length === 0);
	const missing = user_derived(() => narrow.missing);
	const selected = user_derived(() => place(get(shown), address(focus(), items())).current?.id ?? "");
	const optionId = (id) => `inbox-row-${id.replace(/[^\w-]/g, "_")}`;
	const icon = (k) => k === "permission" ? "shield" : k === "question" ? "question" : k.includes("gate") || k.includes("fail") ? "x" : k.includes("ready") ? "decide" : k.includes("conflict") ? "alert" : k.includes("context") ? "clock" : "dot";
	let now = state(proxy(Date.now()));
	user_effect(() => {
		const t = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(t);
	});
	const age = (since) => since ? ago(Math.max(0, (get(now) - Date.parse(since)) / 1e3)) : "";
	var div = root_12();
	var header = child(div);
	var span = sibling(child(header), 2);
	var text = only_child(span, true);
	reset(header);
	var node = sibling(header, 2);
	var consequent = ($$anchor) => {
		var p_1 = root();
		var node_1 = child(p_1);
		Icon(node_1, {
			name: "clock",
			size: 12
		});
		var text_1 = sibling(node_1);
		reset(p_1);
		template_effect(() => set_text(text_1, ` since you last looked · ${close().since_last_look ?? ""}`));
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if (close()?.since_last_look) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var div_1 = root_2();
		var button = child(div_1);
		let classes;
		var node_3 = sibling(button, 2);
		each(node_3, 16, () => get(projects), (p) => p, ($$anchor, p) => {
			var button_1 = root_1();
			let classes_1;
			var text_2 = only_child(button_1, true);
			template_effect(() => {
				set_attribute(button_1, "aria-pressed", project() === p);
				classes_1 = set_class(button_1, 1, "svelte-i436af", null, classes_1, { on: project() === p });
				set_text(text_2, p);
			});
			delegated("click", button_1, () => $$props.open(project() === p ? "" : p));
			append($$anchor, button_1);
		});
		reset(div_1);
		template_effect(() => {
			set_attribute(button, "aria-pressed", !project());
			classes = set_class(button, 1, "svelte-i436af", null, classes, { on: !project() });
		});
		delegated("click", button, () => $$props.open(""));
		append($$anchor, div_1);
	};
	if_block(node_2, ($$render) => {
		if (get(projects).length > 1 || project()) $$render(consequent_1);
	});
	var div_2 = sibling(node_2, 2);
	var node_4 = child(div_2);
	var consequent_2 = ($$anchor) => {
		var p_2 = root_3();
		var text_3 = child(p_2);
		var node_5 = sibling(text_3);
		Inline(node_5, { get text() {
			return narrow.failure.says;
		} });
		next();
		reset(p_2);
		template_effect(() => set_text(text_3, `Could not read the inbox narrowed to ${project() ?? ""}: `));
		append($$anchor, p_2);
	};
	var consequent_3 = ($$anchor) => {
		var p_3 = root_4();
		var text_4 = only_child(p_3);
		template_effect(() => set_text(text_4, `No project is named “${project() ?? ""}”.`));
		append($$anchor, p_3);
	};
	var consequent_4 = ($$anchor) => {
		var fragment = comment();
		var node_6 = first_child(fragment);
		each(node_6, 16, () => [
			0,
			1,
			2
		], (i) => i, ($$anchor, i) => {
			var div_3 = root_5();
			append($$anchor, div_3);
		});
		append($$anchor, fragment);
	};
	var consequent_5 = ($$anchor) => {
		var p_4 = root_6();
		append($$anchor, p_4);
	};
	var consequent_6 = ($$anchor) => {
		var p_5 = root_7();
		var text_5 = only_child(p_5, true);
		template_effect(() => set_text(text_5, project() ? `Nothing needs you in ${project()}.` : "Nothing needs you."));
		append($$anchor, p_5);
	};
	if_block(node_4, ($$render) => {
		if (narrow.on && narrow.failure && !narrow.data) $$render(consequent_2);
		else if (get(missing)) $$render(consequent_3, 1);
		else if (!get(ready) && get(shown).length === 0) $$render(consequent_4, 2);
		else if (get(nothingRaised) && error()) $$render(consequent_5, 3);
		else if (get(nothingRaised)) $$render(consequent_6, 4);
	});
	var node_7 = sibling(node_4, 2);
	each(node_7, 17, () => get(shown), (i) => i.id, ($$anchor, i) => {
		var div_4 = root_10();
		let classes_2;
		var span_1 = child(div_4);
		var node_8 = child(span_1);
		{
			let $0 = user_derived(() => icon(get(i).kind));
			Icon(node_8, {
				get name() {
					return get($0);
				},
				size: 14
			});
		}
		reset(span_1);
		var span_2 = sibling(span_1, 2);
		var text_6 = only_child(span_2, true);
		var node_9 = sibling(span_2, 2);
		var consequent_7 = ($$anchor) => {
			var span_3 = root_8();
			append($$anchor, span_3);
		};
		if_block(node_9, ($$render) => {
			if (get(i).new_to_you) $$render(consequent_7);
		});
		var span_4 = sibling(node_9, 2);
		var text_7 = child(span_4, true);
		var node_10 = sibling(text_7);
		var consequent_8 = ($$anchor) => {
			var span_5 = root_9();
			var text_8 = only_child(span_5, true);
			template_effect(() => set_text(text_8, ` · ${get(i).facts.qualifier.says}`));
			append($$anchor, span_5);
		};
		if_block(node_10, ($$render) => {
			if (get(i).facts?.qualifier?.says) $$render(consequent_8);
		});
		var text_9 = sibling(node_10, 1, true);
		reset(span_4);
		var span_6 = sibling(span_4, 2);
		var text_10 = only_child(span_6, true);
		reset(div_4);
		template_effect(($0, $1, $2) => {
			classes_2 = set_class(div_4, 1, "row svelte-i436af", null, classes_2, { high: get(i).level === "high" });
			set_attribute(div_4, "id", $0);
			set_attribute(div_4, "aria-selected", get(i).id === get(selected));
			set_text(text_6, get(i).title);
			set_text(text_7, $1);
			set_text(text_9, get(i).project_name ? ` · ${get(i).project_name}` : "");
			set_text(text_10, $2);
		}, [
			() => optionId(get(i).id),
			() => get(i).kind.replace(/_/g, " "),
			() => age(get(i).since)
		]);
		delegated("click", div_4, () => $$props.open(itemFocus(get(i).id)));
		delegated("keydown", div_4, (e) => {
			if (e.key === "Enter" || e.key === " ") {
				e.preventDefault();
				$$props.open(itemFocus(get(i).id));
			}
		});
		append($$anchor, div_4);
	});
	var node_11 = sibling(node_7, 2);
	each(node_11, 17, () => get(shownFolded), (f) => f.kind + (f.project ?? ""), ($$anchor, f) => {
		var div_5 = root_11();
		var node_12 = child(div_5);
		Icon(node_12, {
			name: "more",
			size: 13
		});
		var text_11 = sibling(node_12);
		var span_7 = sibling(text_11);
		var text_12 = only_child(span_7);
		reset(div_5);
		template_effect(($0) => {
			set_text(text_11, ` ${get(f).count ?? ""} × ${$0 ?? ""} `);
			set_text(text_12, `${get(f).project ?? "across projects" ?? ""} · folded`);
		}, [() => get(f).kind.replace(/_/g, " ")]);
		append($$anchor, div_5);
	});
	var node_13 = sibling(node_11, 2);
	each(node_13, 17, () => get(shownInhibited), (s) => s.cause, ($$anchor, s) => {
		var div_6 = root_11();
		var node_14 = child(div_6);
		Icon(node_14, {
			name: "more",
			size: 13
		});
		var text_13 = sibling(node_14);
		var span_8 = sibling(text_13);
		var text_14 = only_child(span_8, true);
		reset(div_6);
		template_effect(() => {
			set_text(text_13, ` ${get(s).count ?? ""} more counted `);
			set_text(text_14, get(s).because);
		});
		append($$anchor, div_6);
	});
	reset(div_2);
	reset(div);
	template_effect(($0) => {
		set_text(text, get(ready) && !get(missing) && !(narrow.on && narrow.failure && !narrow.data) ? get(shown).length : "");
		set_attribute(div_2, "aria-activedescendant", $0);
	}, [() => get(selected) ? optionId(get(selected)) : void 0]);
	append($$anchor, div);
	pop();
}
delegate(["click", "keydown"]);
//#endregion
export { List as default };

//# sourceMappingURL=List2.js.map