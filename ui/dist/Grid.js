import { A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, I as event, J as proxy, K as only_child, L as get, O as set_text, P as delegate, S as snippet, V as template_effect, W as child, X as state, Y as set, Z as user_derived, _ as set_style, b as action, et as pop, it as reset, j as from_html, k as append, l as bind_this, m as set_attribute, o as prop, q as sibling, tt as push, v as set_class, z as untrack } from "./api.js";
import { n as Icon } from "./app.js";
//#region src/lib/ui/Grid.svelte
function nextSort(by, c) {
	if (!c.sort) return by;
	if (by?.key !== c.key) return {
		key: c.key,
		dir: 1
	};
	if (by.dir === 1) return {
		key: c.key,
		dir: -1
	};
	return null;
}
function sorted(rows, columns, by) {
	if (!by) return rows;
	const col = columns.find((c) => c.key === by.key);
	if (!col?.sort) return rows;
	const f = col.sort;
	return [...rows].sort((a, b) => {
		const x = f(a);
		const y = f(b);
		if (x == null && y == null) return 0;
		if (x == null) return 1;
		if (y == null) return -1;
		return (x < y ? -1 : x > y ? 1 : 0) * by.dir;
	});
}
var root = from_html(`<div role="columnheader"><span> </span> <!> <span class="grip svelte-jwljd6" role="presentation"></span></div>`);
var root_1 = from_html(`<span> </span>`);
var root_2 = from_html(`<button class="group svelte-jwljd6"><!> <!> <span class="n svelte-jwljd6"> </span></button>`);
var root_3 = from_html(`<div role="gridcell"><!></div>`);
var root_4 = from_html(`<div class="row svelte-jwljd6" role="row" tabindex="-1"></div>`);
var root_5 = from_html(`<!> <!>`, 1);
var root_6 = from_html(`<div role="grid" tabindex="0"><div class="head svelte-jwljd6" role="row"></div> <div class="body svelte-jwljd6"><!></div></div>`);
function Grid($$anchor, $$props) {
	push($$props, true);
	let group = prop($$props, "group", 3, void 0), groupLabel = prop($$props, "groupLabel", 3, void 0), selected = prop($$props, "selected", 15, null), order = prop($$props, "order", 31, () => proxy([])), open = prop($$props, "open", 3, void 0), empty = prop($$props, "empty", 3, void 0), dense = prop($$props, "dense", 3, false), label = prop($$props, "label", 3, "rows");
	const wkey = () => `vp-grid:${$$props.id}`;
	function load() {
		try {
			return JSON.parse(localStorage.getItem(wkey()) ?? "{}");
		} catch {
			return {};
		}
	}
	let widths = state(proxy(load()));
	const template = user_derived(() => $$props.columns.map((c, i) => {
		const w = get(widths)[c.key] ?? c.width;
		return w ? `${w}px` : i === $$props.columns.length - 1 ? "minmax(8rem, 1fr)" : "auto";
	}).join(" "));
	let resizing = null;
	function grab(e, c) {
		e.stopPropagation();
		const th = e.currentTarget.parentElement;
		resizing = {
			key: c.key,
			x: e.clientX,
			from: th.getBoundingClientRect().width
		};
		e.currentTarget.setPointerCapture(e.pointerId);
	}
	function drag(e) {
		if (!resizing) return;
		set(widths, {
			...get(widths),
			[resizing.key]: Math.max(48, resizing.from + e.clientX - resizing.x)
		}, true);
	}
	function drop() {
		if (!resizing) return;
		resizing = null;
		try {
			localStorage.setItem(wkey(), JSON.stringify(get(widths)));
		} catch {}
	}
	let by = state(null);
	function toggle(c) {
		set(by, nextSort(get(by), c), true);
	}
	const ordered = user_derived(() => sorted($$props.rows, $$props.columns, get(by)));
	let folded = state(proxy({}));
	const groups = user_derived(() => {
		if (!group()) return [{
			name: "",
			rows: get(ordered)
		}];
		const out = [];
		const at = /* @__PURE__ */ new Map();
		for (const r of get(ordered)) {
			const g = group()(r);
			if (!at.has(g)) {
				at.set(g, out.length);
				out.push({
					name: g,
					rows: []
				});
			}
			out[at.get(g)].rows.push(r);
		}
		return out;
	});
	const visible = user_derived(() => get(groups).flatMap((g) => get(folded)[g.name] ? [] : g.rows));
	let body = state(void 0);
	user_effect(() => {
		const next = get(visible).map($$props.key);
		if (next.join("\0") !== untrack(() => order()).join("\0")) order(next);
	});
	user_effect(() => {
		const at = selected();
		if (!at || !get(body)) return;
		get(body).querySelector(`[data-key="${CSS.escape(at)}"]`)?.scrollIntoView({ block: "nearest" });
	});
	function whole(el) {
		const on = () => {
			const cut = el.scrollWidth > el.clientWidth;
			const text = el.textContent?.trim() ?? "";
			if (cut && text) el.title = text;
			else el.removeAttribute("title");
		};
		el.addEventListener("pointerenter", on);
		return { destroy: () => el.removeEventListener("pointerenter", on) };
	}
	var div = root_6();
	let classes;
	var div_1 = child(div);
	each(div_1, 21, () => $$props.columns, (c) => c.key, ($$anchor, c) => {
		var div_2 = root();
		let classes_1;
		var span = child(div_2);
		var text_1 = only_child(span, true);
		var node = sibling(span, 2);
		var consequent = ($$anchor) => {
			{
				let $0 = user_derived(() => get(by).dir === 1 ? "down" : "right");
				Icon($$anchor, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
		};
		if_block(node, ($$render) => {
			if (get(by)?.key === get(c).key) $$render(consequent);
		});
		var span_1 = sibling(node, 2);
		reset(div_2);
		template_effect(($0) => {
			classes_1 = set_class(div_2, 1, "th svelte-jwljd6", null, classes_1, {
				end: get(c).align === "end",
				sortable: !!get(c).sort
			});
			set_attribute(div_2, "tabindex", get(c).sort ? 0 : -1);
			set_attribute(div_2, "aria-sort", get(by)?.key === get(c).key ? get(by).dir === 1 ? "ascending" : "descending" : "none");
			set_attribute(div_2, "title", $0);
			set_text(text_1, get(c).label);
		}, [() => get(c).sort ? `sort by ${get(c).label.toLowerCase()} (Enter or Space)` : void 0]);
		delegated("click", div_2, () => toggle(get(c)));
		delegated("keydown", div_2, (e) => {
			if (get(c).sort && (e.key === "Enter" || e.key === " ")) {
				e.preventDefault();
				e.stopPropagation();
				toggle(get(c));
			}
		});
		delegated("pointerdown", span_1, (e) => grab(e, get(c)));
		delegated("pointermove", span_1, drag);
		delegated("pointerup", span_1, drop);
		event("pointercancel", span_1, drop);
		append($$anchor, div_2);
	});
	reset(div_1);
	var div_3 = sibling(div_1, 2);
	var node_1 = child(div_3);
	var consequent_2 = ($$anchor) => {
		var fragment_1 = comment();
		var node_2 = first_child(fragment_1);
		var consequent_1 = ($$anchor) => {
			var fragment_2 = comment();
			var node_3 = first_child(fragment_2);
			snippet(node_3, empty);
			append($$anchor, fragment_2);
		};
		if_block(node_2, ($$render) => {
			if (empty()) $$render(consequent_1);
		});
		append($$anchor, fragment_1);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_3 = comment();
		var node_4 = first_child(fragment_3);
		each(node_4, 17, () => get(groups), (g) => g.name, ($$anchor, g) => {
			var fragment_4 = root_5();
			var node_5 = first_child(fragment_4);
			var consequent_4 = ($$anchor) => {
				var button = root_2();
				var node_6 = child(button);
				{
					let $0 = user_derived(() => get(folded)[get(g).name] ? "right" : "down");
					Icon(node_6, {
						get name() {
							return get($0);
						},
						size: 12
					});
				}
				var node_7 = sibling(node_6, 2);
				var consequent_3 = ($$anchor) => {
					var fragment_5 = comment();
					var node_8 = first_child(fragment_5);
					snippet(node_8, groupLabel, () => get(g).name, () => get(g).rows.length);
					append($$anchor, fragment_5);
				};
				var alternate = ($$anchor) => {
					var span_2 = root_1();
					var text_2 = only_child(span_2, true);
					template_effect(() => set_text(text_2, get(g).name));
					append($$anchor, span_2);
				};
				if_block(node_7, ($$render) => {
					if (groupLabel()) $$render(consequent_3);
					else $$render(alternate, -1);
				});
				var span_3 = sibling(node_7, 2);
				var text_3 = only_child(span_3, true);
				reset(button);
				template_effect(() => set_text(text_3, get(g).rows.length));
				delegated("click", button, () => set(folded, {
					...get(folded),
					[get(g).name]: !get(folded)[get(g).name]
				}, true));
				append($$anchor, button);
			};
			if_block(node_5, ($$render) => {
				if (group()) $$render(consequent_4);
			});
			var node_9 = sibling(node_5, 2);
			var consequent_5 = ($$anchor) => {
				var fragment_6 = comment();
				var node_10 = first_child(fragment_6);
				each(node_10, 17, () => get(g).rows, (r) => $$props.key(r), ($$anchor, r) => {
					var div_4 = root_4();
					each(div_4, 21, () => $$props.columns, (c) => c.key, ($$anchor, c) => {
						var div_5 = root_3();
						let classes_2;
						var node_11 = child(div_5);
						snippet(node_11, () => $$props.cell, () => get(r), () => get(c));
						reset(div_5);
						action(div_5, ($$node) => whole?.($$node));
						template_effect(() => classes_2 = set_class(div_5, 1, "td svelte-jwljd6", null, classes_2, {
							end: get(c).align === "end",
							mono: get(c).mono
						}));
						append($$anchor, div_5);
					});
					reset(div_4);
					template_effect(($0, $1) => {
						set_attribute(div_4, "data-key", $0);
						set_attribute(div_4, "aria-selected", $1);
					}, [() => $$props.key(get(r)), () => selected() === $$props.key(get(r))]);
					delegated("click", div_4, () => selected($$props.key(get(r))));
					delegated("dblclick", div_4, () => open()?.(get(r)));
					append($$anchor, div_4);
				});
				append($$anchor, fragment_6);
			};
			if_block(node_9, ($$render) => {
				if (!get(folded)[get(g).name]) $$render(consequent_5);
			});
			append($$anchor, fragment_4);
		});
		append($$anchor, fragment_3);
	};
	if_block(node_1, ($$render) => {
		if ($$props.rows.length === 0) $$render(consequent_2);
		else $$render(alternate_1, -1);
	});
	reset(div_3);
	bind_this(div_3, ($$value) => set(body, $$value), () => get(body));
	reset(div);
	template_effect(() => {
		classes = set_class(div, 1, "grid svelte-jwljd6", null, classes, { dense: dense() });
		set_attribute(div, "aria-label", label());
		set_attribute(div, "aria-rowcount", $$props.rows.length);
		set_style(div, `--cols: ${get(template) ?? ""}`);
	});
	append($$anchor, div);
	pop();
}
delegate([
	"click",
	"keydown",
	"pointerdown",
	"pointermove",
	"pointerup",
	"dblclick"
]);
//#endregion
export { Grid as t };

//# sourceMappingURL=Grid.js.map