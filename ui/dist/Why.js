import { A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, J as proxy, K as only_child, L as get, N as text, O as set_text, P as delegate, V as template_effect, W as child, X as state, Y as set, Z as user_derived, at as noop, et as pop, f as bind_value, it as reset, j as from_html, k as append, m as set_attribute, o as prop, ot as to_array, p as remove_input_defaults, q as sibling, rt as next, tt as push, v as set_class } from "./api.js";
import { P as surfaces, a as Inline, h as resource, i as Skeleton, n as Icon, t as Pill, w as go } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Failed } from "./Failed.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
//#region src/surfaces/why/Why.svelte
var root = from_html(`<button> <span class="svelte-1r7h8sn"> </span></button>`);
var root_1 = from_html(`<button class="more svelte-1r7h8sn"> </button>`);
var root_2 = from_html(`<p class="about svelte-1r7h8sn">Narrowed to <code> </code> · <a href="#why">show everything</a></p>`);
var root_3 = from_html(`<p class="more svelte-1r7h8sn"> <code> </code> has every one.</p>`);
var root_4 = from_html(`<span> </span>`);
var root_5 = from_html(`<span class="dim svelte-1r7h8sn"> </span>`);
var root_6 = from_html(`<p class="quiet svelte-1r7h8sn">Nothing matches.</p>`);
var root_7 = from_html(`<p><!></p>`);
var root_8 = from_html(`<a class="svelte-1r7h8sn"><!> the change</a>`);
var root_9 = from_html(`<a class="svelte-1r7h8sn"><!> everything about this run</a>`);
var root_10 = from_html(`<aside class="drawer svelte-1r7h8sn"><header class="svelte-1r7h8sn"><!> <b> </b> <button class="x svelte-1r7h8sn" aria-label="close"><!></button></header> <p class="when svelte-1r7h8sn"> </p> <pre class="svelte-1r7h8sn"> </pre> <!> <div class="links svelte-1r7h8sn"><!> <!></div></aside>`);
var root_11 = from_html(`<div class="frame svelte-1r7h8sn"><!> <!></div>`);
var root_12 = from_html(`<div class="ledger svelte-1r7h8sn"><header class="head svelte-1r7h8sn"><h1 class="svelte-1r7h8sn">Ledger</h1> <button title="rule, timer and nobody — what was decided instead of you"><!> Decided for you <span class="svelte-1r7h8sn"> </span></button> <div class="chips svelte-1r7h8sn" role="group" aria-label="by authority"><!> <!></div> <span class="gap svelte-1r7h8sn"></span> <label class="find svelte-1r7h8sn"><!><input placeholder="Filter" aria-label="filter the ledger" class="svelte-1r7h8sn"/></label></header> <!> <!> <!> <!></div>`);
function Why($$anchor, $$props) {
	push($$props, true);
	let about = prop($$props, "about", 3, "");
	const LIMIT = 1e3;
	const want = user_derived(about);
	const read = resource(() => get(want) ? `/api/decisions?about=${encodeURIComponent(get(want))}&limit=${LIMIT}` : `/api/decisions?limit=${LIMIT}`, { tell: () => get(want) ? `devplane audit ${get(want)}` : "devplane audit" });
	const rows = user_derived(() => Array.isArray(read.data) ? read.data : null);
	const AUTH = [
		"person",
		"rule",
		"timer",
		"nobody",
		"devplane"
	];
	const FOR_YOU = /* @__PURE__ */ new Set([
		"rule",
		"timer",
		"nobody"
	]);
	let only = state(null);
	let forYou = state(false);
	let text$1 = state("");
	let selected = state(null);
	const counts = user_derived(() => AUTH.map((a) => [a, (get(rows) ?? []).filter((r) => r.authority === a).length]));
	const shown = user_derived(() => (get(rows) ?? []).filter((r) => (!get(only) || r.authority === get(only)) && (!get(forYou) || FOR_YOU.has(r.authority)) && (!get(text$1).trim() || `${r.action} ${r.subject} ${r.reason ?? ""} ${r.outcome}`.toLowerCase().includes(get(text$1).trim().toLowerCase()))));
	const pick = user_derived(() => get(shown).find((r) => r.id === get(selected)) ?? null);
	let allChips = state(false);
	const zero = user_derived(() => get(counts).filter(([, n]) => n === 0));
	const chips = user_derived(() => get(allChips) || !get(rows) ? get(counts) : get(counts).filter(([a, n]) => n > 0 || get(only) === a));
	let order = state(proxy([]));
	const changePage = user_derived(() => surfaces().find((x) => x.link === "change")?.id ?? "");
	user_effect(() => cursor({
		scope: "why",
		size: () => get(order).length,
		at: () => get(order).indexOf(get(selected) ?? ""),
		go: (i) => set(selected, get(order)[i], true),
		open: (i) => {
			const r = get(shown).find((x) => x.id === get(order)[i]);
			if (r?.change_id && get(changePage)) go(`#${get(changePage)}/${encodeURIComponent(r.change_id)}`);
			else if (r?.run_id) go(`#why/${encodeURIComponent(r.run_id)}`);
		}
	}));
	const tone = (a) => a === "person" ? "work" : a === "rule" ? "wait" : a === "devplane" ? "none" : "fail";
	const day = (r) => new Date(r.at).toLocaleDateString(void 0, {
		weekday: "short",
		month: "short",
		day: "numeric"
	});
	const project = (p) => p ? p.split("/").filter(Boolean).pop() ?? p : "—";
	const columns = [
		{
			key: "at",
			label: "Time",
			width: 80,
			sort: (r) => r.at,
			mono: true
		},
		{
			key: "authority",
			label: "Authority",
			width: 120,
			sort: (r) => r.authority
		},
		{
			key: "outcome",
			label: "Outcome",
			width: 100,
			sort: (r) => r.outcome
		},
		{
			key: "action",
			label: "Action",
			width: 150,
			sort: (r) => r.action,
			mono: true
		},
		{
			key: "project",
			label: "Project",
			width: 110,
			sort: (r) => project(r.project_id)
		},
		{
			key: "subject",
			label: "About",
			width: 340,
			mono: true
		},
		{
			key: "reason",
			label: "Why"
		}
	];
	var div = root_12();
	var header = child(div);
	var button = sibling(child(header), 2);
	let classes;
	var node = child(button);
	Icon(node, {
		name: "person",
		size: 13
	});
	var span = sibling(node, 2);
	var text_1 = only_child(span, true);
	reset(button);
	var div_1 = sibling(button, 2);
	var node_1 = child(div_1);
	each(node_1, 17, () => get(chips), ([a, n]) => a, ($$anchor, $$item) => {
		var $$array = user_derived(() => to_array(get($$item), 2));
		let a = () => get($$array)[0];
		let n = () => get($$array)[1];
		var button_1 = root();
		let classes_1;
		var text_2 = child(button_1);
		var span_1 = sibling(text_2);
		var text_3 = only_child(span_1, true);
		reset(button_1);
		template_effect(() => {
			button_1.disabled = n() === 0;
			classes_1 = set_class(button_1, 1, "svelte-1r7h8sn", null, classes_1, { on: get(only) === a() });
			set_text(text_2, `${a() ?? ""} `);
			set_text(text_3, get(rows) ? n() : "");
		});
		delegated("click", button_1, () => set(only, get(only) === a() ? null : a(), true));
		append($$anchor, button_1);
	});
	var node_2 = sibling(node_1, 2);
	var consequent = ($$anchor) => {
		var button_2 = root_1();
		var text_4 = only_child(button_2);
		template_effect(($0) => {
			set_attribute(button_2, "title", `authorities with nothing recorded: ${$0 ?? ""}`);
			set_text(text_4, `+${get(zero).length ?? ""}`);
		}, [() => get(zero).map(([a]) => a).join(", ")]);
		delegated("click", button_2, () => set(allChips, true));
		append($$anchor, button_2);
	};
	if_block(node_2, ($$render) => {
		if (get(rows) && !get(allChips) && get(zero).length > 0) $$render(consequent);
	});
	reset(div_1);
	var label = sibling(div_1, 4);
	var node_3 = child(label);
	Icon(node_3, {
		name: "search",
		size: 13
	});
	var input = sibling(node_3);
	remove_input_defaults(input);
	reset(label);
	reset(header);
	var node_4 = sibling(header, 2);
	var consequent_1 = ($$anchor) => {
		var p_1 = root_2();
		var code = sibling(child(p_1));
		var text_5 = only_child(code, true);
		next(2);
		reset(p_1);
		template_effect(() => set_text(text_5, about()));
		append($$anchor, p_1);
	};
	if_block(node_4, ($$render) => {
		if (about()) $$render(consequent_1);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_2 = ($$anchor) => {
		Failed($$anchor, {
			what: "the ledger",
			get failure() {
				return read.failure;
			},
			get at() {
				return read.at;
			},
			stale: true
		});
	};
	if_block(node_5, ($$render) => {
		if (read.phase === "stale" && read.failure) $$render(consequent_2);
	});
	var node_6 = sibling(node_5, 2);
	var consequent_3 = ($$anchor) => {
		var p_2 = root_3();
		var text_6 = child(p_2);
		text_6.nodeValue = "The newest 1000 decisions are shown; ";
		var code_1 = sibling(text_6);
		var text_7 = only_child(code_1, true);
		next();
		reset(p_2);
		template_effect(() => set_text(text_7, get(want) ? `devplane audit ${get(want)}` : "devplane audit"));
		append($$anchor, p_2);
	};
	if_block(node_6, ($$render) => {
		if (get(rows) && get(rows).length >= LIMIT) $$render(consequent_3);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_4 = ($$anchor) => {
		Failed($$anchor, {
			what: "the ledger",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_5 = ($$anchor) => {
		Empty($$anchor, {
			icon: "ledger",
			title: "Nothing has been decided yet",
			body: "Every refusal a rule made, every question a person answered, every gate Devplane ran — each lands here with the authority that decided it."
		});
	};
	var alternate_2 = ($$anchor) => {
		var div_2 = root_11();
		var node_8 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_3 = comment();
				var node_9 = first_child(fragment_3);
				var consequent_6 = ($$anchor) => {
					var text_8 = text();
					template_effect(($0) => set_text(text_8, $0), [() => new Date(r().at).toTimeString().slice(0, 5)]);
					append($$anchor, text_8);
				};
				var consequent_7 = ($$anchor) => {
					{
						let $0 = user_derived(() => tone(r().authority));
						Pill($$anchor, {
							get word() {
								return r().authority;
							},
							get as() {
								return get($0);
							}
						});
					}
				};
				var consequent_8 = ($$anchor) => {
					var span_2 = root_4();
					var text_9 = only_child(span_2, true);
					template_effect(() => {
						set_class(span_2, 1, `out ${r().outcome ?? ""}`, "svelte-1r7h8sn");
						set_text(text_9, r().outcome);
					});
					append($$anchor, span_2);
				};
				var consequent_9 = ($$anchor) => {
					var text_10 = text();
					template_effect(() => set_text(text_10, r().action));
					append($$anchor, text_10);
				};
				var consequent_10 = ($$anchor) => {
					var text_11 = text();
					template_effect(($0) => set_text(text_11, $0), [() => project(r().project_id)]);
					append($$anchor, text_11);
				};
				var consequent_11 = ($$anchor) => {
					var span_3 = root_4();
					var text_12 = only_child(span_3, true);
					template_effect(() => {
						set_attribute(span_3, "title", r().subject);
						set_text(text_12, r().subject);
					});
					append($$anchor, span_3);
				};
				var alternate = ($$anchor) => {
					var span_4 = root_5();
					var text_13 = only_child(span_4, true);
					template_effect(() => set_text(text_13, r().reason ?? ""));
					append($$anchor, span_4);
				};
				if_block(node_9, ($$render) => {
					if (c().key === "at") $$render(consequent_6);
					else if (c().key === "authority") $$render(consequent_7, 1);
					else if (c().key === "outcome") $$render(consequent_8, 2);
					else if (c().key === "action") $$render(consequent_9, 3);
					else if (c().key === "project") $$render(consequent_10, 4);
					else if (c().key === "subject") $$render(consequent_11, 5);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_3);
			};
			const empty = ($$anchor) => {
				var fragment_8 = comment();
				var node_10 = first_child(fragment_8);
				var consequent_12 = ($$anchor) => {
					Skeleton($$anchor, {});
				};
				var alternate_1 = ($$anchor) => {
					var p_3 = root_6();
					append($$anchor, p_3);
				};
				if_block(node_10, ($$render) => {
					if (get(rows) === null) $$render(consequent_12);
					else $$render(alternate_1, -1);
				});
				append($$anchor, fragment_8);
			};
			Grid(node_8, {
				id: "ledger",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				group: day,
				label: "decisions",
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
				empty,
				$$slots: {
					cell: true,
					empty: true
				}
			});
		}
		var node_11 = sibling(node_8, 2);
		var consequent_16 = ($$anchor) => {
			var aside = root_10();
			var header_1 = child(aside);
			var node_12 = child(header_1);
			{
				let $0 = user_derived(() => tone(get(pick).authority));
				Pill(node_12, {
					get word() {
						return get(pick).authority;
					},
					get as() {
						return get($0);
					}
				});
			}
			var b = sibling(node_12, 2);
			var text_14 = only_child(b, true);
			var button_3 = sibling(b, 2);
			var node_13 = child(button_3);
			Icon(node_13, {
				name: "x",
				size: 13
			});
			reset(button_3);
			reset(header_1);
			var p_4 = sibling(header_1, 2);
			var text_15 = only_child(p_4);
			var pre = sibling(p_4, 2);
			var text_16 = only_child(pre, true);
			var node_14 = sibling(pre, 2);
			var consequent_13 = ($$anchor) => {
				var p_5 = root_7();
				var node_15 = child(p_5);
				Inline(node_15, { get text() {
					return get(pick).reason;
				} });
				reset(p_5);
				append($$anchor, p_5);
			};
			if_block(node_14, ($$render) => {
				if (get(pick).reason) $$render(consequent_13);
			});
			var div_3 = sibling(node_14, 2);
			var node_16 = child(div_3);
			var consequent_14 = ($$anchor) => {
				var a_1 = root_8();
				var node_17 = child(a_1);
				Icon(node_17, {
					name: "change",
					size: 13
				});
				next();
				reset(a_1);
				template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#change/${encodeURIComponent(get(pick).change_id)}`]);
				append($$anchor, a_1);
			};
			if_block(node_16, ($$render) => {
				if (get(pick).change_id) $$render(consequent_14);
			});
			var node_18 = sibling(node_16, 2);
			var consequent_15 = ($$anchor) => {
				var a_2 = root_9();
				var node_19 = child(a_2);
				Icon(node_19, {
					name: "sessions",
					size: 13
				});
				next();
				reset(a_2);
				template_effect(($0) => set_attribute(a_2, "href", $0), [() => `#why/${encodeURIComponent(get(pick).run_id)}`]);
				append($$anchor, a_2);
			};
			if_block(node_18, ($$render) => {
				if (get(pick).run_id) $$render(consequent_15);
			});
			reset(div_3);
			reset(aside);
			template_effect(($0) => {
				set_text(text_14, get(pick).action);
				set_text(text_15, `${$0 ?? ""} · ${get(pick).outcome ?? ""}`);
				set_text(text_16, get(pick).subject);
			}, [() => new Date(get(pick).at).toLocaleString()]);
			delegated("click", button_3, () => set(selected, null));
			append($$anchor, aside);
		};
		if_block(node_11, ($$render) => {
			if (get(pick)) $$render(consequent_16);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_7, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_4);
		else if (get(rows) && get(rows).length === 0) $$render(consequent_5, 1);
		else $$render(alternate_2, -1);
	});
	reset(div);
	template_effect(($0) => {
		classes = set_class(button, 1, "foryou svelte-1r7h8sn", null, classes, { on: get(forYou) });
		set_text(text_1, $0);
	}, [() => get(rows) ? get(rows).filter((r) => FOR_YOU.has(r.authority)).length : ""]);
	delegated("click", button, () => set(forYou, !get(forYou)));
	bind_value(input, () => get(text$1), ($$value) => set(text$1, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
export { Why as default };

//# sourceMappingURL=Why.js.map