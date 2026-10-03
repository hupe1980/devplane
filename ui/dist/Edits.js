import { B as append, F as index, H as from_html, J as get, L as if_block, P as each, Q as template_effect, V as comment, dt as pop, ft as push, g as prop, ht as reset, it as sibling, nt as first_child, rt as only_child, tt as child, w as set_attribute, z as set_text } from "./api.js";
//#region src/surfaces/why/Edits.svelte
var root = from_html(`<span class="del svelte-14nczdo"> </span> `, 1);
var root_1 = from_html(`<span class="add svelte-14nczdo"> </span> `, 1);
var root_2 = from_html(`<p class="cut svelte-14nczdo"> </p>`);
var root_3 = from_html(`<figure class="svelte-14nczdo"><figcaption class="svelte-14nczdo"> </figcaption> <pre class="svelte-14nczdo"><!><!></pre> <!></figure>`);
var root_4 = from_html(`<section class="edits svelte-14nczdo" aria-label="the agent's report of its edit"><p class="says svelte-14nczdo"> </p> <!></section>`);
var root_5 = from_html(`<p class="none svelte-14nczdo"> </p>`);
function Edits($$anchor, $$props) {
	push($$props, true);
	let none = prop($$props, "none", 3, null);
	const lines = (text, mark) => (text ?? "").replace(/\n$/, "").split("\n").filter((_, __, all) => all.length > 1 || all[0] !== "").map((l) => `${mark} ${l}`);
	const short = (p) => p.split("/").slice(-3).join("/");
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var section = root_4();
		var p_1 = child(section);
		var text_1 = only_child(p_1);
		var node_1 = sibling(p_1, 2);
		each(node_1, 19, () => $$props.edits, (e, i) => `${e.path}-${i}`, ($$anchor, e) => {
			var figure = root_3();
			var figcaption = child(figure);
			var text_2 = only_child(figcaption);
			var pre = sibling(figcaption, 2);
			var node_2 = child(pre);
			each(node_2, 17, () => lines(get(e).old_text, "-"), index, ($$anchor, l) => {
				var fragment_1 = root();
				var span = first_child(fragment_1);
				var text_3 = only_child(span, true);
				var text_4 = sibling(span, 1, true);
				text_4.nodeValue = "\n";
				template_effect(() => set_text(text_3, get(l)));
				append($$anchor, fragment_1);
			});
			var node_3 = sibling(node_2);
			each(node_3, 17, () => lines(get(e).new_text, "+"), index, ($$anchor, l) => {
				var fragment_2 = root_1();
				var span_1 = first_child(fragment_2);
				var text_5 = only_child(span_1, true);
				var text_6 = sibling(span_1, 1, true);
				text_6.nodeValue = "\n";
				template_effect(() => set_text(text_5, get(l)));
				append($$anchor, fragment_2);
			});
			reset(pre);
			var node_4 = sibling(pre, 2);
			var consequent = ($$anchor) => {
				var p_2 = root_2();
				var text_7 = only_child(p_2);
				template_effect(() => set_text(text_7, `${get(e).omitted_bytes ?? ""} bytes not kept`));
				append($$anchor, p_2);
			};
			if_block(node_4, ($$render) => {
				if ((get(e).omitted_bytes ?? 0) > 0) $$render(consequent);
			});
			reset(figure);
			template_effect(($0) => {
				set_attribute(figcaption, "title", get(e).path);
				set_text(text_2, `${$0 ?? ""}${get(e).old_text == null ? " · new file" : ""}`);
			}, [() => short(get(e).path)]);
			append($$anchor, figure);
		});
		reset(section);
		template_effect(() => set_text(text_1, `${$$props.edits[0].says ?? "the agent's report of its edit" ?? ""} — the worktree, not this, is what the gates read`));
		append($$anchor, section);
	};
	var consequent_2 = ($$anchor) => {
		var p_3 = root_5();
		var text_8 = only_child(p_3, true);
		template_effect(() => set_text(text_8, none()));
		append($$anchor, p_3);
	};
	if_block(node, ($$render) => {
		if ($$props.edits && $$props.edits.length > 0) $$render(consequent_1);
		else if (none()) $$render(consequent_2, 1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
export { Edits as t };

//# sourceMappingURL=Edits.js.map