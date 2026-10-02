import { B as append, G as delegate, H as from_html, J as get, K as delegated, L as if_block, Q as template_effect, V as comment, W as text, ct as user_derived, dt as pop, ft as push, g as prop, ht as reset, it as sibling, k as set_class, mt as next, nt as first_child, ot as set, rt as only_child, st as state, tt as child, z as set_text } from "./api.js";
import { a as Inline, s as ago } from "./app.js";
//#region src/lib/Failed.svelte
var root = from_html(`<p role="status"><!></p>`);
var root_1 = from_html(` <!>.`, 1);
var root_2 = from_html(` <!>. Nothing on this page is current.`, 1);
var root_3 = from_html(`<button type="button"> </button>`);
var root_4 = from_html(`<details><summary class="svelte-fezoay">raw error</summary> <pre data-quoted="" class="svelte-fezoay"> </pre></details>`);
var root_5 = from_html(`<div class="more svelte-fezoay"><!> <!></div>`);
var root_6 = from_html(`<div role="alert"><p class="svelte-fezoay"><!> <span class="tell svelte-fezoay"><code class="svelte-fezoay"> </code> tells more.</span></p> <!></div>`);
function Failed($$anchor, $$props) {
	push($$props, true);
	let at = prop($$props, "at", 3, null), stale = prop($$props, "stale", 3, false), retry = prop($$props, "retry", 3, void 0);
	const What = user_derived(() => $$props.what.charAt(0).toUpperCase() + $$props.what.slice(1));
	const again = user_derived(() => retry() ?? $$props.failure.retry);
	let retrying = state(false);
	async function doRetry() {
		if (!get(again) || get(retrying)) return;
		set(retrying, true);
		try {
			await get(again)();
		} finally {
			set(retrying, false);
		}
	}
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
		var div = root_6();
		let classes_1;
		var p_1 = child(div);
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
			var fragment_4 = root_2();
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
		var node_5 = sibling(p_1, 2);
		var consequent_5 = ($$anchor) => {
			var div_1 = root_5();
			var node_6 = child(div_1);
			var consequent_3 = ($$anchor) => {
				var button = root_3();
				var text_5 = only_child(button, true);
				template_effect(() => {
					button.disabled = get(retrying);
					set_text(text_5, get(retrying) ? "Reading…" : "Retry");
				});
				delegated("click", button, doRetry);
				append($$anchor, button);
			};
			if_block(node_6, ($$render) => {
				if (get(again)) $$render(consequent_3);
			});
			var node_7 = sibling(node_6, 2);
			var consequent_4 = ($$anchor) => {
				var details = root_4();
				var pre = sibling(child(details), 2);
				var text_6 = only_child(pre, true);
				reset(details);
				template_effect(() => set_text(text_6, $$props.failure.raw));
				append($$anchor, details);
			};
			if_block(node_7, ($$render) => {
				if ($$props.failure.raw) $$render(consequent_4);
			});
			reset(div_1);
			append($$anchor, div_1);
		};
		if_block(node_5, ($$render) => {
			if (get(again) || $$props.failure.raw) $$render(consequent_5);
		});
		reset(div);
		template_effect(() => {
			classes_1 = set_class(div, 1, "failed svelte-fezoay", null, classes_1, { stale: stale() });
			set_text(text_4, $$props.failure.tell);
		});
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if ($$props.failure.down) $$render(consequent_1);
		else $$render(alternate_2, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
export { Failed as t };

//# sourceMappingURL=Failed.js.map