import { $ as push, A as comment, E as if_block, G as sibling, H as child, L as get, O as set_text, Q as pop, U as first_child, W as only_child, Y as user_derived, j as from_html, k as append, m as set_attribute, nt as reset, o as prop, v as set_class, z as template_effect } from "./api.js";
//#region src/lib/Qualifier.svelte
var root = from_html(`<span class="unseen svelte-1lbadw3"> </span>`);
var root_1 = from_html(`<span class="unseen svelte-1lbadw3">(read)</span>`);
var root_2 = from_html(`<span><span class="sep svelte-1lbadw3" aria-hidden="true">·</span> <!></span>`);
function Qualifier($$anchor, $$props) {
	push($$props, true);
	let compact = prop($$props, "compact", 3, false);
	const shown = user_derived(() => !!$$props.q && $$props.q.says !== "");
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var span = root_2();
		let classes;
		var text = sibling(child(span));
		var node_1 = sibling(text);
		var consequent = ($$anchor) => {
			var span_1 = root();
			var text_1 = only_child(span_1);
			template_effect(() => set_text(text_1, `(${$$props.q.unseen ?? ""} unread)`));
			append($$anchor, span_1);
		};
		var alternate = ($$anchor) => {
			var span_2 = root_1();
			append($$anchor, span_2);
		};
		if_block(node_1, ($$render) => {
			if ($$props.q.unseen > 0) $$render(consequent);
			else $$render(alternate, -1);
		});
		reset(span);
		template_effect(() => {
			classes = set_class(span, 1, "qualifier svelte-1lbadw3", null, classes, { compact: compact() });
			set_attribute(span, "title", $$props.q.unseen > 0 ? `${$$props.q.says} — ${$$props.q.unseen} not yet read` : `${$$props.q.says} — every one marked read`);
			set_text(text, ` ${$$props.q.says ?? ""}`);
		});
		append($$anchor, span);
	};
	if_block(node, ($$render) => {
		if (get(shown) && $$props.q) $$render(consequent_1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
export { Qualifier as t };

//# sourceMappingURL=Qualifier.js.map