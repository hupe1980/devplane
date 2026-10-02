import { A as append, D as if_block, E as key, F as delegate, G as child, H as template_effect, I as delegated, J as sibling, K as first_child, M as from_html, Q as user_derived, R as get, U as user_effect, X as set, Y as proxy, Z as state, at as reset, it as next, j as comment, k as set_text, m as set_attribute, nt as push, o as prop, q as only_child, tt as pop } from "./api.js";
import { M as onAction, a as Inline, h as resource, i as Skeleton, s as ago } from "./app.js";
import { t as hideWindow } from "./frame.js";
import { t as cursor } from "./cursor.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
import { s as Commands } from "./unread.svelte.js";
import { a as takeBack, i as snooze, n as answer, o as Item, r as copyRule, t as act } from "./actions.js";
//#region src/surfaces/answer/Answer.svelte
var root = from_html(`<p class="held svelte-1umb941" role="status"><b> </b> <kbd class="svelte-1umb941">z</kbd> to undo <button>take it back <kbd class="svelte-1umb941">z</kbd></button></p>`);
var root_1 = from_html(`<p class="held failed svelte-1umb941" role="status"><b> </b> </p>`);
var root_2 = from_html(`<p class="dim more svelte-1umb941"> <kbd class="svelte-1umb941">j</kbd> <kbd class="svelte-1umb941">k</kbd> move between them</p>`);
var root_3 = from_html(`<!> <fieldset class="bare svelte-1umb941"><ul role="list" class="one svelte-1umb941" tabindex="-1"><!></ul></fieldset> <!>`, 1);
var root_4 = from_html(`<p class="dim svelte-1umb941"><!></p>`);
var root_5 = from_html(`<p class="dim svelte-1umb941">Nothing needs you.</p> <!>`, 1);
var root_6 = from_html(`<section class="answer svelte-1umb941" aria-labelledby="answer-head"><h2 id="answer-head" class="svelte-1umb941">The one thing that needs you</h2> <p class="said svelte-1umb941" role="status" aria-live="polite"><!></p> <!> <!> <!> <p class="hint dim svelte-1umb941">Esc hides this · ? lists every key</p></section>`);
function Answer($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 3, null), blind = prop($$props, "blind", 3, "");
	const read = resource(() => "/api/inbox?needs_you=true", {
		every: 2e3,
		tell: () => "devplane inbox --needs-you"
	});
	let said = state("");
	let commands = state(proxy([]));
	const shown = user_derived(() => read.data?.items ?? items() ?? []);
	let at = state("");
	const index = user_derived(() => Math.max(0, get(shown).findIndex((x) => x.id === get(at))));
	const first = user_derived(() => get(shown)[get(index)] ?? get(shown)[0] ?? null);
	const held = user_derived(() => read.data?.held ?? null);
	const landed = user_derived(() => read.data?.landed ?? null);
	user_effect(() => cursor({
		scope: "answer",
		size: () => get(shown).length,
		at: () => get(index),
		go: (i) => set(at, get(shown)[i].id, true),
		open: () => document.querySelector(".answer .one")?.focus()
	}));
	let returning = state("");
	user_effect(() => {
		if (!get(returning)) return;
		const back = get(shown).find((x) => x.ask === get(returning) || x.request_id === get(returning));
		if (back) {
			set(returning, "");
			set(at, back.id, true);
		}
	});
	async function takeBack$1() {
		const h = get(held);
		if (!h) return;
		const r = await pending.run("undo", () => takeBack({ where: h.undo }));
		if (r) set(said, r.said, true);
		set(returning, h.ask, true);
		await read.reload();
	}
	user_effect(() => onAction("item-undo", (surface) => {
		if (surface !== "answer") return false;
		if (get(held)) takeBack$1();
		else set(said, get(landed)?.ok ? "that answer was already sent — it can no longer be taken back" : "nothing to undo", true);
		return true;
	}));
	let tick = state(proxy(Date.now()));
	user_effect(() => {
		if (!get(held)) return;
		const id = setInterval(() => set(tick, Date.now(), true), 250);
		return () => clearInterval(id);
	});
	const left = user_derived(() => get(held) ? Math.max(0, Math.ceil((Date.parse(get(held).until) - get(tick)) / 1e3)) : 0);
	const failed = user_derived(() => read.phase === "failed" && items() === null);
	const loaded = user_derived(() => read.data !== null || items() !== null);
	user_effect(() => onAction("leave", () => {
		hideWindow();
		return true;
	}));
	const pending = writer();
	async function report(work) {
		const r = await pending.run("action", work);
		if (!r) return;
		set(said, r.said, true);
		set(commands, r.commands ?? [], true);
		await read.reload();
	}
	const answer$1 = (item, what) => report(() => answer(item, what));
	const act$1 = (item, action) => report(() => act(item, action));
	const snooze$1 = (item) => report(() => snooze(item));
	async function copyRule$1(rule) {
		set(said, (await copyRule(rule)).said, true);
	}
	let now = state(proxy(Date.now()));
	user_effect(() => {
		const id = setInterval(() => set(now, Date.now(), true), 3e4);
		return () => clearInterval(id);
	});
	var section = root_6();
	var p = sibling(child(section), 2);
	var node = child(p);
	Inline(node, { get text() {
		return get(said);
	} });
	reset(p);
	var node_1 = sibling(p, 2);
	var consequent = ($$anchor) => {
		Commands($$anchor, { get commands() {
			return get(commands);
		} });
	};
	if_block(node_1, ($$render) => {
		if (get(commands).length) $$render(consequent);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var p_1 = root();
		var b = child(p_1);
		var text = only_child(b, true);
		var text_1 = sibling(b);
		var button = sibling(text_1, 3);
		reset(p_1);
		template_effect(() => {
			set_text(text, get(held).title);
			set_text(text_1, `: ${get(held).says ?? ""} · ${get(left) > 0 ? `sending in ${get(left)} s` : "sending now"} · `);
			button.disabled = !!pending.busy;
		});
		delegated("click", button, takeBack$1);
		append($$anchor, p_1);
	};
	var consequent_2 = ($$anchor) => {
		var p_2 = root_1();
		var b_1 = child(p_2);
		var text_2 = only_child(b_1, true);
		var text_3 = sibling(b_1);
		reset(p_2);
		template_effect(() => {
			set_text(text_2, get(landed).title);
			set_text(text_3, `: ${get(landed).says ?? ""}`);
		});
		append($$anchor, p_2);
	};
	if_block(node_2, ($$render) => {
		if (get(held)) $$render(consequent_1);
		else if (get(landed) && !get(landed).ok) $$render(consequent_2, 1);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_3 = ($$anchor) => {
		Failed($$anchor, {
			what: "what needs you",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_4 = ($$anchor) => {
		Skeleton($$anchor, { rows: 2 });
	};
	var consequent_7 = ($$anchor) => {
		var fragment_3 = root_3();
		var node_4 = first_child(fragment_3);
		var consequent_5 = ($$anchor) => {
			Failed($$anchor, {
				what: "what needs you",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		if_block(node_4, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_5);
		});
		var fieldset = sibling(node_4, 2);
		var ul = child(fieldset);
		var node_5 = child(ul);
		key(node_5, () => get(first).id, ($$anchor) => {
			{
				let $0 = user_derived(() => get(first).since ? ago(Math.max(0, (get(now) - Date.parse(get(first).since)) / 1e3)) : "");
				Item($$anchor, {
					get item() {
						return get(first);
					},
					current: true,
					get age() {
						return get($0);
					},
					answer: answer$1,
					act: act$1,
					snooze: snooze$1,
					copyRule: copyRule$1,
					say: (s) => set(said, s, true),
					keys: "answer"
				});
			}
		});
		reset(ul);
		reset(fieldset);
		var node_6 = sibling(fieldset, 2);
		var consequent_6 = ($$anchor) => {
			var p_3 = root_2();
			var text_4 = child(p_3);
			next(4);
			reset(p_3);
			template_effect(() => set_text(text_4, `item ${get(index) + 1} of ${get(shown).length ?? ""} · `));
			append($$anchor, p_3);
		};
		if_block(node_6, ($$render) => {
			if (get(shown).length > 1) $$render(consequent_6);
		});
		template_effect(() => {
			fieldset.disabled = !!pending.busy;
			set_attribute(ul, "aria-label", `the item: ${get(first).title ?? ""}`);
		});
		append($$anchor, fragment_3);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_6 = comment();
		var node_7 = first_child(fragment_6);
		var consequent_8 = ($$anchor) => {
			Failed($$anchor, {
				what: "what needs you",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		var alternate = ($$anchor) => {
			var fragment_8 = root_5();
			var node_8 = sibling(first_child(fragment_8), 2);
			var consequent_9 = ($$anchor) => {
				var p_4 = root_4();
				var node_9 = child(p_4);
				Inline(node_9, { get text() {
					return blind();
				} });
				reset(p_4);
				append($$anchor, p_4);
			};
			if_block(node_8, ($$render) => {
				if (blind()) $$render(consequent_9);
			});
			append($$anchor, fragment_8);
		};
		if_block(node_7, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_8);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_6);
	};
	if_block(node_3, ($$render) => {
		if (get(failed) && read.failure) $$render(consequent_3);
		else if (!get(loaded)) $$render(consequent_4, 1);
		else if (get(first)) $$render(consequent_7, 2);
		else $$render(alternate_1, -1);
	});
	next(2);
	reset(section);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
export { Answer as default };

//# sourceMappingURL=Answer.js.map