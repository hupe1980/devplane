import { C as each, E as if_block, F as delegated, K as only_child, L as get, O as set_text, P as delegate, V as template_effect, W as child, et as pop, it as reset, j as from_html, k as append, m as set_attribute, o as prop, q as sibling, tt as push, v as set_class } from "./api.js";
import { n as Icon } from "./app.js";
//#region src/lib/ui/Tabs.svelte
function step(ids, active, key) {
	if (ids.length === 0) return null;
	const i = Math.max(0, ids.indexOf(active));
	if (key === "ArrowRight") return ids[(i + 1) % ids.length];
	if (key === "ArrowLeft") return ids[(i - 1 + ids.length) % ids.length];
	if (key === "Home") return ids[0];
	if (key === "End") return ids[ids.length - 1];
	return null;
}
var root = from_html(`<span> </span>`);
var root_1 = from_html(`<button role="tab" class="tab svelte-1fwbyla"><!> <span> </span> <!></button>`);
var root_2 = from_html(`<div class="tabs svelte-1fwbyla" role="tablist" tabindex="-1"></div>`);
function Tabs($$anchor, $$props) {
	push($$props, true);
	let active = prop($$props, "active", 15), label = prop($$props, "label", 3, "views");
	function key(e) {
		const to = step($$props.tabs.map((t) => t.id), active(), e.key);
		if (to === null) return;
		e.preventDefault();
		active(to);
		const next = $$props.tabs.findIndex((t) => t.id === to);
		e.currentTarget.querySelectorAll("[role=tab]")[next]?.focus();
	}
	var div = root_2();
	each(div, 21, () => $$props.tabs, (t) => t.id, ($$anchor, t) => {
		var button = root_1();
		var node = child(button);
		var consequent = ($$anchor) => {
			Icon($$anchor, {
				get name() {
					return get(t).icon;
				},
				size: 14
			});
		};
		if_block(node, ($$render) => {
			if (get(t).icon) $$render(consequent);
		});
		var span = sibling(node, 2);
		var text = only_child(span, true);
		var node_1 = sibling(span, 2);
		var consequent_1 = ($$anchor) => {
			var span_1 = root();
			var text_1 = only_child(span_1, true);
			template_effect(() => {
				set_class(span_1, 1, `count ${get(t).tone ?? "" ?? ""}`, "svelte-1fwbyla");
				set_text(text_1, get(t).count);
			});
			append($$anchor, span_1);
		};
		if_block(node_1, ($$render) => {
			if (get(t).count != null) $$render(consequent_1);
		});
		reset(button);
		template_effect(() => {
			set_attribute(button, "aria-selected", get(t).id === active());
			set_attribute(button, "tabindex", get(t).id === active() ? 0 : -1);
			set_text(text, get(t).label);
		});
		delegated("click", button, () => active(get(t).id));
		append($$anchor, button);
	});
	reset(div);
	template_effect(() => set_attribute(div, "aria-label", label()));
	delegated("keydown", div, key);
	append($$anchor, div);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
export { Tabs as t };

//# sourceMappingURL=Tabs.js.map