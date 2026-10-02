import { A as comment, E as if_block, G as first_child, H as user_effect, I as event, J as proxy, K as only_child, L as get, N as text, O as set_text, V as template_effect, W as child, X as state, Y as set, Z as user_derived, at as noop, et as pop, f as bind_value, i as api, it as reset, j as from_html, k as append, m as set_attribute, o as prop, p as remove_input_defaults, q as sibling, tt as push, z as untrack } from "./api.js";
import { a as Inline, m as failure, n as Icon } from "./app.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
//#region src/surfaces/search/Search.svelte
var root = from_html(`<p class="fail svelte-1acjvix"><!></p>`);
var root_1 = from_html(`<a class="svelte-1acjvix"> </a>`);
var root_2 = from_html(`<span class="t svelte-1acjvix"> </span>`);
var root_3 = from_html(`<div class="frame svelte-1acjvix"><!></div>`);
var root_4 = from_html(`<div class="page svelte-1acjvix"><h1 class="sr-only">Search every session</h1> <form class="bar svelte-1acjvix"><label class="field svelte-1acjvix"><!> <input type="search" aria-label="search every session" placeholder="A command, a question, an error" class="svelte-1acjvix"/></label> <button type="submit"> </button></form> <p class="note svelte-1acjvix">Tool calls, questions and errors from every session. Prompts and replies are never recorded, so they are never found.</p> <!> <!></div>`);
function Search($$anchor, $$props) {
	push($$props, true);
	let q = prop($$props, "q", 3, "");
	let query = state("");
	let hits = state(proxy([]));
	let ran = state(false);
	let ranFor = state("");
	let busy = state(false);
	let said = state("");
	let selected = state(null);
	const address = user_derived(q);
	user_effect(() => {
		const want = get(address);
		untrack(() => {
			if (want) {
				set(query, want, true);
				run();
			}
		});
	});
	let gen = 0;
	async function run() {
		const want = get(query).trim();
		if (!want) return;
		const mine = ++gen;
		set(busy, true);
		try {
			const r = await api(`/api/search?q=${encodeURIComponent(want)}`);
			if (mine !== gen) return;
			set(hits, r.hits ?? [], true);
			set(ran, true);
			set(ranFor, want, true);
			set(said, "");
		} catch (e) {
			if (mine !== gen) return;
			const f = failure(e, `devplane search ${want}`);
			set(said, `The search did not run: ${f.says}. \`${f.tell}\` tells more.`);
		} finally {
			if (mine === gen) set(busy, false);
		}
	}
	const columns = [
		{
			key: "at",
			label: "When",
			width: 150,
			sort: (h) => h.at,
			mono: true
		},
		{
			key: "run",
			label: "Session",
			width: 150,
			mono: true
		},
		{
			key: "text",
			label: "What matched"
		}
	];
	var div = root_4();
	var form = sibling(child(div), 2);
	var label = child(form);
	var node = child(label);
	Icon(node, {
		name: "search",
		size: 15
	});
	var input = sibling(node, 2);
	remove_input_defaults(input);
	reset(label);
	var button = sibling(label, 2);
	var text$1 = only_child(button, true);
	reset(form);
	var node_1 = sibling(form, 4);
	var consequent = ($$anchor) => {
		var p = root();
		var node_2 = child(p);
		Inline(node_2, { get text() {
			return get(said);
		} });
		reset(p);
		append($$anchor, p);
	};
	if_block(node_1, ($$render) => {
		if (get(said)) $$render(consequent);
	});
	var node_3 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		{
			let $0 = user_derived(() => `Nothing any session recorded contains “${get(ranFor)}”.`);
			Empty($$anchor, {
				icon: "search",
				title: "Nothing matched",
				get body() {
					return get($0);
				}
			});
		}
	};
	var consequent_4 = ($$anchor) => {
		var div_1 = root_3();
		var node_4 = child(div_1);
		{
			const cell = ($$anchor, h = noop, c = noop) => {
				var fragment_1 = comment();
				var node_5 = first_child(fragment_1);
				var consequent_2 = ($$anchor) => {
					var text_1 = text();
					template_effect(($0) => set_text(text_1, $0), [() => new Date(h().at).toLocaleString()]);
					append($$anchor, text_1);
				};
				var consequent_3 = ($$anchor) => {
					var a = root_1();
					var text_2 = only_child(a, true);
					template_effect(($0, $1) => {
						set_attribute(a, "href", $0);
						set_text(text_2, $1);
					}, [() => `#why/${encodeURIComponent(h().run_id)}`, () => h().run_id.slice(0, 14)]);
					append($$anchor, a);
				};
				var alternate = ($$anchor) => {
					var span = root_2();
					var text_3 = only_child(span, true);
					template_effect(() => set_text(text_3, h().text));
					append($$anchor, span);
				};
				if_block(node_5, ($$render) => {
					if (c().key === "at") $$render(consequent_2);
					else if (c().key === "run") $$render(consequent_3, 1);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_1);
			};
			Grid(node_4, {
				id: "search",
				get columns() {
					return columns;
				},
				get rows() {
					return get(hits);
				},
				key: (h) => h.run_id + h.at + h.text,
				label: "matches",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				$$slots: { cell: true }
			});
		}
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_3, ($$render) => {
		if (get(ran) && get(hits).length === 0 && !get(busy)) $$render(consequent_1);
		else if (get(hits).length > 0) $$render(consequent_4, 1);
	});
	reset(div);
	template_effect(() => {
		button.disabled = get(busy);
		set_text(text$1, get(busy) ? "Searching…" : "Search");
	});
	event("submit", form, (e) => {
		e.preventDefault();
		run();
	});
	bind_value(input, () => get(query), ($$value) => set(query, $$value));
	append($$anchor, div);
	pop();
}
//#endregion
export { Search as default };

//# sourceMappingURL=Search.js.map