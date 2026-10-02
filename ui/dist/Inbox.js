import { A as append, D as if_block, E as key, F as delegate, G as child, H as template_effect, I as delegated, J as sibling, K as first_child, M as from_html, Q as user_derived, R as get, T as index, U as user_effect, X as set, Y as proxy, Z as state, at as reset, it as next, j as comment, k as set_text, m as set_attribute, nt as push, o as prop, q as only_child, st as to_array, tt as pop, w as each, y as set_class } from "./api.js";
import { C as place, F as spell, M as onAction, S as itemFocus, a as Inline, f as reread, i as Skeleton, n as Icon, s as ago, w as remember } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
import { s as Commands } from "./unread.svelte.js";
import { a as takeBack, i as snooze, n as answer, o as Item, r as copyRule, t as act } from "./actions.js";
import { t as narrowed } from "./narrow.svelte.js";
//#region src/surfaces/inbox/Inbox.svelte
var root = from_html(`<button class="undo svelte-ytvk4v"> <kbd class="svelte-ytvk4v">z</kbd></button>`);
var root_1 = from_html(`<p class="held svelte-ytvk4v" role="status"><span><b> </b> <kbd class="svelte-ytvk4v">z</kbd> to undo</span> <button class="undo svelte-ytvk4v">take it back <kbd class="svelte-ytvk4v">z</kbd></button></p>`);
var root_2 = from_html(`<p role="status"><span><b> </b> </span></p>`);
var root_3 = from_html(`<div class="missing svelte-ytvk4v" role="alert"><p class="svelte-ytvk4v"><b> </b> Nothing here is narrowed, and nothing here says the inbox is clear.</p> <button>Show the whole inbox</button></div>`);
var root_4 = from_html(`<p class="dim stale svelte-ytvk4v">The last read had nothing for you, and Devplane has not answered since.</p>`);
var root_5 = from_html(`<p class="svelte-ytvk4v">Nothing needed you, and nothing was decided for you.</p>`);
var root_6 = from_html(`<p class="svelte-ytvk4v"><!></p>`);
var root_7 = from_html(`<p class="dim svelte-ytvk4v"> </p>`);
var root_8 = from_html(`<p class="dim blind svelte-ytvk4v"><!></p>`);
var root_9 = from_html(`<div class="close svelte-ytvk4v"><span class="ok svelte-ytvk4v"><!></span> <h1 class="svelte-ytvk4v">Clear.</h1> <!> <!> <!> <!></div>`);
var root_10 = from_html(`<p class="hairline stale svelte-ytvk4v"> </p>`);
var root_11 = from_html(`<p class="gone svelte-ytvk4v" role="status"> </p>`);
var root_12 = from_html(`<span class="proj svelte-ytvk4v"><!> </span>`);
var root_13 = from_html(`<span class="age"> </span>`);
var root_14 = from_html(`<a class="svelte-ytvk4v"><!> Open the change</a>`);
var root_15 = from_html(`· <kbd class="svelte-ytvk4v"> </kbd> `, 1);
var root_16 = from_html(`· <kbd class="svelte-ytvk4v">z</kbd> undo`, 1);
var root_17 = from_html(`<!> <!> <header class="top svelte-ytvk4v"><span> </span> <span class="kind svelte-ytvk4v"> </span> <!> <!> <span class="pos svelte-ytvk4v"> </span></header> <fieldset class="bare svelte-ytvk4v"><ul role="list" tabindex="-1"><!></ul></fieldset> <footer class="nav svelte-ytvk4v"><!> <span class="keys svelte-ytvk4v"><kbd class="svelte-ytvk4v">j</kbd> <kbd class="svelte-ytvk4v">k</kbd> move between items · <kbd class="svelte-ytvk4v">Enter</kbd> moves into this one, <kbd class="svelte-ytvk4v">Tab</kbd> to its controls<!><!> · <kbd class="svelte-ytvk4v"> </kbd> every key</span></footer>`, 1);
var root_18 = from_html(`<section class="detail svelte-ytvk4v" aria-label="the item"><p role="status" aria-live="polite"><!> <!></p> <!> <!> <!></section>`);
function Inbox($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 19, () => []), folded = prop($$props, "folded", 19, () => []), inhibited = prop($$props, "inhibited", 19, () => []), close = prop($$props, "close", 3, null), project = prop($$props, "project", 3, ""), item = prop($$props, "item", 3, ""), wanted = prop($$props, "wanted", 3, ""), loaded = prop($$props, "loaded", 3, false), stale_since = prop($$props, "stale_since", 3, null), error = prop($$props, "error", 3, null), watching = prop($$props, "watching", 3, null), blind = prop($$props, "blind", 3, ""), held = prop($$props, "held", 3, null), landed = prop($$props, "landed", 3, null), open = prop($$props, "open", 3, () => {});
	const unseen = user_derived(() => {
		const v = [...watching()?.driven_only ?? [], ...watching()?.unproved ?? []];
		return v.length <= 1 ? v.join("") : `${v.slice(0, -1).join(", ")} and ${v[v.length - 1]}`;
	});
	const narrow = narrowed(() => project());
	const shown = user_derived(() => narrow.on ? narrow.data?.items ?? [] : items());
	const shownFolded = user_derived(() => narrow.on ? narrow.data?.folded ?? [] : folded());
	const shownInhibited = user_derived(() => narrow.on ? narrow.data?.inhibited ?? [] : inhibited());
	const shownClose = user_derived(() => narrow.on ? null : close());
	const ready = user_derived(() => narrow.on ? narrow.phase !== "loading" : loaded());
	const missing = user_derived(() => narrow.missing);
	let said = state(null);
	let undo = state(null);
	const pending = writer();
	async function report(item, work) {
		const r = await pending.run("action", work);
		if (!r) return;
		set(said, {
			text: r.said,
			id: item.id,
			title: item.title,
			commands: r.commands
		}, true);
		set(undo, r.undo ? {
			...r.undo,
			id: item.id
		} : null, true);
		reread();
		narrow.reload();
	}
	const act$1 = (item, action) => report(item, () => act(item, action));
	const answer$1 = (item, what) => report(item, () => answer(item, what));
	const snooze$1 = (item) => report(item, () => snooze(item));
	async function takeBack$1() {
		if (!get(undo)) return;
		const u = get(undo);
		set(undo, null);
		const r = await pending.run("undo", () => takeBack(u));
		if (r) set(said, {
			text: r.said,
			id: u.id,
			title: get(said)?.title ?? ""
		}, true);
		reread();
		narrow.reload();
	}
	async function copyRule$1(rule) {
		const item = get(current);
		const r = await copyRule(rule);
		if (item) set(said, {
			text: r.said,
			id: item.id,
			title: item.title
		}, true);
	}
	function say(text) {
		if (get(current)) set(said, {
			text,
			id: get(current).id,
			title: get(current).title
		}, true);
	}
	const nothingRaised = user_derived(() => get(shown).length === 0 && get(shownFolded).length === 0 && get(shownInhibited).length === 0);
	const placed = user_derived(() => place(get(shown), {
		item: item(),
		wanted: wanted()
	}));
	const current = user_derived(() => get(placed).current);
	const position = user_derived(() => Math.max(0, get(shown).findIndex((x) => x.id === get(current)?.id)));
	user_effect(() => {
		if (get(current) && get(current).id === item()) remember(item(), get(position));
	});
	const goneSays = user_derived(() => !get(ready) || !get(placed).gone ? "" : get(placed).gone === "ask" ? "The ask you followed was already answered." : get(said)?.id === item() ? "" : "That item was resolved — this is the next one.");
	const sentence = user_derived(() => {
		if (!get(said)) return "";
		if (get(said).id === get(current)?.id) return get(said).text;
		return get(shown).some((x) => x.id === get(said).id) ? "" : `${get(said).title}: ${get(said).text}`;
	});
	const undoHere = user_derived(() => !held() && get(undo) && (get(undo).id === get(current)?.id || !get(shown).some((x) => x.id === get(undo).id)) ? get(undo) : null);
	user_effect(() => cursor({
		scope: "inbox",
		size: () => get(shown).length,
		at: () => get(shown).findIndex((x) => x.id === get(current)?.id),
		go: (i) => open()(itemFocus(get(shown)[i].id)),
		open: () => document.querySelector(".detail .one")?.focus()
	}));
	const heldHere = user_derived(held);
	let mine = state(proxy([]));
	user_effect(() => {
		const h = held()?.ask;
		if (h && !get(mine).includes(h)) set(mine, [...get(mine).slice(-9), h], true);
	});
	const landedHere = user_derived(() => !held() && landed() && (!landed().ok || get(mine).includes(landed().ask)) ? landed() : null);
	let returning = state("");
	user_effect(() => {
		if (!get(returning)) return;
		const back = get(shown).find((x) => x.ask === get(returning) || x.request_id === get(returning));
		if (back) {
			set(returning, "");
			open()(itemFocus(back.id));
		}
	});
	async function takeBackHeld() {
		const h = held();
		if (!h) return;
		const r = await pending.run("undo", () => takeBack({ where: h.undo }));
		if (r) set(said, {
			text: r.said,
			id: "",
			title: h.title
		}, true);
		set(returning, h.ask, true);
		reread();
		narrow.reload();
	}
	user_effect(() => onAction("item-undo", (surface) => {
		if (surface !== "inbox") return false;
		if (pending.busy) return true;
		if (held()) takeBackHeld();
		else if (get(undoHere)) takeBack$1();
		else say(get(landedHere)?.ok ? "that answer was already sent — it can no longer be taken back" : "nothing to undo");
		return true;
	}));
	const keysHere = user_derived(() => {
		const a = get(current)?.actions ?? [];
		const out = [];
		const opts = (get(current)?.options ?? []).filter((o) => o.id).length || (get(current)?.form?.[0]?.options.length ?? 0);
		if (opts) out.push([opts > 1 ? `1–${Math.min(opts, 9)}` : "1", "choose"]);
		if (a.includes("reply") || get(current)?.form?.some((q) => q.custom_field)) out.push(["t", "type an answer"]);
		if (a.includes("allow")) out.push(["a", "allow"]);
		if (a.includes("deny")) out.push(["d", "deny"]);
		if (get(current)?.answer_in && (get(current).session_link || get(current).resume_command)) out.push(["w", get(current).session_link ? "open the session" : "copy the resume command"]);
		if (a.includes("snooze")) out.push(["s", "snooze"]);
		if ((a.includes("open") || a.includes("review")) && (get(current)?.change_id || get(current)?.run_id)) out.push(["o", "open"]);
		if (a.includes("review") && get(current)?.change_id) out.push(["r", "review"]);
		return out;
	});
	let tick = state(proxy(Date.now()));
	user_effect(() => {
		if (!held()) return;
		const id = setInterval(() => set(tick, Date.now(), true), 250);
		return () => clearInterval(id);
	});
	const heldLeft = user_derived(() => held() ? Math.max(0, Math.ceil((Date.parse(held().until) - get(tick)) / 1e3)) : 0);
	let now = state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(id);
	});
	var section = root_18();
	var p = child(section);
	let classes;
	var node = child(p);
	Inline(node, { get text() {
		return get(sentence);
	} });
	var node_1 = sibling(node, 2);
	var consequent = ($$anchor) => {
		var button = root();
		var text_1 = child(button);
		next();
		reset(button);
		template_effect(() => {
			button.disabled = !!pending.busy;
			set_text(text_1, `${get(undoHere).says ?? ""} `);
		});
		delegated("click", button, takeBack$1);
		append($$anchor, button);
	};
	if_block(node_1, ($$render) => {
		if (get(undoHere)) $$render(consequent);
	});
	reset(p);
	var node_2 = sibling(p, 2);
	var consequent_1 = ($$anchor) => {
		var p_1 = root_1();
		var span = child(p_1);
		var b = child(span);
		var text_2 = only_child(b, true);
		var text_3 = sibling(b);
		next(2);
		reset(span);
		var button_1 = sibling(span, 2);
		reset(p_1);
		template_effect(() => {
			set_text(text_2, get(heldHere).title);
			set_text(text_3, `: ${get(heldHere).says ?? ""} · ${get(heldLeft) > 0 ? `sending in ${get(heldLeft)} s` : "sending now"} · `);
			button_1.disabled = !!pending.busy;
		});
		delegated("click", button_1, takeBackHeld);
		append($$anchor, p_1);
	};
	var consequent_2 = ($$anchor) => {
		var p_2 = root_2();
		let classes_1;
		var span_1 = child(p_2);
		var b_1 = child(span_1);
		var text_4 = only_child(b_1, true);
		var text_5 = sibling(b_1);
		reset(span_1);
		reset(p_2);
		template_effect(() => {
			classes_1 = set_class(p_2, 1, "held svelte-ytvk4v", null, classes_1, { failed: !get(landedHere).ok });
			set_text(text_4, get(landedHere).title);
			set_text(text_5, `: ${get(landedHere).says ?? ""}`);
		});
		append($$anchor, p_2);
	};
	if_block(node_2, ($$render) => {
		if (get(heldHere)) $$render(consequent_1);
		else if (get(landedHere)) $$render(consequent_2, 1);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_3 = ($$anchor) => {
		Commands($$anchor, { get commands() {
			return get(said).commands;
		} });
	};
	if_block(node_3, ($$render) => {
		if (get(sentence) && get(said)?.commands?.length) $$render(consequent_3);
	});
	var node_4 = sibling(node_3, 2);
	var consequent_4 = ($$anchor) => {
		{
			let $0 = user_derived(() => `the inbox narrowed to ${project()}`);
			Failed($$anchor, {
				get what() {
					return get($0);
				},
				get failure() {
					return narrow.failure;
				}
			});
		}
	};
	var consequent_5 = ($$anchor) => {
		var div = root_3();
		var p_3 = child(div);
		var b_2 = child(p_3);
		var text_6 = only_child(b_2);
		next();
		reset(p_3);
		var button_2 = sibling(p_3, 2);
		reset(div);
		template_effect(() => set_text(text_6, `No project is named “${project() ?? ""}”.`));
		delegated("click", button_2, () => open()(""));
		append($$anchor, div);
	};
	var consequent_6 = ($$anchor) => {
		Skeleton($$anchor, {});
	};
	var consequent_7 = ($$anchor) => {
		var p_4 = root_4();
		append($$anchor, p_4);
	};
	var consequent_13 = ($$anchor) => {
		var div_1 = root_9();
		var span_2 = child(div_1);
		var node_5 = child(span_2);
		Icon(node_5, {
			name: "check",
			size: 22
		});
		reset(span_2);
		var node_6 = sibling(span_2, 4);
		var consequent_8 = ($$anchor) => {
			var p_5 = root_5();
			append($$anchor, p_5);
		};
		var alternate = ($$anchor) => {
			var fragment_3 = comment();
			var node_7 = first_child(fragment_3);
			each(node_7, 17, () => get(shownClose)?.sentences ?? [], index, ($$anchor, s) => {
				var p_6 = root_6();
				var node_8 = child(p_6);
				Inline(node_8, { get text() {
					return get(s);
				} });
				reset(p_6);
				append($$anchor, p_6);
			});
			append($$anchor, fragment_3);
		};
		if_block(node_6, ($$render) => {
			if (get(shownClose)?.quiet) $$render(consequent_8);
			else $$render(alternate, -1);
		});
		var node_9 = sibling(node_6, 2);
		var consequent_9 = ($$anchor) => {
			var p_7 = root_7();
			var text_7 = only_child(p_7);
			template_effect(() => set_text(text_7, `Next · ${get(shownClose).next ?? ""}`));
			append($$anchor, p_7);
		};
		if_block(node_9, ($$render) => {
			if (get(shownClose)?.next) $$render(consequent_9);
		});
		var node_10 = sibling(node_9, 2);
		var consequent_10 = ($$anchor) => {
			var p_8 = root_7();
			var text_8 = only_child(p_8, true);
			template_effect(() => set_text(text_8, get(shownClose).keeps_running));
			append($$anchor, p_8);
		};
		if_block(node_10, ($$render) => {
			if (get(shownClose)?.keeps_running) $$render(consequent_10);
		});
		var node_11 = sibling(node_10, 2);
		var consequent_11 = ($$anchor) => {
			var p_9 = root_8();
			var node_12 = child(p_9);
			Inline(node_12, { get text() {
				return blind();
			} });
			reset(p_9);
			append($$anchor, p_9);
		};
		var consequent_12 = ($$anchor) => {
			var p_10 = root_7();
			var text_9 = only_child(p_10);
			template_effect(() => set_text(text_9, `Sessions in ${get(unseen) ?? ""} are not on this board unless Devplane started them.`));
			append($$anchor, p_10);
		};
		if_block(node_11, ($$render) => {
			if (blind()) $$render(consequent_11);
			else if (get(unseen)) $$render(consequent_12, 1);
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	var consequent_21 = ($$anchor) => {
		var fragment_4 = root_17();
		var node_13 = first_child(fragment_4);
		var consequent_14 = ($$anchor) => {
			{
				let $0 = user_derived(() => `the inbox narrowed to ${project()}`);
				Failed($$anchor, {
					get what() {
						return get($0);
					},
					get failure() {
						return narrow.failure;
					},
					get at() {
						return narrow.at;
					},
					stale: true
				});
			}
		};
		var consequent_15 = ($$anchor) => {
			var p_11 = root_10();
			var text_10 = only_child(p_11);
			template_effect(($0) => set_text(text_10, `stale · as read ${$0 ?? ""}`), [() => stale_since() ? ago(Math.max(0, (Date.now() - Date.parse(stale_since())) / 1e3)) + " ago" : "before Devplane stopped answering"]);
			append($$anchor, p_11);
		};
		if_block(node_13, ($$render) => {
			if (narrow.on && narrow.failure) $$render(consequent_14);
			else if (error()) $$render(consequent_15, 1);
		});
		var node_14 = sibling(node_13, 2);
		var consequent_16 = ($$anchor) => {
			var p_12 = root_11();
			var text_11 = only_child(p_12, true);
			template_effect(() => set_text(text_11, get(goneSays)));
			append($$anchor, p_12);
		};
		if_block(node_14, ($$render) => {
			if (get(goneSays)) $$render(consequent_16);
		});
		var header = sibling(node_14, 2);
		var span_3 = child(header);
		var text_12 = only_child(span_3, true);
		var span_4 = sibling(span_3, 2);
		var text_13 = only_child(span_4, true);
		var node_15 = sibling(span_4, 2);
		var consequent_17 = ($$anchor) => {
			var span_5 = root_12();
			var node_16 = child(span_5);
			Icon(node_16, {
				name: "folder",
				size: 12
			});
			var text_14 = sibling(node_16);
			reset(span_5);
			template_effect(() => set_text(text_14, ` ${get(current).project_name ?? ""}`));
			append($$anchor, span_5);
		};
		if_block(node_15, ($$render) => {
			if (get(current).project_name) $$render(consequent_17);
		});
		var node_17 = sibling(node_15, 2);
		var consequent_18 = ($$anchor) => {
			var span_6 = root_13();
			var text_15 = only_child(span_6);
			template_effect(($0) => {
				set_attribute(span_6, "title", get(current).since);
				set_text(text_15, `${$0 ?? ""} ago`);
			}, [() => ago(Math.max(0, (get(now) - Date.parse(get(current).since)) / 1e3))]);
			append($$anchor, span_6);
		};
		if_block(node_17, ($$render) => {
			if (get(current).since) $$render(consequent_18);
		});
		var span_7 = sibling(node_17, 2);
		var text_16 = only_child(span_7);
		reset(header);
		var fieldset = sibling(header, 2);
		var ul = child(fieldset);
		let classes_2;
		var node_18 = child(ul);
		key(node_18, () => get(current).id, ($$anchor) => {
			Item($$anchor, {
				get item() {
					return get(current);
				},
				current: true,
				age: "",
				answer: answer$1,
				act: act$1,
				snooze: snooze$1,
				copyRule: copyRule$1,
				say,
				keys: "inbox"
			});
		});
		reset(ul);
		reset(fieldset);
		var footer = sibling(fieldset, 2);
		var node_19 = child(footer);
		var consequent_19 = ($$anchor) => {
			var a_1 = root_14();
			var node_20 = child(a_1);
			Icon(node_20, {
				name: "change",
				size: 13
			});
			next();
			reset(a_1);
			template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#change/${encodeURIComponent(get(current).change_id)}`]);
			append($$anchor, a_1);
		};
		if_block(node_19, ($$render) => {
			if (get(current).change_id) $$render(consequent_19);
		});
		var span_8 = sibling(node_19, 2);
		var node_21 = sibling(child(span_8), 8);
		each(node_21, 17, () => get(keysHere), ([k, what]) => k, ($$anchor, $$item) => {
			var $$array = user_derived(() => to_array(get($$item), 2));
			let k = () => get($$array)[0];
			let what = () => get($$array)[1];
			next();
			var fragment_7 = root_15();
			var kbd = sibling(first_child(fragment_7));
			var text_17 = only_child(kbd, true);
			var text_18 = sibling(kbd);
			template_effect(() => {
				set_text(text_17, k());
				set_text(text_18, ` ${what() ?? ""}`);
			});
			append($$anchor, fragment_7);
		});
		var node_22 = sibling(node_21);
		var consequent_20 = ($$anchor) => {
			var fragment_8 = root_16();
			next(2);
			append($$anchor, fragment_8);
		};
		if_block(node_22, ($$render) => {
			if (get(heldHere) || get(undoHere)) $$render(consequent_20);
		});
		var kbd_1 = sibling(node_22, 2);
		var text_19 = only_child(kbd_1, true);
		next();
		reset(span_8);
		reset(footer);
		template_effect(($0, $1) => {
			set_class(span_3, 1, `lvl ${get(current).level ?? ""}`, "svelte-ytvk4v");
			set_text(text_12, get(current).level);
			set_text(text_13, $0);
			set_text(text_16, `item ${get(position) + 1} · ${get(shown).length ?? ""} in the list`);
			fieldset.disabled = !!pending.busy;
			classes_2 = set_class(ul, 1, "one svelte-ytvk4v", null, classes_2, { stale: !!error() });
			set_attribute(ul, "aria-label", `the item: ${get(current).title ?? ""}`);
			set_text(text_19, $1);
		}, [() => get(current).kind.replace(/_/g, " "), () => spell("?")]);
		append($$anchor, fragment_4);
	};
	if_block(node_4, ($$render) => {
		if (narrow.on && narrow.failure && !narrow.data) $$render(consequent_4);
		else if (get(missing)) $$render(consequent_5, 1);
		else if (!get(ready) && get(nothingRaised)) $$render(consequent_6, 2);
		else if (get(nothingRaised) && error()) $$render(consequent_7, 3);
		else if (get(nothingRaised)) $$render(consequent_13, 4);
		else if (get(current)) $$render(consequent_21, 5);
	});
	reset(section);
	template_effect(() => classes = set_class(p, 1, "said svelte-ytvk4v", null, classes, { empty: !get(sentence) && !get(undoHere) }));
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
export { Inbox as default };

//# sourceMappingURL=Inbox.js.map