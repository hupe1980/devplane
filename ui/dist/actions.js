import { A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, J as proxy, K as only_child, L as get, O as set_text, P as delegate, V as template_effect, W as child, X as state, Y as set, Z as user_derived, et as pop, f as bind_value, i as api, it as reset, j as from_html, k as append, l as bind_this, m as set_attribute, o as prop, p as remove_input_defaults, q as sibling, rt as next, tt as push, v as set_class, w as index } from "./api.js";
import { C as State, T as answerItemKeys, a as Inline, c as clip, j as spell, p as copyText } from "./app.js";
import { a as markedFor, n as unreadSays } from "./unread.svelte.js";
import { t as Qualifier } from "./Qualifier.js";
import { t as safeHref } from "./href.js";
//#region src/surfaces/inbox/Item.svelte
var root_1 = from_html(`<span class="unseen svelte-kz8fai">new</span>`);
var root_2 = from_html(`<span> </span>`);
var root_3 = from_html(`<p class="d svelte-kz8fai"><!></p>`);
var root_4 = from_html(`<span>· <!></span>`);
var root_5 = from_html(`<p class="facts svelte-kz8fai"><!> <span class="unread" title="hunks not marked in this browser's review, and weakened rows not recorded as read"> </span> <!></p>`);
var root_6 = from_html(`<kbd class="svelte-kz8fai"> </kbd>`);
var root_7 = from_html(`<span class="keyed svelte-kz8fai"><button> </button><!></span>`);
var root_8 = from_html(`<kbd title="puts you in this field" class="svelte-kz8fai">t</kbd>`);
var root_9 = from_html(`<div class="reply svelte-kz8fai"><input type="text" placeholder="your answer" class="svelte-kz8fai"/> <button data-send="">reply</button><kbd title="sends from inside the field" class="svelte-kz8fai"> </kbd> <!></div>`);
var root_10 = from_html(`<fieldset class="q svelte-kz8fai"><legend class="svelte-kz8fai"> </legend> <div class="opts svelte-kz8fai"></div> <!></fieldset>`);
var root_11 = from_html(`<div class="reply svelte-kz8fai"><button> </button></div>`);
var root_12 = from_html(`<!> <!>`, 1);
var root_13 = from_html(`<span class="keyed svelte-kz8fai"><button class="choice svelte-kz8fai"> </button><!></span>`);
var root_14 = from_html(`<span class="dead svelte-kz8fai"> </span>`);
var root_15 = from_html(`<p class="elsewhere svelte-kz8fai">1–9 choose the first nine; Tab reaches the rest, and Enter chooses one.</p>`);
var root_16 = from_html(`<div class="opts svelte-kz8fai"></div> <!>`, 1);
var root_17 = from_html(`<div class="reply svelte-kz8fai"><input type="text" placeholder="your answer" data-key="t" class="svelte-kz8fai"/> <button data-send="">reply</button><kbd title="sends from inside the field" class="svelte-kz8fai"> </kbd> <kbd title="puts you in this field" class="svelte-kz8fai">t</kbd></div>`);
var root_18 = from_html(`<p class="offer svelte-kz8fai"><span class="dim svelte-kz8fai">To stop being asked, paste this rule:</span> <code class="svelte-kz8fai"> </code> <button>copy</button> <span class="dim svelte-kz8fai"> </span></p>`);
var root_19 = from_html(`<p class="elsewhere svelte-kz8fai"><!></p>`);
var root_20 = from_html(`<span class="keyed svelte-kz8fai"><a class="act primary svelte-kz8fai" data-key="w">open the session in VS Code</a><kbd class="svelte-kz8fai">w</kbd></span>`);
var root_21 = from_html(`<span class="keyed svelte-kz8fai"><button class="primary svelte-kz8fai" data-key="w">copy the resume command</button><kbd class="svelte-kz8fai">w</kbd></span> <code class="cmd svelte-kz8fai"> </code>`, 1);
var root_22 = from_html(`<p class="session svelte-kz8fai"><!></p>`);
var root_23 = from_html(`<span class="keyed svelte-kz8fai"><button data-key="a">allow</button><kbd class="svelte-kz8fai">a</kbd></span>`);
var root_24 = from_html(`<span class="keyed svelte-kz8fai"><button data-key="d">deny</button><kbd class="svelte-kz8fai">d</kbd></span>`);
var root_25 = from_html(`<button>retry</button>`);
var root_26 = from_html(`<button>resume</button>`);
var root_27 = from_html(`<span class="keyed svelte-kz8fai"><a class="act primary svelte-kz8fai" data-key="r">review</a><kbd class="svelte-kz8fai">r</kbd></span>`);
var root_28 = from_html(`<button>tell the run</button>`);
var root_29 = from_html(`<button>accept the drift</button>`);
var root_30 = from_html(`<span class="keyed svelte-kz8fai"><a class="act svelte-kz8fai" data-key="o">open</a><kbd class="svelte-kz8fai">o</kbd></span>`);
var root_31 = from_html(`<a class="act svelte-kz8fai" target="_blank" rel="noreferrer noopener"> </a>`);
var root_32 = from_html(`<a class="act svelte-kz8fai">open an agent with this typed</a>`);
var root_33 = from_html(`<span class="keyed svelte-kz8fai"><button data-key="s">snooze 1h</button><kbd class="svelte-kz8fai">s</kbd></span>`);
var root_34 = from_html(`<code class="cmd svelte-kz8fai"> </code>`);
var root_35 = from_html(`<li><span class="lvl svelte-kz8fai"><!></span> <div class="body svelte-kz8fai"><div class="t svelte-kz8fai"><b class="svelte-kz8fai"> </b> <span class="kind svelte-kz8fai"> </span> <!> <span class="meta svelte-kz8fai"><!> <!></span></div> <!> <!> <!> <!> <!> <!> <div class="acts svelte-kz8fai"><!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!> <!></div></div></li>`);
function Item($$anchor, $$props) {
	push($$props, true);
	const DETAIL_CHARS = 400;
	let current = prop($$props, "current", 3, false), age = prop($$props, "age", 3, ""), keys = prop($$props, "keys", 3, "");
	const acts = user_derived(() => $$props.item.actions ?? []);
	const markedHunks = user_derived(() => $$props.item.facts && $$props.item.change_id ? Math.min(markedFor($$props.item.change_id), $$props.item.facts.hunks) : 0);
	const has = (a) => get(acts).includes(a);
	const url = user_derived(() => safeHref($$props.item.url));
	const launch = user_derived(() => safeHref($$props.item.launch));
	const loud = user_derived(() => $$props.item.level === "critical" || $$props.item.level === "high");
	let typed = state("");
	let custom = proxy({});
	function reply(field) {
		const words = (field ? custom[field] ?? "" : get(typed)).trim();
		if (!words) {
			$$props.say("type your answer first — an empty reply is not an answer");
			return;
		}
		if (field) pick(field, { custom: words });
		else $$props.answer($$props.item, { custom: words });
	}
	const form = user_derived(() => ($$props.item.form ?? []).length > 0 ? $$props.item.form ?? [] : null);
	let picked = state(proxy({}));
	function pick(field, what) {
		const fields = get(form) ?? [];
		if (fields.length <= 1) {
			$$props.answer($$props.item, what.custom !== void 0 ? {
				custom: what.custom,
				field
			} : {
				option: what.option,
				field
			});
			return;
		}
		set(picked, {
			...get(picked),
			[field]: what
		}, true);
		if (fields.every((q) => get(picked)[q.field])) send();
	}
	const outstanding = user_derived(() => (get(form) ?? []).filter((q) => !get(picked)[q.field]).length);
	const keyedField = user_derived(() => (get(form) ?? []).find((q) => !get(picked)[q.field])?.field ?? null);
	const optionKey = (index) => {
		const n = ($$props.item.options ?? []).slice(0, index + 1).filter((o) => o.id).length;
		return n <= 9 ? String(n) : null;
	};
	const answerable = user_derived(() => ($$props.item.options ?? []).filter((o) => o.id).length);
	const sessionLink = user_derived(() => safeHref($$props.item.session_link));
	async function copyResume() {
		const cmd = $$props.item.resume_command;
		if (!cmd) return;
		$$props.say(await copyText(cmd) ? `copied: ${cmd} — run it in a terminal to answer there` : `nothing was copied: this page has no clipboard — run ${cmd}`);
	}
	let root = state(void 0);
	user_effect(() => keys() ? answerItemKeys(keys(), () => get(root), () => $$props.item, $$props.say) : void 0);
	function send() {
		$$props.answer($$props.item, { answers: (get(form) ?? []).map((q) => ({
			field: q.field,
			...get(picked)[q.field]
		})) });
	}
	var li = root_35();
	let classes;
	var span = child(li);
	var node = child(span);
	var consequent = ($$anchor) => {
		State($$anchor, {
			state: "needs you",
			get word() {
				return $$props.item.level;
			}
		});
	};
	var alternate = ($$anchor) => {
		State($$anchor, {
			state: "idle",
			get word() {
				return $$props.item.level;
			},
			quiet: true
		});
	};
	if_block(node, ($$render) => {
		if (get(loud)) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(span);
	var div = sibling(span, 2);
	var div_1 = child(div);
	var b = child(div_1);
	var text = only_child(b, true);
	var span_1 = sibling(b, 2);
	var text_1 = only_child(span_1, true);
	var node_1 = sibling(span_1, 2);
	var consequent_1 = ($$anchor) => {
		var span_2 = root_1();
		append($$anchor, span_2);
	};
	if_block(node_1, ($$render) => {
		if ($$props.item.new_to_you) $$render(consequent_1);
	});
	var span_3 = sibling(node_1, 2);
	var node_2 = child(span_3);
	var consequent_2 = ($$anchor) => {
		var span_4 = root_2();
		var text_2 = only_child(span_4, true);
		template_effect(() => set_text(text_2, $$props.item.project_name));
		append($$anchor, span_4);
	};
	if_block(node_2, ($$render) => {
		if ($$props.item.project_name) $$render(consequent_2);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_3 = ($$anchor) => {
		var span_5 = root_2();
		var text_3 = only_child(span_5, true);
		template_effect(() => {
			set_attribute(span_5, "title", $$props.item.since);
			set_text(text_3, age());
		});
		append($$anchor, span_5);
	};
	if_block(node_3, ($$render) => {
		if (age()) $$render(consequent_3);
	});
	reset(span_3);
	reset(div_1);
	var node_4 = sibling(div_1, 2);
	var consequent_4 = ($$anchor) => {
		var p = root_3();
		var node_5 = child(p);
		{
			let $0 = user_derived(() => clip($$props.item.detail, DETAIL_CHARS));
			Inline(node_5, { get text() {
				return get($0);
			} });
		}
		reset(p);
		template_effect(() => set_attribute(p, "title", $$props.item.detail.length > DETAIL_CHARS ? $$props.item.detail : void 0));
		append($$anchor, p);
	};
	if_block(node_4, ($$render) => {
		if ($$props.item.detail) $$render(consequent_4);
	});
	var node_6 = sibling(node_4, 2);
	var consequent_5 = ($$anchor) => {
		var p_1 = root_5();
		var node_7 = child(p_1);
		Qualifier(node_7, { get q() {
			return $$props.item.facts.qualifier;
		} });
		var span_6 = sibling(node_7, 2);
		var text_4 = only_child(span_6);
		var node_8 = sibling(span_6, 2);
		each(node_8, 16, () => $$props.item.facts.says, (s) => s, ($$anchor, s) => {
			var span_7 = root_4();
			var node_9 = sibling(child(span_7));
			Inline(node_9, { get text() {
				return s;
			} });
			reset(span_7);
			append($$anchor, span_7);
		});
		reset(p_1);
		template_effect(($0) => set_text(text_4, `· ${$0 ?? ""}`), [() => unreadSays({
			hunks: $$props.item.facts.hunks - get(markedHunks),
			weakened: $$props.item.facts.qualifier.unseen
		})]);
		append($$anchor, p_1);
	};
	if_block(node_6, ($$render) => {
		if ($$props.item.facts) $$render(consequent_5);
	});
	var node_10 = sibling(node_6, 2);
	var consequent_10 = ($$anchor) => {
		var fragment_2 = root_12();
		var node_11 = first_child(fragment_2);
		each(node_11, 17, () => get(form), (q) => q.field, ($$anchor, q) => {
			var fieldset = root_10();
			var legend = child(fieldset);
			var text_5 = only_child(legend, true);
			var div_2 = sibling(legend, 2);
			each(div_2, 21, () => get(q).options, index, ($$anchor, o, oi) => {
				const k = user_derived(() => get(q).field === get(keyedField) && oi < 9 ? String(oi + 1) : null);
				var span_8 = root_7();
				var button = child(span_8);
				let classes_1;
				var text_6 = only_child(button, true);
				var node_12 = sibling(button);
				var consequent_6 = ($$anchor) => {
					var kbd = root_6();
					var text_7 = only_child(kbd, true);
					template_effect(() => set_text(text_7, get(k)));
					append($$anchor, kbd);
				};
				if_block(node_12, ($$render) => {
					if (get(k)) $$render(consequent_6);
				});
				reset(span_8);
				template_effect(() => {
					classes_1 = set_class(button, 1, "choice svelte-kz8fai", null, classes_1, { picked: get(picked)[get(q).field]?.option === get(o).value });
					set_attribute(button, "title", get(o).detail ?? void 0);
					set_attribute(button, "data-key", get(k));
					set_text(text_6, get(o).label);
				});
				delegated("click", button, () => pick(get(q).field, { option: get(o).value }));
				append($$anchor, span_8);
			});
			reset(div_2);
			var node_13 = sibling(div_2, 2);
			var consequent_8 = ($$anchor) => {
				var div_3 = root_9();
				var input = child(div_3);
				remove_input_defaults(input);
				var button_1 = sibling(input, 2);
				var kbd_1 = sibling(button_1);
				var text_8 = only_child(kbd_1, true);
				var node_14 = sibling(kbd_1, 2);
				var consequent_7 = ($$anchor) => {
					var kbd_2 = root_8();
					append($$anchor, kbd_2);
				};
				if_block(node_14, ($$render) => {
					if (get(q).field === get(keyedField)) $$render(consequent_7);
				});
				reset(div_3);
				template_effect(($0) => {
					set_attribute(input, "aria-label", `your answer to: ${get(q).title ?? ""}`);
					set_attribute(input, "data-key", get(q).field === get(keyedField) ? "t" : null);
					set_text(text_8, $0);
				}, [() => spell("Mod+Enter")]);
				bind_value(input, () => custom[get(q).field], ($$value) => custom[get(q).field] = $$value);
				delegated("click", button_1, () => reply(get(q).field));
				append($$anchor, div_3);
			};
			if_block(node_13, ($$render) => {
				if (get(q).custom_field) $$render(consequent_8);
			});
			reset(fieldset);
			template_effect(() => set_text(text_5, get(q).title));
			append($$anchor, fieldset);
		});
		var node_15 = sibling(node_11, 2);
		var consequent_9 = ($$anchor) => {
			var div_4 = root_11();
			var button_2 = child(div_4);
			var text_9 = only_child(button_2, true);
			reset(div_4);
			template_effect(() => {
				button_2.disabled = get(outstanding) > 0;
				set_text(text_9, get(outstanding) > 0 ? `${get(outstanding)} still to answer` : `send ${get(form).length} answers`);
			});
			delegated("click", button_2, send);
			append($$anchor, div_4);
		};
		if_block(node_15, ($$render) => {
			if (get(form).length > 1) $$render(consequent_9);
		});
		append($$anchor, fragment_2);
	};
	var d = user_derived(() => get(form) && (has("choose") || has("reply")));
	var alternate_2 = ($$anchor) => {
		var fragment_3 = root_12();
		var node_16 = first_child(fragment_3);
		var consequent_14 = ($$anchor) => {
			var fragment_4 = root_16();
			var div_5 = first_child(fragment_4);
			each(div_5, 21, () => $$props.item.options ?? [], index, ($$anchor, o, oi) => {
				var fragment_5 = comment();
				var node_17 = first_child(fragment_5);
				var consequent_12 = ($$anchor) => {
					const k = user_derived(() => optionKey(oi));
					var span_9 = root_13();
					var button_3 = child(span_9);
					var text_10 = only_child(button_3, true);
					var node_18 = sibling(button_3);
					var consequent_11 = ($$anchor) => {
						var kbd_3 = root_6();
						var text_11 = only_child(kbd_3, true);
						template_effect(() => set_text(text_11, get(k)));
						append($$anchor, kbd_3);
					};
					if_block(node_18, ($$render) => {
						if (get(k)) $$render(consequent_11);
					});
					reset(span_9);
					template_effect(() => {
						set_attribute(button_3, "data-key", get(k));
						set_text(text_10, get(o).label);
					});
					delegated("click", button_3, () => $$props.answer($$props.item, { option: get(o).id }));
					append($$anchor, span_9);
				};
				var alternate_1 = ($$anchor) => {
					var span_10 = root_14();
					var text_12 = only_child(span_10, true);
					template_effect(() => set_text(text_12, get(o).label));
					append($$anchor, span_10);
				};
				if_block(node_17, ($$render) => {
					if (get(o).id) $$render(consequent_12);
					else $$render(alternate_1, -1);
				});
				append($$anchor, fragment_5);
			});
			reset(div_5);
			var node_19 = sibling(div_5, 2);
			var consequent_13 = ($$anchor) => {
				var p_2 = root_15();
				append($$anchor, p_2);
			};
			if_block(node_19, ($$render) => {
				if (get(answerable) > 9) $$render(consequent_13);
			});
			append($$anchor, fragment_4);
		};
		var d_1 = user_derived(() => ($$props.item.options ?? []).length > 0 && has("choose"));
		if_block(node_16, ($$render) => {
			if (get(d_1)) $$render(consequent_14);
		});
		var node_20 = sibling(node_16, 2);
		var consequent_15 = ($$anchor) => {
			var div_6 = root_17();
			var input_1 = child(div_6);
			remove_input_defaults(input_1);
			var button_4 = sibling(input_1, 2);
			var kbd_4 = sibling(button_4);
			var text_13 = only_child(kbd_4, true);
			next(2);
			reset(div_6);
			template_effect(($0) => {
				set_attribute(input_1, "aria-label", `your answer to: ${$$props.item.title ?? ""}`);
				set_text(text_13, $0);
			}, [() => spell("Mod+Enter")]);
			bind_value(input_1, () => get(typed), ($$value) => set(typed, $$value));
			delegated("click", button_4, () => reply());
			append($$anchor, div_6);
		};
		var d_2 = user_derived(() => has("reply"));
		if_block(node_20, ($$render) => {
			if (get(d_2)) $$render(consequent_15);
		});
		append($$anchor, fragment_3);
	};
	if_block(node_10, ($$render) => {
		if (get(d)) $$render(consequent_10);
		else $$render(alternate_2, -1);
	});
	var node_21 = sibling(node_10, 2);
	var consequent_16 = ($$anchor) => {
		var p_3 = root_18();
		var code = sibling(child(p_3), 2);
		var text_14 = only_child(code, true);
		var button_5 = sibling(code, 2);
		var span_11 = sibling(button_5, 2);
		var text_15 = only_child(span_11);
		reset(p_3);
		template_effect(() => {
			set_text(text_14, $$props.item.offer.rule);
			set_text(text_15, `paste into ${$$props.item.offer.file ?? ""} ${$$props.item.offer.section ?? ""} · covers ${$$props.item.offer.covers ?? ""}${$$props.item.offer.more ? "+" : ""} like it`);
		});
		delegated("click", button_5, () => $$props.copyRule($$props.item.offer.rule));
		append($$anchor, p_3);
	};
	var consequent_17 = ($$anchor) => {
		var p_4 = root_19();
		var node_22 = child(p_4);
		Inline(node_22, { get text() {
			return $$props.item.no_offer.sentence;
		} });
		reset(p_4);
		append($$anchor, p_4);
	};
	if_block(node_21, ($$render) => {
		if ($$props.item.offer) $$render(consequent_16);
		else if ($$props.item.no_offer) $$render(consequent_17, 1);
	});
	var node_23 = sibling(node_21, 2);
	var consequent_18 = ($$anchor) => {
		var p_5 = root_19();
		var node_24 = child(p_5);
		Inline(node_24, { get text() {
			return $$props.item.answer_in;
		} });
		reset(p_5);
		append($$anchor, p_5);
	};
	if_block(node_23, ($$render) => {
		if ($$props.item.answer_in) $$render(consequent_18);
	});
	var node_25 = sibling(node_23, 2);
	var consequent_20 = ($$anchor) => {
		var p_6 = root_22();
		var node_26 = child(p_6);
		var consequent_19 = ($$anchor) => {
			var span_12 = root_20();
			var a_1 = child(span_12);
			next();
			reset(span_12);
			template_effect(() => set_attribute(a_1, "href", get(sessionLink)));
			append($$anchor, span_12);
		};
		var alternate_3 = ($$anchor) => {
			var fragment_6 = root_21();
			var span_13 = first_child(fragment_6);
			var button_6 = child(span_13);
			next();
			reset(span_13);
			var code_1 = sibling(span_13, 2);
			var text_16 = only_child(code_1, true);
			template_effect(() => set_text(text_16, $$props.item.resume_command));
			delegated("click", button_6, copyResume);
			append($$anchor, fragment_6);
		};
		if_block(node_26, ($$render) => {
			if (get(sessionLink)) $$render(consequent_19);
			else $$render(alternate_3, -1);
		});
		reset(p_6);
		append($$anchor, p_6);
	};
	if_block(node_25, ($$render) => {
		if ($$props.item.answer_in && (get(sessionLink) || $$props.item.resume_command)) $$render(consequent_20);
	});
	var div_7 = sibling(node_25, 2);
	var node_27 = child(div_7);
	var consequent_21 = ($$anchor) => {
		var span_14 = root_23();
		var button_7 = child(span_14);
		next();
		reset(span_14);
		delegated("click", button_7, () => $$props.answer($$props.item, { decision: "allow" }));
		append($$anchor, span_14);
	};
	var d_3 = user_derived(() => has("allow"));
	if_block(node_27, ($$render) => {
		if (get(d_3)) $$render(consequent_21);
	});
	var node_28 = sibling(node_27, 2);
	var consequent_22 = ($$anchor) => {
		var span_15 = root_24();
		var button_8 = child(span_15);
		next();
		reset(span_15);
		delegated("click", button_8, () => $$props.answer($$props.item, { decision: "deny" }));
		append($$anchor, span_15);
	};
	var d_4 = user_derived(() => has("deny"));
	if_block(node_28, ($$render) => {
		if (get(d_4)) $$render(consequent_22);
	});
	var node_29 = sibling(node_28, 2);
	var consequent_23 = ($$anchor) => {
		var button_9 = root_25();
		delegated("click", button_9, () => $$props.act($$props.item, "retry"));
		append($$anchor, button_9);
	};
	var d_5 = user_derived(() => has("retry"));
	if_block(node_29, ($$render) => {
		if (get(d_5)) $$render(consequent_23);
	});
	var node_30 = sibling(node_29, 2);
	var consequent_24 = ($$anchor) => {
		var button_10 = root_26();
		delegated("click", button_10, () => $$props.act($$props.item, "resume"));
		append($$anchor, button_10);
	};
	var d_6 = user_derived(() => has("resume"));
	if_block(node_30, ($$render) => {
		if (get(d_6)) $$render(consequent_24);
	});
	var node_31 = sibling(node_30, 2);
	var consequent_25 = ($$anchor) => {
		var span_16 = root_27();
		var a_2 = child(span_16);
		next();
		reset(span_16);
		template_effect(($0) => set_attribute(a_2, "href", $0), [() => `#change/${encodeURIComponent($$props.item.change_id)}/review`]);
		append($$anchor, span_16);
	};
	var d_7 = user_derived(() => has("review") && $$props.item.change_id);
	if_block(node_31, ($$render) => {
		if (get(d_7)) $$render(consequent_25);
	});
	var node_32 = sibling(node_31, 2);
	var consequent_26 = ($$anchor) => {
		var button_11 = root_28();
		delegated("click", button_11, () => $$props.act($$props.item, "tell_run"));
		append($$anchor, button_11);
	};
	var d_8 = user_derived(() => has("tell_run"));
	if_block(node_32, ($$render) => {
		if (get(d_8)) $$render(consequent_26);
	});
	var node_33 = sibling(node_32, 2);
	var consequent_27 = ($$anchor) => {
		var button_12 = root_29();
		delegated("click", button_12, () => $$props.act($$props.item, "accept_drift"));
		append($$anchor, button_12);
	};
	var d_9 = user_derived(() => has("accept_drift"));
	if_block(node_33, ($$render) => {
		if (get(d_9)) $$render(consequent_27);
	});
	var node_34 = sibling(node_33, 2);
	var consequent_28 = ($$anchor) => {
		var span_17 = root_30();
		var a_3 = child(span_17);
		next();
		reset(span_17);
		template_effect(($0) => set_attribute(a_3, "href", $0), [() => $$props.item.change_id ? `#change/${encodeURIComponent($$props.item.change_id)}` : `#why/${encodeURIComponent($$props.item.run_id ?? "")}`]);
		append($$anchor, span_17);
	};
	var d_10 = user_derived(() => has("open") && ($$props.item.run_id || $$props.item.change_id));
	if_block(node_34, ($$render) => {
		if (get(d_10)) $$render(consequent_28);
	});
	var node_35 = sibling(node_34, 2);
	var consequent_29 = ($$anchor) => {
		var a_4 = root_31();
		var text_17 = only_child(a_4, true);
		template_effect(($0) => {
			set_attribute(a_4, "href", get(url));
			set_text(text_17, $0);
		}, [() => has("open_pr") ? "open pull request" : "open issue"]);
		append($$anchor, a_4);
	};
	var d_11 = user_derived(() => (has("open_pr") || has("open_issue")) && get(url));
	if_block(node_35, ($$render) => {
		if (get(d_11)) $$render(consequent_29);
	});
	var node_36 = sibling(node_35, 2);
	var consequent_30 = ($$anchor) => {
		var a_5 = root_32();
		template_effect(() => set_attribute(a_5, "href", get(launch)));
		append($$anchor, a_5);
	};
	if_block(node_36, ($$render) => {
		if (get(launch)) $$render(consequent_30);
	});
	var node_37 = sibling(node_36, 2);
	var consequent_31 = ($$anchor) => {
		var span_18 = root_33();
		var button_13 = child(span_18);
		next();
		reset(span_18);
		delegated("click", button_13, () => $$props.snooze($$props.item));
		append($$anchor, span_18);
	};
	var d_12 = user_derived(() => has("snooze"));
	if_block(node_37, ($$render) => {
		if (get(d_12)) $$render(consequent_31);
	});
	var node_38 = sibling(node_37, 2);
	var consequent_32 = ($$anchor) => {
		var code_2 = root_34();
		var text_18 = only_child(code_2);
		template_effect(() => set_text(text_18, `devplane run attach ${$$props.item.run_id ?? ""}`));
		append($$anchor, code_2);
	};
	var d_13 = user_derived(() => has("attach") && $$props.item.run_id);
	if_block(node_38, ($$render) => {
		if (get(d_13)) $$render(consequent_32);
	});
	reset(div_7);
	reset(div);
	reset(li);
	bind_this(li, ($$value) => set(root, $$value), () => get(root));
	template_effect(($0) => {
		classes = set_class(li, 1, "item svelte-kz8fai", null, classes, {
			loud: get(loud),
			current: current()
		});
		set_attribute(li, "id", $$props.item.ask ? `row-${$$props.item.ask}` : void 0);
		set_attribute(li, "aria-current", current() ? "true" : void 0);
		set_text(text, $$props.item.title);
		set_text(text_1, $0);
	}, [() => $$props.item.kind.replace(/_/g, " ")]);
	append($$anchor, li);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/inbox/actions.ts
function outcome(r, ours) {
	const commands = [r?.push, r?.create].filter((c) => !!c);
	if (commands.length) return {
		said: r?.says ?? "Nothing was pushed — run these yourself:",
		undo: null,
		commands
	};
	return {
		said: r?.says ?? ours,
		undo: null
	};
}
var ROUTES = {
	retry: {
		of: "change",
		route: "/api/changes/{id}/retry",
		says: "retrying"
	},
	resume: {
		of: "change",
		route: "/api/changes/{id}/resume",
		says: "resumed"
	},
	tell_run: {
		of: "change",
		route: "/api/changes/{id}/drift/tell",
		says: "told — the run was handed the files that changed, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	},
	accept_drift: {
		of: "change",
		route: "/api/changes/{id}/drift/accept",
		says: "accepted — the change now works to what the run saw, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	}
};
var failed = (e) => e instanceof Error ? e.message : String(e);
async function act(item, action) {
	const r = ROUTES[action];
	const id = r?.of === "change" ? item.change_id : item.run_id;
	if (!r || !id) return {
		said: `${action} cannot be done from here`,
		undo: null
	};
	try {
		return outcome(await api(r.route.replace("{id}", encodeURIComponent(id)), {
			method: "POST",
			...r.body ? { body: JSON.stringify(r.body(item)) } : {}
		}), r.says);
	} catch (e) {
		return {
			said: `${action} did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function answer(item, what) {
	const ask = item.ask ?? item.request_id;
	if (!ask) return {
		said: "this one cannot be answered from here",
		undo: null
	};
	try {
		const r = await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				...what,
				hold: true
			})
		});
		return {
			said: r?.held ? `${r.held.says} — held; z takes it back before it is sent` : "answered — on its way to the agent. That cannot be taken back.",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function snooze(item) {
	const where = item.change_id ? `/api/changes/${encodeURIComponent(item.change_id)}/snooze` : item.run_id ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze` : item.project_id ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze` : null;
	if (!where) return {
		said: "there is nothing to snooze this against",
		undo: null
	};
	try {
		await api(`${where}?minutes=60`, { method: "POST" });
		return {
			said: "hidden for an hour",
			undo: {
				says: "put it back",
				where: `${where}?minutes=0`
			}
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function takeBack(undo) {
	try {
		return {
			said: (await api(undo.where, { method: "POST" }))?.says ?? "back in the list",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function copyRule(rule) {
	return await copyText(rule) ? {
		said: `copied: ${rule}`,
		undo: null
	} : {
		said: "nothing was copied: this page has no clipboard — select the rule above",
		undo: null
	};
}
//#endregion
export { takeBack as a, snooze as i, answer as n, Item as o, copyRule as r, act as t };

//# sourceMappingURL=actions.js.map