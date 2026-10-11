import { B as append, H as from_html, J as get, L as if_block, Q as template_effect, V as comment, ct as user_derived, dt as pop, ft as push, g as prop, ht as reset, it as sibling, nt as first_child, rt as only_child, tt as child, w as set_attribute, z as set_text } from "./api.js";
import { n as Icon, t as Pill } from "./app.js";
import { t as Qualifier } from "./Qualifier.js";
//#region src/lib/ChangeState.svelte
var weakLeads = (q) => !!q && q.says !== "" && q.unseen > 0;
var root = from_html(`<span class="lead svelte-p2t6jw"><span class="weak svelte-p2t6jw"><!> </span><span class="sep svelte-p2t6jw" aria-hidden="true">·</span><span class="sub svelte-p2t6jw"> </span></span>`);
var root_1 = from_html(`<!><!>`, 1);
function ChangeState($$anchor, $$props) {
	push($$props, true);
	let q = prop($$props, "q", 3, null);
	const leads = user_derived(() => weakLeads(q()));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var span = root();
		var span_1 = child(span);
		var node_1 = child(span_1);
		Icon(node_1, {
			name: "alert",
			size: 12
		});
		var text = sibling(node_1, 1, true);
		reset(span_1);
		var span_2 = sibling(span_1, 2);
		var text_1 = only_child(span_2, true);
		reset(span);
		template_effect(() => {
			set_attribute(span, "title", `${q().says ?? ""} — ${q().unseen ?? ""} not yet read; offering waits for every one`);
			set_text(text, q().says);
			set_text(text_1, $$props.state);
		});
		append($$anchor, span);
	};
	var alternate = ($$anchor) => {
		var fragment_1 = root_1();
		var node_2 = first_child(fragment_1);
		Pill(node_2, { get word() {
			return $$props.state;
		} });
		var node_3 = sibling(node_2);
		Qualifier(node_3, { get q() {
			return q();
		} });
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (get(leads) && q()) $$render(consequent);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
export { weakLeads as n, ChangeState as t };

//# sourceMappingURL=ChangeState.js.map