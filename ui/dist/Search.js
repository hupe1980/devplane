import { A as append, B as untrack, D as if_block, G as child, H as template_effect, J as sibling, K as first_child, L as event, M as from_html, P as text, Q as user_derived, R as get, U as user_effect, X as set, Y as proxy, Z as state, at as reset, f as bind_value, i as api, j as comment, k as set_text, m as set_attribute, nt as push, o as prop, ot as noop, p as remove_input_defaults, q as only_child, tt as pop } from "./api.js";
import { O as go, a as Inline, m as failure, n as Icon } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
//#region src/surfaces/search/Search.svelte
var root = from_html(`<p class="fail svelte-1acjvix"><!></p>`);
var root_1 = from_html(`<p class="note svelte-1acjvix"><!></p>`);
var root_2 = from_html(`<a class="svelte-1acjvix"> </a>`);
var root_3 = from_html(`<span class="t svelte-1acjvix"> </span>`);
var root_4 = from_html(`<div class="frame svelte-1acjvix"><!></div>`);
var root_5 = from_html(`<div class="page svelte-1acjvix"><h1 class="sr-only">Search every session</h1> <form class="bar svelte-1acjvix"><label class="field svelte-1acjvix"><!> <input type="search" aria-label="search every session" placeholder="A command, a question, an error" class="svelte-1acjvix"/></label> <button type="submit"> </button></form> <p class="note svelte-1acjvix">Tool calls, questions and errors from every session. Prompts and replies are never recorded, so they are never found.</p> <!> <!> <!></div>`);
function Search($$anchor, $$props) {
	push($$props, true);
	let q = prop($$props, "q", 3, ""), blind = prop($$props, "blind", 3, "");
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
	const keyOf = (h) => h.run_id + h.at + h.text;
	let order = state(proxy([]));
	user_effect(() => cursor({
		scope: "search",
		size: () => get(order).length,
		at: () => get(order).indexOf(get(selected) ?? ""),
		go: (i) => set(selected, get(order)[i], true),
		open: (i) => {
			const h = get(hits).find((x) => keyOf(x) === get(order)[i]);
			if (h) go(`#why/${encodeURIComponent(h.run_id)}`);
		}
	}));
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
	var div = root_5();
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
		var p_1 = root_1();
		var node_4 = child(p_1);
		Inline(node_4, { get text() {
			return blind();
		} });
		reset(p_1);
		append($$anchor, p_1);
	};
	if_block(node_3, ($$render) => {
		if (blind()) $$render(consequent_1);
	});
	var node_5 = sibling(node_3, 2);
	var consequent_2 = ($$anchor) => {
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
	var consequent_5 = ($$anchor) => {
		var div_1 = root_4();
		var node_6 = child(div_1);
		{
			const cell = ($$anchor, h = noop, c = noop) => {
				var fragment_1 = comment();
				var node_7 = first_child(fragment_1);
				var consequent_3 = ($$anchor) => {
					var text_1 = text();
					template_effect(($0) => set_text(text_1, $0), [() => new Date(h().at).toLocaleString()]);
					append($$anchor, text_1);
				};
				var consequent_4 = ($$anchor) => {
					var a = root_2();
					var text_2 = only_child(a, true);
					template_effect(($0, $1) => {
						set_attribute(a, "href", $0);
						set_text(text_2, $1);
					}, [() => `#why/${encodeURIComponent(h().run_id)}`, () => h().run_id.slice(0, 14)]);
					append($$anchor, a);
				};
				var alternate = ($$anchor) => {
					var span = root_3();
					var text_3 = only_child(span, true);
					template_effect(() => set_text(text_3, h().text));
					append($$anchor, span);
				};
				if_block(node_7, ($$render) => {
					if (c().key === "at") $$render(consequent_3);
					else if (c().key === "run") $$render(consequent_4, 1);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_1);
			};
			Grid(node_6, {
				id: "search",
				get columns() {
					return columns;
				},
				get rows() {
					return get(hits);
				},
				key: keyOf,
				label: "matches",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				get order() {
					return get(order);
				},
				set order($$value) {
					set(order, $$value, true);
				},
				cell,
				$$slots: { cell: true }
			});
		}
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node_5, ($$render) => {
		if (get(ran) && get(hits).length === 0 && !get(busy)) $$render(consequent_2);
		else if (get(hits).length > 0) $$render(consequent_5, 1);
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