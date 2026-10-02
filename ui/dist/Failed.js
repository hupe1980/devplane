import { A as comment, E as if_block, G as first_child, K as only_child, L as get, N as text, O as set_text, V as template_effect, W as child, Z as user_derived, et as pop, it as reset, j as from_html, k as append, o as prop, q as sibling, rt as next, tt as push, v as set_class } from "./api.js";
import { a as Inline, s as ago } from "./app.js";
//#region src/lib/Failed.svelte
var root = from_html(`<p role="status"><!></p>`);
var root_1 = from_html(` <!>.`, 1);
var root_2 = from_html(`<p role="alert"><!> <span class="tell svelte-fezoay"><code class="svelte-fezoay"> </code> tells more.</span></p>`);
function Failed($$anchor, $$props) {
	push($$props, true);
	let at = prop($$props, "at", 3, null), stale = prop($$props, "stale", 3, false);
	const What = user_derived(() => $$props.what.charAt(0).toUpperCase() + $$props.what.slice(1));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var p = root();
		let classes;
		var node_1 = child(p);
		var consequent = ($$anchor) => {
			var text$1 = text();
			template_effect(($0) => set_text(text$1, `${get(What) ?? ""} as read ${$0 ?? ""} — not read since.`), [() => at() ? `${ago((Date.now() - at()) / 1e3)} ago` : "earlier"]);
			append($$anchor, text$1);
		};
		var alternate = ($$anchor) => {
			var text_1 = text();
			template_effect(() => set_text(text_1, `${get(What) ?? ""}: not read.`));
			append($$anchor, text_1);
		};
		if_block(node_1, ($$render) => {
			if (stale()) $$render(consequent);
			else $$render(alternate, -1);
		});
		reset(p);
		template_effect(() => classes = set_class(p, 1, "failed down svelte-fezoay", null, classes, { stale: stale() }));
		append($$anchor, p);
	};
	var alternate_2 = ($$anchor) => {
		var p_1 = root_2();
		let classes_1;
		var node_2 = child(p_1);
		var consequent_2 = ($$anchor) => {
			var fragment_3 = root_1();
			var text_2 = first_child(fragment_3);
			var node_3 = sibling(text_2);
			Inline(node_3, { get text() {
				return $$props.failure.says;
			} });
			next();
			template_effect(($0) => set_text(text_2, `Showing ${$$props.what ?? ""} as read ${$0 ?? ""} — the re-read failed: `), [() => at() ? `${ago((Date.now() - at()) / 1e3)} ago` : "earlier"]);
			append($$anchor, fragment_3);
		};
		var alternate_1 = ($$anchor) => {
			var fragment_4 = root_1();
			var text_3 = first_child(fragment_4);
			var node_4 = sibling(text_3);
			Inline(node_4, { get text() {
				return $$props.failure.says;
			} });
			next();
			template_effect(() => set_text(text_3, `Could not read ${$$props.what ?? ""}: `));
			append($$anchor, fragment_4);
		};
		if_block(node_2, ($$render) => {
			if (stale()) $$render(consequent_2);
			else $$render(alternate_1, -1);
		});
		var span = sibling(node_2, 2);
		var code = child(span);
		var text_4 = only_child(code, true);
		next();
		reset(span);
		reset(p_1);
		template_effect(() => {
			classes_1 = set_class(p_1, 1, "failed svelte-fezoay", null, classes_1, { stale: stale() });
			set_text(text_4, $$props.failure.tell);
		});
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if ($$props.failure.down) $$render(consequent_1);
		else $$render(alternate_2, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
export { Failed as t };

//# sourceMappingURL=Failed.js.map