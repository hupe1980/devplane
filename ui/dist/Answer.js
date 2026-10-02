import { A as comment, E as if_block, G as first_child, H as user_effect, J as proxy, K as only_child, L as get, O as set_text, T as key, V as template_effect, W as child, X as state, Y as set, Z as user_derived, et as pop, it as reset, j as from_html, k as append, o as prop, q as sibling, rt as next, tt as push } from "./api.js";
import { D as onAction, a as Inline, h as resource, i as Skeleton, s as ago } from "./app.js";
import { t as hideWindow } from "./frame.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
import { c as Commands } from "./unread.svelte.js";
import { i as snooze, n as answer, o as Item, r as copyRule, t as act } from "./actions.js";
//#region src/surfaces/answer/Answer.svelte
var root = from_html(`<p class="dim more svelte-1umb941"> </p>`);
var root_1 = from_html(`<!> <fieldset class="bare svelte-1umb941"><ul role="list" class="svelte-1umb941"><!></ul></fieldset> <!>`, 1);
var root_2 = from_html(`<p class="dim svelte-1umb941">Nothing needs you.</p>`);
var root_3 = from_html(`<section class="answer svelte-1umb941" aria-labelledby="answer-head"><h2 id="answer-head" class="svelte-1umb941">The one thing that needs you</h2> <p class="said svelte-1umb941" role="status" aria-live="polite"><!></p> <!> <!> <p class="hint dim svelte-1umb941">Esc hides this</p></section>`);
function Answer($$anchor, $$props) {
	push($$props, true);
	let items = prop($$props, "items", 3, null);
	const read = resource(() => "/api/inbox?needs_you=true", {
		every: 2e3,
		tell: () => "devplane inbox --needs-you"
	});
	let said = state("");
	let commands = state(proxy([]));
	const shown = user_derived(() => read.data?.items ?? items() ?? []);
	const first = user_derived(() => get(shown)[0] ?? null);
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
	var section = root_3();
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
		Failed($$anchor, {
			what: "what needs you",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_2 = ($$anchor) => {
		Skeleton($$anchor, { rows: 2 });
	};
	var consequent_5 = ($$anchor) => {
		var fragment_3 = root_1();
		var node_3 = first_child(fragment_3);
		var consequent_3 = ($$anchor) => {
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
		if_block(node_3, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_3);
		});
		var fieldset = sibling(node_3, 2);
		var ul = child(fieldset);
		var node_4 = child(ul);
		key(node_4, () => get(first).id, ($$anchor) => {
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
					say: (s) => set(said, s, true)
				});
			}
		});
		reset(ul);
		reset(fieldset);
		var node_5 = sibling(fieldset, 2);
		var consequent_4 = ($$anchor) => {
			var p_1 = root();
			var text = only_child(p_1);
			template_effect(() => set_text(text, `${get(shown).length - 1} more after this one`));
			append($$anchor, p_1);
		};
		if_block(node_5, ($$render) => {
			if (get(shown).length > 1) $$render(consequent_4);
		});
		template_effect(() => fieldset.disabled = !!pending.busy);
		append($$anchor, fragment_3);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_6 = comment();
		var node_6 = first_child(fragment_6);
		var consequent_6 = ($$anchor) => {
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
			var p_2 = root_2();
			append($$anchor, p_2);
		};
		if_block(node_6, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_6);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_6);
	};
	if_block(node_2, ($$render) => {
		if (get(failed) && read.failure) $$render(consequent_1);
		else if (!get(loaded)) $$render(consequent_2, 1);
		else if (get(first)) $$render(consequent_5, 2);
		else $$render(alternate_1, -1);
	});
	next(2);
	reset(section);
	append($$anchor, section);
	pop();
}
//#endregion
export { Answer as default };

//# sourceMappingURL=Answer.js.map