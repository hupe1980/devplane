import { A as clsx, B as append, H as from_html, J as get, L as if_block, N as snippet, P as each, Q as template_effect, V as comment, ht as reset, it as sibling, k as set_class, nt as first_child, rt as only_child, tt as child, z as set_text } from "./api.js";
//#region src/lib/ui/Props.svelte
var root = from_html(`<dt class="svelte-1gth30k"> </dt> <dd> </dd>`, 1);
var root_1 = from_html(`<dl class="props svelte-1gth30k"><!> <!></dl>`);
function Props($$anchor, $$props) {
	var dl = root_1();
	var node = child(dl);
	each(node, 17, () => $$props.rows, (r) => r.label, ($$anchor, r) => {
		var fragment = root();
		var dt = first_child(fragment);
		var text = only_child(dt, true);
		var dd = sibling(dt, 2);
		let classes;
		var text_1 = only_child(dd, true);
		template_effect(() => {
			set_text(text, get(r).label);
			classes = set_class(dd, 1, clsx(get(r).tone ?? ""), "svelte-1gth30k", classes, {
				mono: get(r).mono,
				missing: get(r).value == null || get(r).value === ""
			});
			set_text(text_1, get(r).value == null || get(r).value === "" ? get(r).missing ?? "—" : get(r).value);
		});
		append($$anchor, fragment);
	});
	var node_1 = sibling(node, 2);
	var consequent = ($$anchor) => {
		var fragment_1 = comment();
		var node_2 = first_child(fragment_1);
		snippet(node_2, () => $$props.extra);
		append($$anchor, fragment_1);
	};
	if_block(node_1, ($$render) => {
		if ($$props.extra) $$render(consequent);
	});
	reset(dl);
	append($$anchor, dl);
}
//#endregion
export { Props as t };

//# sourceMappingURL=Props.js.map