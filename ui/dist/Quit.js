import { $ as user_effect, B as append, G as delegate, H as from_html, J as get, K as delegated, L as if_block, Q as template_effect, ct as user_derived, d as onAction, dt as pop, ft as push, g as prop, ht as reset, i as api, it as sibling, ot as set, rt as only_child, st as state, tt as child, z as set_text } from "./api.js";
import { n as failure, r as resource } from "./resource.svelte.js";
import { t as hideWindow } from "./frame.js";
//#region src/surfaces/quit/Quit.svelte
var root = from_html(`<pre class="says svelte-1pz5pdz"> </pre>`);
var root_1 = from_html(`<p class="says warn svelte-1pz5pdz"> </p>`);
var root_2 = from_html(`<p class="dim svelte-1pz5pdz">reading what this would stop…</p>`);
var root_3 = from_html(`<section class="quit svelte-1pz5pdz" aria-labelledby="quit-head"><h2 id="quit-head" class="svelte-1pz5pdz">Quit Devplane?</h2> <!> <div class="acts svelte-1pz5pdz"><button class="primary"> </button> <button>Keep running</button></div></section>`);
function Quit($$anchor, $$props) {
	push($$props, true);
	let says = prop($$props, "says", 3, null);
	const read = resource(() => "/api/quitting", { tell: () => "devplane down" });
	let refused = state("");
	let stopping = state(false);
	const shown = user_derived(() => read.data ? read.data.says ?? "" : says());
	const unreadable = user_derived(() => get(refused) || (read.failure ? `Could not read what this would stop (${read.failure.says}). Quitting anyway ends anything it started.` : ""));
	user_effect(() => onAction("leave", () => {
		keep();
		return true;
	}));
	async function quit() {
		set(stopping, true);
		try {
			await api("/api/quit", { method: "POST" });
		} catch (e) {
			set(stopping, false);
			set(refused, `that did not land: ${failure(e).says}`);
		}
	}
	function keep() {
		hideWindow();
	}
	var section = root_3();
	var node = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var pre = root();
		var text = only_child(pre, true);
		template_effect(() => set_text(text, get(shown)));
		append($$anchor, pre);
	};
	var consequent_1 = ($$anchor) => {
		var p = root_1();
		var text_1 = only_child(p, true);
		template_effect(() => set_text(text_1, get(unreadable)));
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var p_1 = root_2();
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if (get(shown) !== null) $$render(consequent);
		else if (get(unreadable)) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	var div = sibling(node, 2);
	var button = child(div);
	var text_2 = only_child(button, true);
	var button_1 = sibling(button, 2);
	reset(div);
	reset(section);
	template_effect(() => {
		button.disabled = get(stopping);
		set_text(text_2, get(stopping) ? "stopping…" : "Quit");
		button_1.disabled = get(stopping);
	});
	delegated("click", button, quit);
	delegated("click", button_1, keep);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
export { Quit as default };

//# sourceMappingURL=Quit.js.map