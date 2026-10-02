import { A as append, D as if_block, F as delegate, G as child, H as template_effect, I as delegated, J as sibling, K as first_child, M as from_html, P as text, Q as user_derived, R as get, U as user_effect, X as set, Y as proxy, Z as state, at as reset, b as clsx, g as init_select, h as bind_select_value, it as next, j as comment, k as set_text, m as set_attribute, nt as push, o as prop, ot as noop, q as only_child, st as to_array, tt as pop, u as bind_element_size, w as each, y as set_class } from "./api.js";
import { O as go, R as surfaces, a as Inline, i as Skeleton, n as Icon, p as copyText, t as Pill } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
import { t as Props } from "./Props.js";
//#region src/surfaces/board/Board.svelte
var root = from_html(`<button> <span class="svelte-13ck13b"> </span></button>`);
var root_1 = from_html(`<div class="chips svelte-13ck13b" role="group" aria-label="by state"><button>all <span class="svelte-13ck13b"> </span></button> <!></div>`);
var root_2 = from_html(`<span class="hidden svelte-13ck13b"> </span>`);
var root_3 = from_html(`<span class="cost svelte-13ck13b" title="reported by the vendors' own telemetry"> </span>`);
var root_4 = from_html(`<p class="quiet svelte-13ck13b">The last read had no session in it, and Devplane has not answered since.</p>`);
var root_5 = from_html(`<span class="hot svelte-13ck13b" title="context nearly full"><!><span class="sr-only">context nearly full</span></span>`);
var root_6 = from_html(`<span class="st svelte-13ck13b"><!><!></span>`);
var root_7 = from_html(`<span class="sess svelte-13ck13b"><!> <b class="svelte-13ck13b"> </b> <span class="dim svelte-13ck13b"> </span></span>`);
var root_8 = from_html(`<span class="dim svelte-13ck13b"> </span>`);
var root_9 = from_html(`<!><span class="sr-only">context nearly full</span>`, 1);
var root_10 = from_html(`<span><!> </span>`);
var root_11 = from_html(`<span class="dim svelte-13ck13b" title="not reported">—</span>`);
var root_12 = from_html(`<span class="dim svelte-13ck13b">—</span>`);
var root_13 = from_html(`<p class="quiet svelte-13ck13b"> </p>`);
var root_14 = from_html(`<p class="said svelte-13ck13b"><!></p>`);
var root_15 = from_html(`<aside class="drawer svelte-13ck13b" aria-label="the session"><header class="svelte-13ck13b"><b> </b> <!> <button class="x svelte-13ck13b" aria-label="close"><!></button></header> <!> <div class="acts svelte-13ck13b"><button class="svelte-13ck13b"><!> Copy attach command</button></div> <!></aside>`);
var root_16 = from_html(`<div class="frame svelte-13ck13b"><!> <!></div>`);
var root_17 = from_html(`<div class="board svelte-13ck13b"><header class="head svelte-13ck13b"><h1 class="svelte-13ck13b">Sessions</h1> <!> <span class="gap svelte-13ck13b"></span> <label class="group svelte-13ck13b">Group <select aria-label="group by" class="svelte-13ck13b"><option>by project</option><option>by state</option><option>none</option></select></label> <!> <!></header> <!></div>`);
function Board($$anchor, $$props) {
	push($$props, true);
	let runs = prop($$props, "runs", 19, () => []), summary = prop($$props, "summary", 3, null), thresholds = prop($$props, "thresholds", 3, null), watching = prop($$props, "watching", 3, null), loaded = prop($$props, "loaded", 3, false), error = prop($$props, "error", 3, null), focus = prop($$props, "focus", 3, "");
	const BUCKETS = [
		["working", "working"],
		["waiting", "needs_you"],
		["idle", "idle"],
		["failed", "failed"],
		["dormant", "dormant"]
	];
	const IN_PLAY_SECONDS = 21600;
	const bucket = (r) => {
		const s = r.state ?? "";
		const w = r.waiting_for ?? "";
		const waiting = s === "waiting";
		if (!(waiting && w !== "idle" || s === "working" || s === "starting" || (r.idle_seconds ?? 0) < IN_PLAY_SECONDS && (!!r.reporting || s === "failed" || s === "lost"))) return "dormant";
		if (s === "working" || s === "starting") return "working";
		if (waiting) return "waiting";
		if (s === "failed" || s === "lost") return "failed";
		return "idle";
	};
	let only = state(null);
	let groupBy = state("project");
	let selected = state(null);
	user_effect(() => {
		if (focus()) set(selected, focus());
	});
	const counts = user_derived(() => loaded() && summary() ? BUCKETS.map(([b, field]) => [b, summary()[field] ?? 0]) : null);
	const shown = user_derived(() => get(only) ? runs().filter((r) => bucket(r) === get(only)) : runs());
	const pick = user_derived(() => runs().find((r) => r.id === get(selected)) ?? null);
	const high = user_derived(() => thresholds()?.context_high_percent ?? null);
	const crowded = (r) => get(high) != null && r.context_percent != null && r.context_percent >= get(high);
	const blind = user_derived(() => [watching()?.driven_only?.length ? `${watching().driven_only.join(", ")} ${watching().driven_only.length === 1 ? "appears only when Devplane starts it" : "appear only when Devplane starts them"}.` : "", watching()?.unproved?.length ? `${watching().unproved.join(", ")} ${watching().unproved.length === 1 ? "is" : "are"} read, but has not been proved against a live session.` : ""].filter(Boolean).join(" "));
	function when(at) {
		if (!at) return "—";
		const s = Math.max(0, Math.round((Date.now() - Date.parse(at)) / 1e3));
		return s < 60 ? `${s}s` : s < 3600 ? `${Math.round(s / 60)}m` : s < 86400 ? `${Math.round(s / 3600)}h` : `${Math.round(s / 86400)}d`;
	}
	const columns = [
		{
			key: "state",
			label: "State",
			width: 130,
			sort: (r) => bucket(r)
		},
		{
			key: "session",
			label: "Session",
			width: 220,
			sort: (r) => r.name ?? r.agent ?? r.id
		},
		{
			key: "project",
			label: "Project",
			width: 120,
			sort: (r) => r.project_name ?? ""
		},
		{
			key: "doing",
			label: "Doing",
			width: 320
		},
		{
			key: "context",
			label: "Context",
			width: 90,
			align: "end",
			sort: (r) => r.context_percent ?? -1,
			mono: true
		},
		{
			key: "cost",
			label: "Cost",
			width: 80,
			align: "end",
			sort: (r) => r.cost_usd ?? -1,
			mono: true
		},
		{
			key: "tools",
			label: "Tools",
			width: 70,
			align: "end",
			sort: (r) => r.tool_calls ?? 0,
			mono: true
		},
		{
			key: "last",
			label: "Last",
			align: "end",
			sort: (r) => r.last_event_at ?? "",
			mono: true
		}
	];
	const DROP = [
		"tools",
		"cost",
		"context",
		"last",
		"project",
		"doing"
	];
	let width = state(0);
	const fitted = user_derived(() => {
		if (!get(width)) return columns;
		const room = get(width) - (get(pick) ? 352 : 0) - 4;
		const need = (cs) => cs.reduce((n, c) => n + (c.width ?? 128), 0);
		let cols = columns;
		for (const k of DROP) {
			if (need(cols) <= room) break;
			cols = cols.filter((c) => c.key !== k);
		}
		return cols;
	});
	const hidden = user_derived(() => columns.filter((c) => !get(fitted).includes(c)));
	const shows = (k) => get(fitted).some((c) => c.key === k);
	let order = state(proxy([]));
	const runPage = user_derived(() => surfaces().find((x) => x.link === "run")?.id ?? "");
	user_effect(() => cursor({
		scope: "board",
		size: () => get(order).length,
		at: () => get(order).indexOf(get(selected) ?? ""),
		go: (i) => set(selected, get(order)[i], true),
		open: (i) => {
			if (get(runPage)) go(`#${get(runPage)}/${encodeURIComponent(get(order)[i])}`);
		}
	}));
	let said = state("");
	async function copy(t) {
		set(said, await copyText(t) ? `Copied: ${t}` : `Nothing was copied — this page has no clipboard: ${t}`, true);
	}
	var div = root_17();
	var header = child(div);
	var node = sibling(child(header), 2);
	var consequent = ($$anchor) => {
		var div_1 = root_1();
		var button = child(div_1);
		let classes;
		var span = sibling(child(button));
		var text = only_child(span, true);
		reset(button);
		var node_1 = sibling(button, 2);
		each(node_1, 17, () => get(counts), ([b, n]) => b, ($$anchor, $$item) => {
			var $$array = user_derived(() => to_array(get($$item), 2));
			let b = () => get($$array)[0];
			let n = () => get($$array)[1];
			var button_1 = root();
			let classes_1;
			var text_1 = child(button_1);
			var span_1 = sibling(text_1);
			var text_2 = only_child(span_1, true);
			reset(button_1);
			template_effect(() => {
				set_attribute(button_1, "aria-pressed", get(only) === b());
				classes_1 = set_class(button_1, 1, clsx(b()), "svelte-13ck13b", classes_1, { on: get(only) === b() });
				button_1.disabled = n() === 0;
				set_text(text_1, `${b() ?? ""} `);
				set_text(text_2, n());
			});
			delegated("click", button_1, () => set(only, get(only) === b() ? null : b(), true));
			append($$anchor, button_1);
		});
		reset(div_1);
		template_effect(() => {
			set_attribute(button, "aria-pressed", get(only) === null);
			classes = set_class(button, 1, "svelte-13ck13b", null, classes, { on: get(only) === null });
			set_text(text, summary()?.runs ?? runs().length);
		});
		delegated("click", button, () => set(only, null));
		append($$anchor, div_1);
	};
	if_block(node, ($$render) => {
		if (get(counts)) $$render(consequent);
	});
	var label = sibling(node, 4);
	var select = sibling(child(label));
	var option = child(select);
	option.value = option.__value = "project";
	var option_1 = sibling(option);
	option_1.value = option_1.__value = "state";
	var option_2 = sibling(option_1);
	option_2.value = option_2.__value = "none";
	reset(select);
	init_select(select);
	reset(label);
	var node_2 = sibling(label, 2);
	var consequent_1 = ($$anchor) => {
		var span_2 = root_2();
		var text_3 = only_child(span_2);
		template_effect(($0) => {
			set_attribute(span_2, "title", `hidden to fit: ${$0 ?? ""}`);
			set_text(text_3, `${get(hidden).length ?? ""} ${get(hidden).length === 1 ? "column" : "columns"} hidden`);
		}, [() => get(hidden).map((c) => c.label).join(", ")]);
		append($$anchor, span_2);
	};
	if_block(node_2, ($$render) => {
		if (get(hidden).length) $$render(consequent_1);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_2 = ($$anchor) => {
		var span_3 = root_3();
		var text_4 = only_child(span_3);
		template_effect(($0) => set_text(text_4, `$${$0 ?? ""} reported today`), [() => summary().cost_usd.toFixed(2)]);
		append($$anchor, span_3);
	};
	if_block(node_3, ($$render) => {
		if (summary()?.cost_usd) $$render(consequent_2);
	});
	reset(header);
	var node_4 = sibling(header, 2);
	var consequent_3 = ($$anchor) => {
		Skeleton($$anchor, {});
	};
	var consequent_4 = ($$anchor) => {
		var p = root_4();
		append($$anchor, p);
	};
	var consequent_5 = ($$anchor) => {
		Empty($$anchor, {
			icon: "sessions",
			title: "No session is reporting",
			body: "Sessions you start yourself appear here as soon as they report — Claude Code with no configuration, others once connected.",
			get limit() {
				return get(blind);
			}
		});
	};
	var alternate_3 = ($$anchor) => {
		var div_2 = root_16();
		var node_5 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_6 = first_child(fragment_2);
				var consequent_7 = ($$anchor) => {
					var span_4 = root_6();
					var node_7 = child(span_4);
					{
						let $0 = user_derived(() => r().state ?? "unknown");
						Pill(node_7, { get word() {
							return get($0);
						} });
					}
					var node_8 = sibling(node_7);
					var consequent_6 = ($$anchor) => {
						var span_5 = root_5();
						var node_9 = child(span_5);
						Icon(node_9, {
							name: "alert",
							size: 12
						});
						next();
						reset(span_5);
						append($$anchor, span_5);
					};
					var d = user_derived(() => !shows("context") && crowded(r()));
					if_block(node_8, ($$render) => {
						if (get(d)) $$render(consequent_6);
					});
					reset(span_4);
					append($$anchor, span_4);
				};
				var consequent_8 = ($$anchor) => {
					var span_6 = root_7();
					var node_10 = child(span_6);
					{
						let $0 = user_derived(() => r().mode === "driven" ? "agent" : "eye");
						Icon(node_10, {
							get name() {
								return get($0);
							},
							size: 13
						});
					}
					var b_1 = sibling(node_10, 2);
					var text_5 = only_child(b_1, true);
					var span_7 = sibling(b_1, 2);
					var text_6 = only_child(span_7, true);
					reset(span_6);
					template_effect(() => {
						set_text(text_5, r().name ?? r().agent ?? "session");
						set_text(text_6, r().model ?? "");
					});
					append($$anchor, span_6);
				};
				var consequent_9 = ($$anchor) => {
					var text_7 = text();
					template_effect(() => set_text(text_7, r().project_name ?? "—"));
					append($$anchor, text_7);
				};
				var consequent_10 = ($$anchor) => {
					var span_8 = root_8();
					var text_8 = only_child(span_8, true);
					template_effect(() => set_text(text_8, r().summary ?? ""));
					append($$anchor, span_8);
				};
				var consequent_13 = ($$anchor) => {
					var fragment_4 = comment();
					var node_11 = first_child(fragment_4);
					var consequent_12 = ($$anchor) => {
						var span_9 = root_10();
						let classes_2;
						var node_12 = child(span_9);
						var consequent_11 = ($$anchor) => {
							var fragment_5 = root_9();
							var node_13 = first_child(fragment_5);
							Icon(node_13, {
								name: "alert",
								size: 12
							});
							next();
							append($$anchor, fragment_5);
						};
						var d_1 = user_derived(() => crowded(r()));
						if_block(node_12, ($$render) => {
							if (get(d_1)) $$render(consequent_11);
						});
						var text_9 = sibling(node_12);
						reset(span_9);
						template_effect(($0, $1, $2) => {
							classes_2 = set_class(span_9, 1, "ctx svelte-13ck13b", null, classes_2, { hot: $0 });
							set_attribute(span_9, "title", $1);
							set_text(text_9, `${$2 ?? ""}%`);
						}, [
							() => crowded(r()),
							() => crowded(r()) ? "context nearly full" : void 0,
							() => Math.round(r().context_percent)
						]);
						append($$anchor, span_9);
					};
					var alternate = ($$anchor) => {
						var span_10 = root_11();
						append($$anchor, span_10);
					};
					if_block(node_11, ($$render) => {
						if (r().context_percent != null) $$render(consequent_12);
						else $$render(alternate, -1);
					});
					append($$anchor, fragment_4);
				};
				var consequent_15 = ($$anchor) => {
					var fragment_6 = comment();
					var node_14 = first_child(fragment_6);
					var consequent_14 = ($$anchor) => {
						var span_11 = root_12();
						append($$anchor, span_11);
					};
					var alternate_1 = ($$anchor) => {
						var text_10 = text();
						template_effect(($0) => set_text(text_10, `$${$0 ?? ""}`), [() => r().cost_usd.toFixed(2)]);
						append($$anchor, text_10);
					};
					if_block(node_14, ($$render) => {
						if (r().cost_unknown || r().cost_usd == null) $$render(consequent_14);
						else $$render(alternate_1, -1);
					});
					append($$anchor, fragment_6);
				};
				var consequent_16 = ($$anchor) => {
					var text_11 = text();
					template_effect(() => set_text(text_11, r().tool_calls ?? 0));
					append($$anchor, text_11);
				};
				var alternate_2 = ($$anchor) => {
					var text_12 = text();
					template_effect(($0) => set_text(text_12, $0), [() => when(r().last_event_at)]);
					append($$anchor, text_12);
				};
				if_block(node_6, ($$render) => {
					if (c().key === "state") $$render(consequent_7);
					else if (c().key === "session") $$render(consequent_8, 1);
					else if (c().key === "project") $$render(consequent_9, 2);
					else if (c().key === "doing") $$render(consequent_10, 3);
					else if (c().key === "context") $$render(consequent_13, 4);
					else if (c().key === "cost") $$render(consequent_15, 5);
					else if (c().key === "tools") $$render(consequent_16, 6);
					else $$render(alternate_2, -1);
				});
				append($$anchor, fragment_2);
			};
			const empty = ($$anchor) => {
				var p_1 = root_13();
				var text_13 = only_child(p_1, true);
				template_effect(() => set_text(text_13, get(only) ? `No session is ${get(only)}.` : "No session."));
				append($$anchor, p_1);
			};
			let $0 = user_derived(() => get(groupBy) === "none" ? void 0 : get(groupBy) === "project" ? (r) => r.project_name ?? "no project" : bucket);
			Grid(node_5, {
				id: "sessions",
				get columns() {
					return get(fitted);
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				get group() {
					return get($0);
				},
				label: "sessions",
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
		var node_15 = sibling(node_5, 2);
		var consequent_18 = ($$anchor) => {
			var aside = root_15();
			var header_1 = child(aside);
			var b_2 = child(header_1);
			var text_14 = only_child(b_2, true);
			var node_16 = sibling(b_2, 2);
			{
				let $0 = user_derived(() => get(pick).state ?? "unknown");
				Pill(node_16, { get word() {
					return get($0);
				} });
			}
			var button_2 = sibling(node_16, 2);
			var node_17 = child(button_2);
			Icon(node_17, {
				name: "x",
				size: 13
			});
			reset(button_2);
			reset(header_1);
			var node_18 = sibling(header_1, 2);
			{
				let $0 = user_derived(() => [
					{
						label: "Project",
						value: get(pick).project_name
					},
					{
						label: "Agent",
						value: get(pick).agent
					},
					{
						label: "Mode",
						value: get(pick).mode === "driven" ? "driven by Devplane" : "watched"
					},
					{
						label: "Permissions",
						value: get(pick).permission_mode,
						missing: "not reported"
					},
					{
						label: "Model",
						value: get(pick).model,
						missing: "not reported"
					},
					{
						label: "Branch",
						value: get(pick).branch,
						mono: true,
						missing: "none"
					},
					{
						label: "Directory",
						value: get(pick).cwd,
						mono: true
					},
					{
						label: "Plan",
						value: get(pick).plan_total ? `${get(pick).plan_total} steps · ${get(pick).plan_done ?? 0} done` : null,
						missing: "no plan reported"
					},
					{
						label: "Sent",
						value: get(pick).sent_says
					},
					{
						label: "Subagents",
						value: get(pick).subagents ? String(get(pick).subagents) : null,
						missing: "none"
					},
					{
						label: "Session",
						value: get(pick).id,
						mono: true
					}
				]);
				Props(node_18, { get rows() {
					return get($0);
				} });
			}
			var div_3 = sibling(node_18, 2);
			var button_3 = child(div_3);
			var node_19 = child(button_3);
			Icon(node_19, {
				name: "terminal",
				size: 13
			});
			next();
			reset(button_3);
			reset(div_3);
			var node_20 = sibling(div_3, 2);
			var consequent_17 = ($$anchor) => {
				var p_2 = root_14();
				var node_21 = child(p_2);
				Inline(node_21, { get text() {
					return get(said);
				} });
				reset(p_2);
				append($$anchor, p_2);
			};
			if_block(node_20, ($$render) => {
				if (get(said)) $$render(consequent_17);
			});
			reset(aside);
			template_effect(() => set_text(text_14, get(pick).name ?? get(pick).agent ?? "session"));
			delegated("click", button_2, () => set(selected, null));
			delegated("click", button_3, () => copy(`devplane run attach ${get(pick).id}`));
			append($$anchor, aside);
		};
		if_block(node_15, ($$render) => {
			if (get(pick)) $$render(consequent_18);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_4, ($$render) => {
		if (!loaded() && runs().length === 0) $$render(consequent_3);
		else if (runs().length === 0 && error()) $$render(consequent_4, 1);
		else if (runs().length === 0) $$render(consequent_5, 2);
		else $$render(alternate_3, -1);
	});
	reset(div);
	bind_select_value(select, () => get(groupBy), ($$value) => set(groupBy, $$value));
	bind_element_size(div, "clientWidth", ($$value) => set(width, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
export { Board as default };

//# sourceMappingURL=Board.js.map