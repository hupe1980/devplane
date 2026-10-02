import { A as append, C as snippet, D as if_block, G as child, J as sibling, M as from_html, at as reset, o as prop } from "./api.js";
import { a as Inline, n as Icon } from "./app.js";
//#region src/lib/ui/Empty.svelte
var root = from_html(`<p class="body svelte-x83d2f"><!></p>`);
var root_1 = from_html(`<p class="limit svelte-x83d2f"><!> <!></p>`);
var root_2 = from_html(`<div class="action svelte-x83d2f"><!></div>`);
var root_3 = from_html(`<div class="empty svelte-x83d2f"><span class="glyph svelte-x83d2f"><!></span> <p class="title svelte-x83d2f"><!></p> <!> <!> <!></div>`);
function Empty($$anchor, $$props) {
	let icon = prop($$props, "icon", 3, "dot"), body = prop($$props, "body", 3, ""), limit = prop($$props, "limit", 3, "");
	var div = root_3();
	var span = child(div);
	var node = child(span);
	Icon(node, {
		get name() {
			return icon();
		},
		size: 22
	});
	reset(span);
	var p = sibling(span, 2);
	var node_1 = child(p);
	Inline(node_1, { get text() {
		return $$props.title;
	} });
	reset(p);
	var node_2 = sibling(p, 2);
	var consequent = ($$anchor) => {
		var p_1 = root();
		var node_3 = child(p_1);
		Inline(node_3, { get text() {
			return body();
		} });
		reset(p_1);
		append($$anchor, p_1);
	};
	if_block(node_2, ($$render) => {
		if (body()) $$render(consequent);
	});
	var node_4 = sibling(node_2, 2);
	var consequent_1 = ($$anchor) => {
		var p_2 = root_1();
		var node_5 = child(p_2);
		Icon(node_5, {
			name: "eye",
			size: 13
		});
		var node_6 = sibling(node_5, 2);
		Inline(node_6, { get text() {
			return limit();
		} });
		reset(p_2);
		append($$anchor, p_2);
	};
	if_block(node_4, ($$render) => {
		if (limit()) $$render(consequent_1);
	});
	var node_7 = sibling(node_4, 2);
	var consequent_2 = ($$anchor) => {
		var div_1 = root_2();
		var node_8 = child(div_1);
		snippet(node_8, () => $$props.action);
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_7, ($$render) => {
		if ($$props.action) $$render(consequent_2);
	});
	reset(div);
	append($$anchor, div);
}
//#endregion
export { Empty as t };

//# sourceMappingURL=Empty.js.map