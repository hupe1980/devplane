import { $ as user_effect, A as clsx, B as append, F as index, G as delegate, H as from_html, J as get, K as delegated, L as if_block, N as snippet, O as set_style, P as each, Q as template_effect, S as bind_value, V as comment, W as text, X as untrack, _t as to_array, at as proxy, ct as user_derived, d as onAction, dt as pop, et as user_pre_effect, ft as push, g as prop, gt as noop, h as spell, ht as reset, i as api, it as sibling, k as set_class, mt as next, nt as first_child, ot as set, p as setMode, q as event, rt as only_child, st as state, t as Refused, tt as child, ut as remove_textarea_child, w as set_attribute, y as bind_this, z as set_text } from "./api.js";
import { n as failure, r as resource, t as copyText } from "./resource.svelte.js";
import { O as surfaces, S as STATES, a as Inline, d as shape, i as Skeleton, n as Icon, o as blocks, r as Split, t as Pill, x as LIFE } from "./app.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
import { a as unreadIn, i as unread, n as markSeen, o as unreadSays, r as unitSeen, s as Commands, t as isSeen } from "./unread.svelte.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
import { t as Props } from "./Props.js";
import { n as offerCommands } from "./offered.svelte.js";
import { n as weakLeads, t as ChangeState } from "./ChangeState.js";
import { t as Tabs } from "./Tabs.js";
//#region src/lib/NoCheck.svelte
function declaresNoCheck(change) {
	if (!change) return false;
	return change.standing?.standing === "no_gates_declared" || /^no gates declared/i.test(change.standing_says ?? "");
}
var root$19 = from_html(`<a class="declare svelte-ojuoew">Declare a check</a>`);
var root_1$18 = from_html(`<p role="note"><!> <span>This change cannot become verified until a check is declared.</span> <!></p>`);
function NoCheck($$anchor, $$props) {
	push($$props, true);
	let compact = prop($$props, "compact", 3, false);
	const at = user_derived(() => surfaces().find((s) => s.holds === "gates")?.id ?? "");
	const href = user_derived(() => get(at) ? `#${get(at)}/declare/${encodeURIComponent($$props.project)}` : "");
	var p = root_1$18();
	let classes;
	var node = child(p);
	Icon(node, {
		name: "gate",
		size: 13
	});
	var node_1 = sibling(node, 4);
	var consequent = ($$anchor) => {
		var a = root$19();
		template_effect(() => set_attribute(a, "href", get(href)));
		append($$anchor, a);
	};
	if_block(node_1, ($$render) => {
		if (get(href)) $$render(consequent);
	});
	reset(p);
	template_effect(() => classes = set_class(p, 1, "nocheck svelte-ojuoew", null, classes, { compact: compact() }));
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/surfaces/change/Stepper.svelte
var VERBS = [
	"isolate it",
	"start an agent",
	"run the gates",
	"offer",
	"archive",
	""
];
var NEXT = Object.fromEntries(LIFE.map((l, i) => [l, VERBS[i]]));
function where(state, archived, offered, weakUnread = 0, noCheck = false) {
	const at = archived ? STATES.archived.word : offered ? STATES.offered.word : state;
	const step = LIFE.includes(at) ? at : null;
	if (weakUnread > 0 && at !== STATES.archived.word) return {
		at,
		next: `read ${weakUnread} weakened ${weakUnread === 1 ? "check" : "checks"}`
	};
	const next = step ? NEXT[step] : "";
	return {
		at,
		next: noCheck && next === NEXT[LIFE[2]] ? "declare a check" : next
	};
}
var root$18 = from_html(`<span class="arrow svelte-bllsbm" aria-hidden="true">→</span><span class="next"> </span>`, 1);
var root_1$17 = from_html(`<p class="stepper svelte-bllsbm" aria-label="where this change is" title="drafted → isolated → in flight → verified → offered → archived"><span class="at svelte-bllsbm" aria-current="step"> </span><!></p>`);
function Stepper($$anchor, $$props) {
	push($$props, true);
	let archived = prop($$props, "archived", 3, false), offered = prop($$props, "offered", 3, false), weakUnread = prop($$props, "weakUnread", 3, 0), noCheck = prop($$props, "noCheck", 3, false), qualified = prop($$props, "qualified", 3, "");
	const line = user_derived(() => where($$props.state, archived(), offered(), weakUnread(), noCheck()));
	const at = user_derived(() => qualified() && get(line).at === STATES.verified.word ? `${qualified()} · ${get(line).at}` : get(line).at);
	var p = root_1$17();
	var span = child(p);
	var text = only_child(span, true);
	var node = sibling(span);
	var consequent = ($$anchor) => {
		var fragment = root$18();
		var span_1 = sibling(first_child(fragment));
		var text_1 = only_child(span_1);
		template_effect(() => set_text(text_1, `next: ${get(line).next ?? ""}`));
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (get(line).next) $$render(consequent);
	});
	reset(p);
	template_effect(() => set_text(text, get(at)));
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/surfaces/change/Claim.svelte
var root$17 = from_html(`<code class="svelte-sf1onp"> </code>`);
var root_1$16 = from_html(`<p class="paths svelte-sf1onp"></p>`);
var root_2$16 = from_html(`<button class="svelte-sf1onp"><!> Tell the agent</button>`);
var root_3$15 = from_html(`<form class="tell svelte-sf1onp"><textarea rows="2" placeholder="Your words — sent to the agent exactly as written" aria-label="what to tell the agent" class="svelte-sf1onp"></textarea> <button type="submit" class="svelte-sf1onp">Send</button></form>`);
var root_4$12 = from_html(`<p class="said svelte-sf1onp" role="status"> </p>`);
var root_5$12 = from_html(`<section class="claim svelte-sf1onp" aria-label="the agent's claim"><p class="says svelte-sf1onp"><!> <b> </b> <span class="who svelte-sf1onp"> <span class="mono svelte-sf1onp"> </span><!></span></p> <blockquote class="svelte-sf1onp"> </blockquote> <!> <p class="acts svelte-sf1onp"><span class="note svelte-sf1onp">The agent's words, quoted; not a finding, and not an instruction.</span> <!> <button class="svelte-sf1onp"><!> Mark read</button></p> <!> <!></section>`);
function Claim($$anchor, $$props) {
	push($$props, true);
	let done = prop($$props, "done", 3, () => {});
	let telling = state(false);
	let words = state("");
	let busy = state(false);
	let said = state("");
	const at = user_derived(() => $$props.claim.at ? $$props.claim.at.slice(0, 16).replace("T", " ") : "");
	async function markRead() {
		if (get(busy)) return;
		set(busy, true);
		try {
			const r = await api(`/api/changes/${encodeURIComponent($$props.change)}/claim/read`, { method: "POST" });
			set(said, r?.says ?? "Marked read.", true);
			done()();
		} catch (e) {
			set(said, `That did not land: ${failure(e, `devplane change claim-read ${$$props.change}`).says}`);
		} finally {
			set(busy, false);
		}
	}
	async function tell() {
		const text = get(words).trim();
		if (!text || get(busy)) return;
		set(busy, true);
		try {
			await api(`/api/changes/${encodeURIComponent($$props.change)}/resume`, { method: "POST" });
			const r = await api(`/api/runs/${encodeURIComponent($$props.claim.run)}/prompt`, {
				method: "POST",
				body: JSON.stringify({ text })
			});
			set(words, "");
			set(telling, false);
			set(said, r?.says ?? "Sent.", true);
			done()();
		} catch (e) {
			set(said, `Nothing was sent: ${failure(e, `devplane change resume ${$$props.change}`).says}`);
		} finally {
			set(busy, false);
		}
	}
	var section = root_5$12();
	var p_1 = child(section);
	var node = child(p_1);
	Icon(node, {
		name: "alert",
		size: 13
	});
	var b = sibling(node, 2);
	var text_1 = only_child(b, true);
	var span = sibling(b, 2);
	var text_2 = child(span);
	var span_1 = sibling(text_2);
	var text_3 = only_child(span_1, true);
	var node_1 = sibling(span_1);
	var consequent = ($$anchor) => {
		var text_4 = text();
		template_effect(() => set_text(text_4, `, ${get(at) ?? ""} UTC`));
		append($$anchor, text_4);
	};
	if_block(node_1, ($$render) => {
		if (get(at)) $$render(consequent);
	});
	reset(span);
	reset(p_1);
	var blockquote = sibling(p_1, 2);
	var text_5 = only_child(blockquote, true);
	var node_2 = sibling(blockquote, 2);
	var consequent_1 = ($$anchor) => {
		var p_2 = root_1$16();
		each(p_2, 20, () => $$props.claim.paths, (p) => p, ($$anchor, p) => {
			var code = root$17();
			var text_6 = only_child(code, true);
			template_effect(() => set_text(text_6, p));
			append($$anchor, code);
		});
		reset(p_2);
		append($$anchor, p_2);
	};
	if_block(node_2, ($$render) => {
		if ($$props.claim.paths.length) $$render(consequent_1);
	});
	var p_3 = sibling(node_2, 2);
	var node_3 = sibling(child(p_3), 2);
	var consequent_2 = ($$anchor) => {
		var button = root_2$16();
		var node_4 = child(button);
		Icon(node_4, {
			name: "agent",
			size: 13
		});
		next();
		reset(button);
		template_effect(() => {
			button.disabled = get(busy);
			set_attribute(button, "aria-expanded", get(telling));
		});
		delegated("click", button, () => set(telling, !get(telling)));
		append($$anchor, button);
	};
	if_block(node_3, ($$render) => {
		if ($$props.claim.can_tell) $$render(consequent_2);
	});
	var button_1 = sibling(node_3, 2);
	var node_5 = child(button_1);
	Icon(node_5, {
		name: "check",
		size: 13
	});
	next();
	reset(button_1);
	reset(p_3);
	var node_6 = sibling(p_3, 2);
	var consequent_3 = ($$anchor) => {
		var form = root_3$15();
		var textarea = child(form);
		remove_textarea_child(textarea);
		var button_2 = sibling(textarea, 2);
		reset(form);
		template_effect(($0) => button_2.disabled = $0, [() => get(busy) || !get(words).trim()]);
		event("submit", form, (e) => (e.preventDefault(), tell()));
		bind_value(textarea, () => get(words), ($$value) => set(words, $$value));
		append($$anchor, form);
	};
	if_block(node_6, ($$render) => {
		if (get(telling)) $$render(consequent_3);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_4 = ($$anchor) => {
		var p_4 = root_4$12();
		var text_7 = only_child(p_4, true);
		template_effect(() => set_text(text_7, get(said)));
		append($$anchor, p_4);
	};
	if_block(node_7, ($$render) => {
		if (get(said)) $$render(consequent_4);
	});
	reset(section);
	template_effect(() => {
		set_text(text_1, $$props.claim.says);
		set_text(text_2, `— ${$$props.claim.agent ?? ""}, run `);
		set_text(text_3, $$props.claim.run);
		set_text(text_5, $$props.claim.reason);
		button_1.disabled = get(busy);
	});
	delegated("click", button_1, markRead);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Head.svelte
var root$16 = from_html(`<dt class="svelte-axcc9r">Project</dt><dd class="svelte-axcc9r"> </dd>`, 1);
var root_1$15 = from_html(`<dt class="svelte-axcc9r">Branch</dt><dd class="svelte-axcc9r"><button class="link mono svelte-axcc9r" title="copy the branch name"> </button></dd>`, 1);
var root_2$15 = from_html(`<dt class="svelte-axcc9r">Specification</dt><dd class="mono svelte-axcc9r"> </dd>`, 1);
var root_3$14 = from_html(`<dt class="svelte-axcc9r">Size</dt><dd class="svelte-axcc9r"> </dd>`, 1);
var root_4$11 = from_html(`<dt class="svelte-axcc9r">Worktree</dt><dd class="mono svelte-axcc9r"> </dd>`, 1);
var root_5$11 = from_html(`<span class="waiting svelte-axcc9r"><!></span>`);
var root_6$9 = from_html(`<span class="warn svelte-axcc9r"><!> in place — no parallel safety</span>`);
var root_7$9 = from_html(`<button class="branch mono svelte-axcc9r" title="copy the branch name"><!> </button>`);
var root_8$8 = from_html(`<button><!> Review</button>`);
var root_9$7 = from_html(`<button aria-label="Run gates" title="Run gates" class="svelte-axcc9r"><!> <span class="label svelte-axcc9r">Run gates</span></button>`);
var root_10$6 = from_html(`<button disabled="" aria-label="Offer as pull request" aria-describedby="offer-held" class="svelte-axcc9r"><!> Offer</button> <span class="held svelte-axcc9r" id="offer-held"> </span>`, 1);
var root_11$6 = from_html(`<li class="mono svelte-axcc9r"> </li>`);
var root_12$6 = from_html(`<details class="pop opened svelte-axcc9r"><summary class="held svelte-axcc9r"> </summary> <ul class="card svelte-axcc9r"></ul></details>`);
var root_13$6 = from_html(`<button><!> <span class="full svelte-axcc9r">Offer as pull request</span><span class="short svelte-axcc9r" aria-hidden="true">Offer</span></button> <!>`, 1);
var root_14$5 = from_html(`<button class="svelte-axcc9r"><!> Try again</button>`);
var root_15$2 = from_html(`<button class="svelte-axcc9r"><!> Pick it back up</button> <!>`, 1);
var root_16$2 = from_html(`<button role="menuitem" class="svelte-axcc9r"><!> Finish</button>`);
var root_17$2 = from_html(`<button role="menuitem" class="svelte-axcc9r"><!> Open in the editor</button> <button role="menuitem" class="svelte-axcc9r"><!> Open a terminal</button>`, 1);
var root_18$2 = from_html(`<button role="menuitem" title="remove the worktree; the branch and the record are kept" class="svelte-axcc9r"><!> Archive</button>`);
var root_19$1 = from_html(`<details class="pop more svelte-axcc9r"><summary aria-label="More actions" title="More actions" class="svelte-axcc9r"><!> <span class="label svelte-axcc9r">More</span></summary> <div class="card menu svelte-axcc9r" role="menu"><!> <!> <!></div></details>`);
var root_20$1 = from_html(`<span><!></span>`);
var root_21$1 = from_html(`<span> </span>`);
var root_22$1 = from_html(`<span class="counts svelte-axcc9r"> <!></span>`);
var root_23$1 = from_html(`<p class="sub svelte-axcc9r"><!> <!></p>`);
var root_24$1 = from_html(`<header class="head svelte-axcc9r"><div class="line svelte-axcc9r"><h1 class="svelte-axcc9r"> </h1> <details class="pop state svelte-axcc9r"><summary title="where this change is, and its facts" class="svelte-axcc9r"><!><!></summary> <div class="card svelte-axcc9r"><!> <dl class="svelte-axcc9r"><!> <!> <!> <!> <!></dl></div></details> <!> <!> <!> <span class="gap svelte-axcc9r"></span> <div role="toolbar" aria-label="what you can do to this change"><!> <!> <!> <!> <!></div></div> <!> <!></header>`);
function Head($$anchor, $$props) {
	push($$props, true);
	let notOpened = prop($$props, "notOpened", 19, () => []), reload = prop($$props, "reload", 3, () => {});
	const project = user_derived(() => ($$props.d.project_id ?? "").split("/").filter(Boolean).pop() ?? "");
	const noCheck = user_derived(() => declaresNoCheck($$props.d));
	const reviewing = user_derived(() => $$props.view === "review");
	const more = user_derived(() => !!$$props.d.worktree || !$$props.d.completion && !$$props.d.archived_at && !!$$props.d.runs?.length || !$$props.d.archived_at && (!!$$props.d.completion || !!$$props.d.stopped));
	let menu = state(null);
	const shut = () => get(menu)?.removeAttribute("open");
	var header = root_24$1();
	var div = child(header);
	var h1 = child(div);
	var text = only_child(h1, true);
	var details = sibling(h1, 2);
	var summary = child(details);
	var node = child(summary);
	ChangeState(node, {
		get state() {
			return $$props.d.state;
		},
		get q() {
			return $$props.d.qualifier;
		}
	});
	var node_1 = sibling(node);
	Icon(node_1, {
		name: "down",
		size: 11
	});
	reset(summary);
	var div_1 = sibling(summary, 2);
	var node_2 = child(div_1);
	{
		let $0 = user_derived(() => !!$$props.d.archived_at);
		let $1 = user_derived(() => !!$$props.d.pull_request);
		let $2 = user_derived(() => $$props.d.qualifier?.says ?? "");
		Stepper(node_2, {
			get state() {
				return $$props.d.state;
			},
			get archived() {
				return get($0);
			},
			get offered() {
				return get($1);
			},
			get weakUnread() {
				return $$props.unseen;
			},
			get noCheck() {
				return get(noCheck);
			},
			get qualified() {
				return get($2);
			}
		});
	}
	var dl = sibling(node_2, 2);
	var node_3 = child(dl);
	var consequent = ($$anchor) => {
		var fragment = root$16();
		var dd = sibling(first_child(fragment));
		var text_1 = only_child(dd, true);
		template_effect(() => set_text(text_1, get(project)));
		append($$anchor, fragment);
	};
	if_block(node_3, ($$render) => {
		if (get(project)) $$render(consequent);
	});
	var node_4 = sibling(node_3, 2);
	var consequent_1 = ($$anchor) => {
		var fragment_1 = root_1$15();
		var dd_1 = sibling(first_child(fragment_1));
		var button = child(dd_1);
		var text_2 = only_child(button, true);
		reset(dd_1);
		template_effect(() => set_text(text_2, $$props.d.branch));
		delegated("click", button, () => $$props.copy($$props.d.branch, "the branch name"));
		append($$anchor, fragment_1);
	};
	if_block(node_4, ($$render) => {
		if ($$props.d.branch) $$render(consequent_1);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_2 = ($$anchor) => {
		var fragment_2 = root_2$15();
		var dd_2 = sibling(first_child(fragment_2));
		var text_3 = only_child(dd_2, true);
		template_effect(() => set_text(text_3, $$props.d.spec));
		append($$anchor, fragment_2);
	};
	if_block(node_5, ($$render) => {
		if ($$props.d.spec) $$render(consequent_2);
	});
	var node_6 = sibling(node_5, 2);
	var consequent_3 = ($$anchor) => {
		var fragment_3 = root_3$14();
		var dd_3 = sibling(first_child(fragment_3));
		var text_4 = only_child(dd_3, true);
		template_effect(($0) => set_text(text_4, $0), [() => shape($$props.d.shape_says)]);
		append($$anchor, fragment_3);
	};
	if_block(node_6, ($$render) => {
		if ($$props.d.shape_says) $$render(consequent_3);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_4 = ($$anchor) => {
		var fragment_4 = root_4$11();
		var dd_4 = sibling(first_child(fragment_4));
		var text_5 = only_child(dd_4, true);
		template_effect(() => set_text(text_5, $$props.d.worktree));
		append($$anchor, fragment_4);
	};
	if_block(node_7, ($$render) => {
		if ($$props.d.worktree) $$render(consequent_4);
	});
	reset(dl);
	reset(div_1);
	reset(details);
	var node_8 = sibling(details, 2);
	var consequent_5 = ($$anchor) => {
		var span = root_5$11();
		var node_9 = child(span);
		Pill(node_9, {
			get word() {
				return $$props.d.waiting_says;
			},
			as: "wait"
		});
		reset(span);
		append($$anchor, span);
	};
	if_block(node_8, ($$render) => {
		if ($$props.d.waiting_says) $$render(consequent_5);
	});
	var node_10 = sibling(node_8, 2);
	var consequent_6 = ($$anchor) => {
		var span_1 = root_6$9();
		var node_11 = child(span_1);
		Icon(node_11, {
			name: "alert",
			size: 12
		});
		next();
		reset(span_1);
		append($$anchor, span_1);
	};
	if_block(node_10, ($$render) => {
		if ($$props.d.in_place) $$render(consequent_6);
	});
	var node_12 = sibling(node_10, 2);
	var consequent_7 = ($$anchor) => {
		var button_1 = root_7$9();
		var node_13 = child(button_1);
		Icon(node_13, {
			name: "change",
			size: 12
		});
		var text_6 = sibling(node_13);
		reset(button_1);
		template_effect(() => set_text(text_6, ` ${$$props.d.branch ?? ""}`));
		delegated("click", button_1, () => $$props.copy($$props.d.branch, "the branch name"));
		append($$anchor, button_1);
	};
	if_block(node_12, ($$render) => {
		if ($$props.d.branch) $$render(consequent_7);
	});
	var div_2 = sibling(node_12, 4);
	let classes;
	var node_14 = child(div_2);
	var consequent_8 = ($$anchor) => {
		var button_2 = root_8$8();
		let classes_1;
		var node_15 = child(button_2);
		Icon(node_15, {
			name: "split",
			size: 14
		});
		next();
		reset(button_2);
		template_effect(() => classes_1 = set_class(button_2, 1, "svelte-axcc9r", null, classes_1, { primary: !get(noCheck) && ($$props.unseen > 0 || !$$props.verified) }));
		delegated("click", button_2, function(...$$args) {
			$$props.review?.apply(this, $$args);
		});
		append($$anchor, button_2);
	};
	if_block(node_14, ($$render) => {
		if (!get(reviewing)) $$render(consequent_8);
	});
	var node_16 = sibling(node_14, 2);
	var consequent_9 = ($$anchor) => {
		NoCheck($$anchor, {
			get project() {
				return $$props.d.project_id;
			},
			compact: true
		});
	};
	var consequent_10 = ($$anchor) => {
		var button_3 = root_9$7();
		var node_17 = child(button_3);
		Icon(node_17, {
			name: "gate",
			size: 14
		});
		next(2);
		reset(button_3);
		template_effect(() => button_3.disabled = !!$$props.busy);
		delegated("click", button_3, () => $$props.act("verify"));
		append($$anchor, button_3);
	};
	if_block(node_16, ($$render) => {
		if (get(noCheck)) $$render(consequent_9);
		else if ($$props.d.worktree) $$render(consequent_10, 1);
	});
	var node_18 = sibling(node_16, 2);
	var consequent_13 = ($$anchor) => {
		var fragment_6 = comment();
		var node_19 = first_child(fragment_6);
		var consequent_11 = ($$anchor) => {
			var fragment_7 = root_10$6();
			var button_4 = first_child(fragment_7);
			var node_20 = child(button_4);
			Icon(node_20, {
				name: "forge",
				size: 14
			});
			next();
			reset(button_4);
			var span_2 = sibling(button_4, 2);
			var text_7 = only_child(span_2, true);
			template_effect(() => {
				set_attribute(button_4, "title", $$props.offerHeld);
				set_text(text_7, $$props.offerHeld);
			});
			append($$anchor, fragment_7);
		};
		var alternate = ($$anchor) => {
			var fragment_8 = root_13$6();
			var button_5 = first_child(fragment_8);
			let classes_2;
			var node_21 = child(button_5);
			Icon(node_21, {
				name: "forge",
				size: 14
			});
			next(3);
			reset(button_5);
			var node_22 = sibling(button_5, 2);
			var consequent_12 = ($$anchor) => {
				var details_1 = root_12$6();
				var summary_1 = child(details_1);
				var text_8 = only_child(summary_1, true);
				var ul = sibling(summary_1, 2);
				each(ul, 20, notOpened, (p) => p, ($$anchor, p) => {
					var li = root_11$6();
					var text_9 = only_child(li, true);
					template_effect(() => set_text(text_9, p));
					append($$anchor, li);
				});
				reset(ul);
				reset(details_1);
				template_effect(() => set_text(text_8, notOpened().length === 1 ? "1 file not opened" : `${notOpened().length} files not opened`));
				append($$anchor, details_1);
			};
			if_block(node_22, ($$render) => {
				if (notOpened().length) $$render(consequent_12);
			});
			template_effect(() => {
				button_5.disabled = !!$$props.busy;
				classes_2 = set_class(button_5, 1, "svelte-axcc9r", null, classes_2, { primary: $$props.unseen === 0 });
			});
			delegated("click", button_5, () => $$props.act("offer"));
			append($$anchor, fragment_8);
		};
		if_block(node_19, ($$render) => {
			if ($$props.offerHeld) $$render(consequent_11);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_6);
	};
	if_block(node_18, ($$render) => {
		if ($$props.verified) $$render(consequent_13);
	});
	var node_23 = sibling(node_18, 2);
	var consequent_15 = ($$anchor) => {
		var fragment_9 = root_15$2();
		var button_6 = first_child(fragment_9);
		var node_24 = child(button_6);
		Icon(node_24, {
			name: "play",
			size: 14
		});
		next();
		reset(button_6);
		var node_25 = sibling(button_6, 2);
		var consequent_14 = ($$anchor) => {
			var button_7 = root_14$5();
			var node_26 = child(button_7);
			Icon(node_26, {
				name: "refresh",
				size: 14
			});
			next();
			reset(button_7);
			template_effect(() => button_7.disabled = !!$$props.busy);
			delegated("click", button_7, () => $$props.act("retry"));
			append($$anchor, button_7);
		};
		if_block(node_25, ($$render) => {
			if ($$props.d.can_retry) $$render(consequent_14);
		});
		template_effect(() => button_6.disabled = !!$$props.busy);
		delegated("click", button_6, () => $$props.act("resume"));
		append($$anchor, fragment_9);
	};
	if_block(node_23, ($$render) => {
		if ($$props.d.stopped) $$render(consequent_15);
	});
	var node_27 = sibling(node_23, 2);
	var consequent_19 = ($$anchor) => {
		var details_2 = root_19$1();
		var summary_2 = child(details_2);
		var node_28 = child(summary_2);
		Icon(node_28, {
			name: "more",
			size: 14
		});
		next(2);
		reset(summary_2);
		var div_3 = sibling(summary_2, 2);
		var node_29 = child(div_3);
		var consequent_16 = ($$anchor) => {
			var button_8 = root_16$2();
			var node_30 = child(button_8);
			Icon(node_30, {
				name: "check",
				size: 14
			});
			next();
			reset(button_8);
			template_effect(() => button_8.disabled = !!$$props.busy);
			delegated("click", button_8, () => (shut(), $$props.act("finish")));
			append($$anchor, button_8);
		};
		if_block(node_29, ($$render) => {
			if (!$$props.d.completion && !$$props.d.archived_at && $$props.d.runs?.length) $$render(consequent_16);
		});
		var node_31 = sibling(node_29, 2);
		var consequent_17 = ($$anchor) => {
			var fragment_10 = root_17$2();
			var button_9 = first_child(fragment_10);
			var node_32 = child(button_9);
			Icon(node_32, {
				name: "external",
				size: 14
			});
			next();
			reset(button_9);
			var button_10 = sibling(button_9, 2);
			var node_33 = child(button_10);
			Icon(node_33, {
				name: "terminal",
				size: 14
			});
			next();
			reset(button_10);
			delegated("click", button_9, () => (shut(), $$props.openIn("editor")));
			delegated("click", button_10, () => (shut(), $$props.openIn("terminal")));
			append($$anchor, fragment_10);
		};
		if_block(node_31, ($$render) => {
			if ($$props.d.worktree) $$render(consequent_17);
		});
		var node_34 = sibling(node_31, 2);
		var consequent_18 = ($$anchor) => {
			var button_11 = root_18$2();
			var node_35 = child(button_11);
			Icon(node_35, {
				name: "folder",
				size: 14
			});
			next();
			reset(button_11);
			template_effect(() => button_11.disabled = !!$$props.busy);
			delegated("click", button_11, () => (shut(), $$props.act("archive")));
			append($$anchor, button_11);
		};
		if_block(node_34, ($$render) => {
			if (!$$props.d.archived_at && ($$props.d.completion || $$props.d.stopped)) $$render(consequent_18);
		});
		reset(div_3);
		reset(details_2);
		bind_this(details_2, ($$value) => set(menu, $$value), () => get(menu));
		append($$anchor, details_2);
	};
	if_block(node_27, ($$render) => {
		if (get(more)) $$render(consequent_19);
	});
	reset(div_2);
	reset(div);
	var node_36 = sibling(div, 2);
	var consequent_20 = ($$anchor) => {
		Claim($$anchor, {
			get claim() {
				return $$props.d.stop_claim;
			},
			get change() {
				return $$props.d.id;
			},
			get done() {
				return reload();
			}
		});
	};
	if_block(node_36, ($$render) => {
		if (!get(reviewing) && $$props.d.stop_claim) $$render(consequent_20);
	});
	var node_37 = sibling(node_36, 2);
	var consequent_25 = ($$anchor) => {
		var p_1 = root_23$1();
		var node_38 = child(p_1);
		var consequent_21 = ($$anchor) => {
			var span_3 = root_20$1();
			var node_39 = child(span_3);
			Inline(node_39, { get text() {
				return $$props.standing;
			} });
			reset(span_3);
			template_effect(() => set_class(span_3, 1, `standing ${$$props.verified && !$$props.leads ? "done" : ""}`, "svelte-axcc9r"));
			append($$anchor, span_3);
		};
		if_block(node_38, ($$render) => {
			if ($$props.standing) $$render(consequent_21);
		});
		var node_40 = sibling(node_38, 2);
		var consequent_24 = ($$anchor) => {
			var span_4 = root_22$1();
			var text_10 = child(span_4, true);
			var node_41 = sibling(text_10);
			var consequent_22 = ($$anchor) => {
				var span_5 = root_21$1();
				var text_11 = only_child(span_5);
				template_effect(() => {
					set_class(span_5, 1, `w-${$$props.d.base_checks.headline.weight ?? ""}`, "svelte-axcc9r");
					set_text(text_11, `· ${$$props.d.base_checks.headline.says ?? ""}`);
				});
				append($$anchor, span_5);
			};
			var consequent_23 = ($$anchor) => {
				var span_6 = root_21$1();
				var text_12 = only_child(span_6);
				template_effect(() => {
					set_class(span_6, 1, `w-${$$props.d.base_checks.on_base.weight ?? ""}`, "svelte-axcc9r");
					set_text(text_12, `· ${$$props.d.base_checks.on_base.says ?? ""}`);
				});
				append($$anchor, span_6);
			};
			if_block(node_41, ($$render) => {
				if ($$props.d.base_checks.headline) $$render(consequent_22);
				else if ($$props.d.base_checks.on_base && !$$props.d.base_checks.stale) $$render(consequent_23, 1);
			});
			reset(span_4);
			template_effect(() => set_text(text_10, $$props.d.base_checks.counts_says));
			append($$anchor, span_4);
		};
		if_block(node_40, ($$render) => {
			if ($$props.d.base_checks) $$render(consequent_24);
		});
		reset(p_1);
		append($$anchor, p_1);
	};
	if_block(node_37, ($$render) => {
		if (!get(reviewing) && ($$props.standing || $$props.d.base_checks)) $$render(consequent_25);
	});
	reset(header);
	template_effect(() => {
		set_attribute(h1, "title", $$props.d.title || $$props.d.id);
		set_text(text, $$props.d.title || $$props.d.id);
		classes = set_class(div_2, 1, "acts svelte-axcc9r", null, classes, { "held-offer": !!$$props.offerHeld });
	});
	append($$anchor, header);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/ui/Timeline.svelte
var root$15 = from_html(`<p class="none svelte-hwm3dn">Nothing has happened to it yet.</p>`);
var root_1$14 = from_html(`<button></button>`);
var root_2$14 = from_html(`<div class="lane svelte-hwm3dn"><span class="name svelte-hwm3dn"> </span> <div class="track svelte-hwm3dn"><!> <span class="now svelte-hwm3dn" aria-hidden="true"></span></div></div>`);
var root_3$13 = from_html(`<figure class="tl svelte-hwm3dn"><!> <div class="axis svelte-hwm3dn"><span></span><span class="ticks svelte-hwm3dn"><span> </span><span> </span></span></div> <figcaption aria-live="polite" class="svelte-hwm3dn"> </figcaption></figure>`);
function Timeline($$anchor, $$props) {
	push($$props, true);
	const times = user_derived(() => $$props.marks.map((m) => new Date(m.at).getTime()).filter((t) => !Number.isNaN(t)));
	const lo = user_derived(() => get(times).length ? Math.min(...get(times)) : 0);
	const hi = user_derived(() => get(times).length ? Math.max(Date.now(), ...get(times)) : 1);
	const span = user_derived(() => Math.max(1, get(hi) - get(lo)));
	const pct = (at) => (new Date(at).getTime() - get(lo)) / get(span) * 100;
	function clock(t) {
		const d = new Date(t);
		const sameDay = (/* @__PURE__ */ new Date()).toDateString() === d.toDateString();
		const date = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
		return sameDay ? d.toTimeString().slice(0, 5) : `${date} ${d.toTimeString().slice(0, 5)}`;
	}
	let hover = state(null);
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var p = root$15();
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var figure = root_3$13();
		var node_1 = child(figure);
		each(node_1, 16, () => $$props.lanes, (lane) => lane, ($$anchor, lane) => {
			var div = root_2$14();
			var span_1 = child(div);
			var text = only_child(span_1, true);
			var div_1 = sibling(span_1, 2);
			var node_2 = child(div_1);
			each(node_2, 17, () => $$props.marks.filter((m) => m.lane === lane), index, ($$anchor, m) => {
				var button = root_1$14();
				let styles;
				template_effect(($0, $1, $2) => {
					set_class(button, 1, `mark ${get(m).tone ?? "none" ?? ""}`, "svelte-hwm3dn");
					set_attribute(button, "title", `${get(m).label ?? ""} — ${$0 ?? ""}`);
					set_attribute(button, "aria-label", `${get(m).label ?? ""} at ${$1 ?? ""}`);
					styles = set_style(button, "", styles, { left: $2 });
				}, [
					() => new Date(get(m).at).toLocaleString(),
					() => new Date(get(m).at).toLocaleString(),
					() => `${pct(get(m).at) ?? ""}%`
				]);
				event("mouseenter", button, () => set(hover, get(m), true));
				event("mouseleave", button, () => set(hover, null));
				event("focus", button, () => set(hover, get(m), true));
				event("blur", button, () => set(hover, null));
				append($$anchor, button);
			});
			next(2);
			reset(div_1);
			reset(div);
			template_effect(() => set_text(text, lane));
			append($$anchor, div);
		});
		var div_2 = sibling(node_1, 2);
		var span_2 = sibling(child(div_2));
		var span_3 = child(span_2);
		var text_1 = only_child(span_3, true);
		var span_4 = sibling(span_3);
		var text_2 = only_child(span_4);
		reset(span_2);
		reset(div_2);
		var figcaption = sibling(div_2, 2);
		var text_3 = only_child(figcaption, true);
		reset(figure);
		template_effect(($0, $1) => {
			set_attribute(figure, "aria-label", `timeline of ${$$props.marks.length ?? ""} events`);
			set_text(text_1, $0);
			set_text(text_2, `now · ${$1 ?? ""}`);
			set_text(text_3, get(hover) ? `${get(hover).lane} — ${get(hover).label}` : " ");
		}, [() => clock(get(lo)), () => clock(get(hi))]);
		append($$anchor, figure);
	};
	if_block(node, ($$render) => {
		if ($$props.marks.length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
//#region src/surfaces/change/types.ts
function countSays(t) {
	if (!t || t.count === "no_runner") return "no test runner's summary in this output";
	if (t.count === "no_exit") return "did not finish, so no count was read";
	if (t.count === "not_recognised") return t.says;
	return t.tallies.map((x) => [`${x.runner} ran ${x.ran}`, ...x.counts.filter(([n, c]) => c > 0 && ![
		"run",
		"total",
		"tests"
	].includes(n)).map(([n, c]) => `${n} ${c}`)].join(" · ")).join(" · ");
}
function taskLine(d) {
	const c = d.counts;
	if (!c) return "";
	const sent = new Set((d.run_rows ?? []).flatMap((r) => (r.sent ?? []).map((t) => `${t.path}:${t.text}`))).size;
	const seen = c.seen_by_pass === null ? "no gates declared" : typeof c.seen_by_pass === "number" ? `${c.seen_by_pass} seen by a passing check` : "";
	return [
		`${c.tasks} ${c.tasks === 1 ? "task" : "tasks"}`,
		`${sent} sent`,
		`${c.ticked} ticked`,
		seen
	].filter(Boolean).join(" · ");
}
function exit(o) {
	if (o.outcome === "exited") return `exit ${o.code}`;
	if (o.outcome === "timed_out") return `timed out after ${o.after_secs}s`;
	return `${o.outcome.replace(/_/g, " ")}${o.reason ? `: ${o.reason}` : ""}`;
}
function took(ms) {
	if (ms < 1e3) return `${ms} ms`;
	if (ms < 6e4) return `${(ms / 1e3).toFixed(1)} s`;
	return `${Math.floor(ms / 6e4)} m ${Math.round(ms % 6e4 / 1e3)} s`;
}
function ago(at) {
	if (!at) return "—";
	const t = new Date(at).getTime();
	if (Number.isNaN(t)) return "—";
	const s = Math.max(0, Math.round((Date.now() - t) / 1e3));
	if (s < 60) return `${s}s ago`;
	if (s < 3600) return `${Math.round(s / 60)}m ago`;
	if (s < 86400) return `${Math.round(s / 3600)}h ago`;
	return `${Math.round(s / 86400)}d ago`;
}
//#endregion
//#region src/surfaces/change/Overview.svelte
var root$14 = from_html(`<span class="v quiet svelte-1og7e16">No gate has run against this change.</span>`);
var root_1$13 = from_html(`<span> </span>`);
var root_2$13 = from_html(`<code class="cmd svelte-1og7e16"> </code>`);
var root_3$12 = from_html(`<span class="v fail svelte-1og7e16"> </span> <!>`, 1);
var root_4$10 = from_html(`<span class="s wait svelte-1og7e16"> </span>`);
var root_5$10 = from_html(`<span class="v svelte-1og7e16"> </span> <!>`, 1);
var root_6$8 = from_html(`<span class="v quiet svelte-1og7e16"><!></span>`);
var root_7$8 = from_html(`<span class="s wait svelte-1og7e16"><!></span>`);
var root_8$7 = from_html(`<span class="v quiet svelte-1og7e16">Not read.</span>`);
var root_9$6 = from_html(`<span class="v fail svelte-1og7e16">Not read: <!></span> <span class="s svelte-1og7e16"><code> </code> tells more</span>`, 1);
var root_10$5 = from_html(`<span class="v quiet svelte-1og7e16" aria-busy="true">…</span>`);
var root_11$5 = from_html(`<span class="v quiet svelte-1og7e16">Nothing has been decided about it.</span>`);
var root_12$5 = from_html(`<span class="v svelte-1og7e16"><b></b> read — there are more</span> <span class="s svelte-1og7e16"><code> </code> has every one</span>`, 1);
var root_13$5 = from_html(`<span class="v svelte-1og7e16"><b> </b> recorded</span> <span class="s svelte-1og7e16"> </span>`, 1);
var root_14$4 = from_html(`<span> <!></span>`);
var root_15$1 = from_html(`<span class="s fail svelte-1og7e16"> </span>`);
var root_16$1 = from_html(`<p class="svelte-1og7e16"><!></p>`);
var root_17$1 = from_html(`<section class="alert svelte-1og7e16"><!> <div><b>The specification moved under a run.</b> <!> <button class="svelte-1og7e16">Decide in Tasks</button></div></section>`);
var root_18$1 = from_html(`<div class="grid svelte-1og7e16"><section class="cards svelte-1og7e16"><button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Gates</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Tasks</span> <!> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Decisions</span> <!> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Agent</span> <span class="v svelte-1og7e16"><b> </b> <!></span> <!></button></section> <!> <section class="facts svelte-1og7e16"><h2 class="svelte-1og7e16">Facts</h2> <!></section> <section class="history svelte-1og7e16"><h2 class="svelte-1og7e16">History</h2> <!> <!></section></div>`);
function Overview($$anchor, $$props) {
	push($$props, true);
	const LIMIT = 200;
	const about = user_derived(() => $$props.d.id);
	const read = resource(() => `/api/decisions?about=${encodeURIComponent(get(about))}&limit=${LIMIT}`, { tell: () => `devplane audit ${get(about)}` });
	const decisions = user_derived(() => Array.isArray(read.data) ? read.data : []);
	const failing = user_derived(() => $$props.d.gate && !$$props.d.gate.passed ? ($$props.d.gate.commands ?? []).filter((c) => !(c.outcome.outcome === "exited" && c.outcome.code === 0)) : []);
	const short = (s) => s ? s.slice(0, 10) : null;
	const tone = (a) => a === "person" ? "work" : a === "rule" ? "wait" : a === "timer" || a === "nobody" ? "fail" : "none";
	const marks = user_derived(() => [
		...$$props.d.created_at ? [{
			at: $$props.d.created_at,
			lane: "change",
			label: "started",
			tone: "work"
		}] : [],
		...($$props.d.gates ?? []).map((g) => {
			const ok = g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);
			return {
				at: g.at,
				lane: "gates",
				label: `${g.gate} attempt ${g.attempt}: ${ok ? "passed" : "failed"}`,
				tone: ok ? "done" : "fail"
			};
		}),
		...get(decisions).map((x) => ({
			at: x.at,
			lane: "decisions",
			label: `${x.authority} · ${x.action} · ${x.outcome}`,
			tone: tone(x.authority)
		})),
		...($$props.d.claim_history ?? []).map((c) => ({
			at: c.at,
			lane: "agent",
			label: `${c.says} — run ${c.run}${c.closed_says ? ` · closed: ${c.closed_says}` : ""}`,
			tone: "wait"
		})),
		...$$props.d.archived_at ? [{
			at: $$props.d.archived_at,
			lane: "change",
			label: STATES.archived.word,
			tone: "none"
		}] : []
	]);
	const byAuthority = user_derived(() => Object.entries(get(decisions).reduce((m, x) => (m[x.authority] = (m[x.authority] ?? 0) + 1, m), {})));
	var div = root_18$1();
	var section = child(div);
	var button = child(section);
	var span = child(button);
	var node = child(span);
	Icon(node, {
		name: "gate",
		size: 14
	});
	next();
	reset(span);
	var node_1 = sibling(span, 2);
	var consequent = ($$anchor) => {
		var span_1 = root$14();
		append($$anchor, span_1);
	};
	var consequent_1 = ($$anchor) => {
		var span_2 = root_1$13();
		let classes;
		var text = only_child(span_2);
		template_effect(($0) => {
			classes = set_class(span_2, 1, "v svelte-1og7e16", null, classes, { done: $0 });
			set_text(text, `${$$props.d.gate.name ?? ""} exited zero · attempt ${$$props.d.gate.attempt ?? ""}`);
		}, [() => $$props.d.state === STATES.verified.word && !weakLeads($$props.d.qualifier)]);
		append($$anchor, span_2);
	};
	var alternate = ($$anchor) => {
		var fragment = root_3$12();
		var span_3 = first_child(fragment);
		var text_1 = only_child(span_3);
		var node_2 = sibling(span_3, 2);
		each(node_2, 17, () => get(failing).slice(0, 2), (c) => c.command, ($$anchor, c) => {
			var code = root_2$13();
			var text_2 = only_child(code);
			template_effect(($0) => set_text(text_2, `${get(c).command ?? ""} — ${$0 ?? ""}`), [() => exit(get(c).outcome)]);
			append($$anchor, code);
		});
		template_effect(() => set_text(text_1, `${$$props.d.gate.name ?? ""} failed · ${get(failing).length ?? ""} of ${$$props.d.gate.commands?.length ?? 0 ?? ""} commands`));
		append($$anchor, fragment);
	};
	if_block(node_1, ($$render) => {
		if (!$$props.d.gate) $$render(consequent);
		else if ($$props.d.gate.passed) $$render(consequent_1, 1);
		else $$render(alternate, -1);
	});
	reset(button);
	var button_1 = sibling(button, 2);
	var span_4 = child(button_1);
	var node_3 = child(span_4);
	Icon(node_3, {
		name: "spec",
		size: 14
	});
	next();
	reset(span_4);
	var node_4 = sibling(span_4, 2);
	var consequent_3 = ($$anchor) => {
		var fragment_1 = root_5$10();
		var span_5 = first_child(fragment_1);
		var text_3 = only_child(span_5, true);
		var node_5 = sibling(span_5, 2);
		var consequent_2 = ($$anchor) => {
			var span_6 = root_4$10();
			var text_4 = only_child(span_6);
			template_effect(() => set_text(text_4, `${$$props.d.counts.ticked_unsent.length ?? ""} ticked that no run was sent`));
			append($$anchor, span_6);
		};
		if_block(node_5, ($$render) => {
			if ($$props.d.counts.ticked_unsent?.length) $$render(consequent_2);
		});
		template_effect(($0) => set_text(text_3, $0), [() => taskLine($$props.d)]);
		append($$anchor, fragment_1);
	};
	var alternate_2 = ($$anchor) => {
		var span_7 = root_6$8();
		var node_6 = child(span_7);
		var consequent_4 = ($$anchor) => {
			Inline($$anchor, { get text() {
				return $$props.d.plan.no_progress;
			} });
		};
		var alternate_1 = ($$anchor) => {
			var text_5 = text();
			template_effect(() => set_text(text_5, $$props.d.spec ? "The specification has no tasks." : "No specification is attached."));
			append($$anchor, text_5);
		};
		if_block(node_6, ($$render) => {
			if ($$props.d.plan?.no_progress) $$render(consequent_4);
			else $$render(alternate_1, -1);
		});
		reset(span_7);
		append($$anchor, span_7);
	};
	if_block(node_4, ($$render) => {
		if ($$props.d.counts) $$render(consequent_3);
		else $$render(alternate_2, -1);
	});
	var node_7 = sibling(node_4, 2);
	var consequent_5 = ($$anchor) => {
		var span_8 = root_7$8();
		var node_8 = child(span_8);
		Inline(node_8, { get text() {
			return $$props.d.planning_absent;
		} });
		reset(span_8);
		append($$anchor, span_8);
	};
	if_block(node_7, ($$render) => {
		if ($$props.d.planning_absent) $$render(consequent_5);
	});
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	var span_9 = child(button_2);
	var node_9 = child(span_9);
	Icon(node_9, {
		name: "ledger",
		size: 14
	});
	next();
	reset(span_9);
	var node_10 = sibling(span_9, 2);
	var consequent_7 = ($$anchor) => {
		var fragment_4 = comment();
		var node_11 = first_child(fragment_4);
		var consequent_6 = ($$anchor) => {
			var span_10 = root_8$7();
			append($$anchor, span_10);
		};
		var alternate_3 = ($$anchor) => {
			var fragment_5 = root_9$6();
			var span_11 = first_child(fragment_5);
			var node_12 = sibling(child(span_11));
			Inline(node_12, { get text() {
				return read.failure.says;
			} });
			reset(span_11);
			var span_12 = sibling(span_11, 2);
			var code_1 = child(span_12);
			var text_6 = only_child(code_1, true);
			next();
			reset(span_12);
			template_effect(() => set_text(text_6, read.failure.tell));
			append($$anchor, fragment_5);
		};
		if_block(node_11, ($$render) => {
			if (read.failure.down) $$render(consequent_6);
			else $$render(alternate_3, -1);
		});
		append($$anchor, fragment_4);
	};
	var consequent_8 = ($$anchor) => {
		var span_13 = root_10$5();
		append($$anchor, span_13);
	};
	var consequent_9 = ($$anchor) => {
		var span_14 = root_11$5();
		append($$anchor, span_14);
	};
	var consequent_10 = ($$anchor) => {
		var fragment_6 = root_12$5();
		var span_15 = first_child(fragment_6);
		var b = child(span_15);
		b.textContent = "200";
		next();
		reset(span_15);
		var span_16 = sibling(span_15, 2);
		var code_2 = child(span_16);
		var text_7 = only_child(code_2);
		next();
		reset(span_16);
		template_effect(() => set_text(text_7, `devplane audit ${get(about) ?? ""}`));
		append($$anchor, fragment_6);
	};
	var alternate_4 = ($$anchor) => {
		var fragment_7 = root_13$5();
		var span_17 = first_child(fragment_7);
		var b_1 = child(span_17);
		var text_8 = only_child(b_1, true);
		next();
		reset(span_17);
		var span_18 = sibling(span_17, 2);
		var text_9 = only_child(span_18, true);
		template_effect(($0) => {
			set_text(text_8, get(decisions).length);
			set_text(text_9, $0);
		}, [() => get(byAuthority).map(([a, n]) => `${n} by ${a}`).join(" · ")]);
		append($$anchor, fragment_7);
	};
	if_block(node_10, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_7);
		else if (read.phase === "loading") $$render(consequent_8, 1);
		else if (get(decisions).length === 0) $$render(consequent_9, 2);
		else if (get(decisions).length >= LIMIT) $$render(consequent_10, 3);
		else $$render(alternate_4, -1);
	});
	var node_13 = sibling(node_10, 2);
	var consequent_12 = ($$anchor) => {
		var span_19 = root_14$4();
		let classes_1;
		var text_10 = child(span_19, true);
		var node_14 = sibling(text_10);
		var consequent_11 = ($$anchor) => {
			var text_11 = text();
			template_effect(($0) => set_text(text_11, `· ${$0 ?? ""}`), [() => ago($$props.d.mode_statement.at)]);
			append($$anchor, text_11);
		};
		if_block(node_14, ($$render) => {
			if ($$props.d.mode_statement.at) $$render(consequent_11);
		});
		reset(span_19);
		template_effect(() => {
			classes_1 = set_class(span_19, 1, "s svelte-1og7e16", null, classes_1, { fail: $$props.d.mode_statement.reading === "decides_without_you" });
			set_attribute(span_19, "title", $$props.d.mode_statement.mode ?? void 0);
			set_text(text_10, $$props.d.mode_statement.sentence);
		});
		append($$anchor, span_19);
	};
	if_block(node_13, ($$render) => {
		if ($$props.d.mode_statement) $$render(consequent_12);
	});
	reset(button_2);
	var button_3 = sibling(button_2, 2);
	var span_20 = child(button_3);
	var node_15 = child(span_20);
	Icon(node_15, {
		name: "agent",
		size: 14
	});
	next();
	reset(span_20);
	var span_21 = sibling(span_20, 2);
	var b_2 = child(span_21);
	var text_12 = only_child(b_2, true);
	var text_13 = sibling(b_2);
	var node_16 = sibling(text_13);
	var consequent_13 = ($$anchor) => {
		var text_14 = text();
		template_effect(() => set_text(text_14, `· ${$$props.d.feedback_rounds ?? ""} handed back`));
		append($$anchor, text_14);
	};
	if_block(node_16, ($$render) => {
		if ($$props.d.feedback_rounds) $$render(consequent_13);
	});
	reset(span_21);
	var node_17 = sibling(span_21, 2);
	var consequent_14 = ($$anchor) => {
		var span_22 = root_15$1();
		var text_15 = only_child(span_22, true);
		template_effect(() => set_text(text_15, $$props.d.stopped_summary));
		append($$anchor, span_22);
	};
	if_block(node_17, ($$render) => {
		if ($$props.d.stopped_summary) $$render(consequent_14);
	});
	reset(button_3);
	reset(section);
	var node_18 = sibling(section, 2);
	var consequent_15 = ($$anchor) => {
		var section_1 = root_17$1();
		var node_19 = child(section_1);
		Icon(node_19, {
			name: "alert",
			size: 16
		});
		var div_1 = sibling(node_19, 2);
		var node_20 = sibling(child(div_1), 2);
		each(node_20, 17, () => $$props.d.drifts ?? [], index, ($$anchor, x) => {
			var p = root_16$1();
			var node_21 = child(p);
			Inline(node_21, { get text() {
				return get(x).says;
			} });
			reset(p);
			append($$anchor, p);
		});
		var button_4 = sibling(node_20, 2);
		reset(div_1);
		reset(section_1);
		delegated("click", button_4, () => $$props.go("tasks"));
		append($$anchor, section_1);
	};
	if_block(node_18, ($$render) => {
		if (($$props.d.drifts ?? []).length > 0) $$render(consequent_15);
	});
	var section_2 = sibling(node_18, 2);
	var node_22 = sibling(child(section_2), 2);
	{
		let $0 = user_derived(() => [
			{
				label: "Branch",
				value: $$props.d.branch,
				mono: true,
				missing: "none — works in place"
			},
			{
				label: "Worktree",
				value: $$props.d.worktree,
				mono: true,
				missing: "none"
			},
			{
				label: "Specification",
				value: $$props.d.spec,
				mono: true,
				missing: "none attached"
			},
			{
				label: "Tree now",
				value: $$props.d.tree_now ? `${short($$props.d.tree_now.tree) ?? "?"} · ${$$props.d.tree_now.clean ? "clean" : `${$$props.d.tree_now.changed_files ?? 0} uncommitted`}` : null,
				mono: true,
				missing: "unknown"
			},
			{
				label: "Commit",
				value: short($$props.d.tree_now?.commit),
				mono: true,
				missing: "no commits yet"
			},
			{
				label: "Pushed",
				value: $$props.d.tree_now?.reach?.replace(/_/g, " ") ?? null
			},
			{
				label: "Pull request",
				value: $$props.d.pull_request?.url ?? null,
				missing: "not offered"
			},
			{
				label: "Cost",
				value: $$props.d.cost_usd ? `$${$$props.d.cost_usd.toFixed(2)}` : null,
				missing: "not reported"
			},
			{
				label: "Started",
				value: ago($$props.d.created_at)
			},
			{
				label: "Last moved",
				value: ago($$props.d.updated_at)
			}
		]);
		Props(node_22, { get rows() {
			return get($0);
		} });
	}
	reset(section_2);
	var section_3 = sibling(section_2, 2);
	var node_23 = sibling(child(section_3), 2);
	var consequent_16 = ($$anchor) => {
		{
			let $0 = user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the decisions",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	if_block(node_23, ($$render) => {
		if (read.failure && !(read.failure.down && read.data === null)) $$render(consequent_16);
	});
	var node_24 = sibling(node_23, 2);
	{
		let $0 = user_derived(() => ($$props.d.claim_history ?? []).length ? [
			"change",
			"agent",
			"gates",
			"decisions"
		] : [
			"change",
			"gates",
			"decisions"
		]);
		Timeline(node_24, {
			get marks() {
				return get(marks);
			},
			get lanes() {
				return get($0);
			}
		});
	}
	reset(section_3);
	reset(div);
	template_effect(() => {
		set_text(text_12, $$props.d.runs?.length ?? 0);
		set_text(text_13, ` ${$$props.d.runs?.length === 1 ? "run" : "runs"}`);
	});
	delegated("click", button, () => $$props.go("gates"));
	delegated("click", button_1, () => $$props.go("tasks"));
	delegated("click", button_2, () => $$props.go("ledger"));
	delegated("click", button_3, () => $$props.go("agent"));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/weakKind.ts
var weakKindSays = {
	skip: "check weakened",
	deleted: "test deleted",
	gate: "gate changed",
	agent_controls: "agent controls",
	ownership: "ownership",
	supply_chain: "supply chain",
	base_tests: "base tests"
};
var weakKindLabel = (k) => {
	const s = weakKindSays[k] ?? "";
	return s.charAt(0).toUpperCase() + s.slice(1);
};
//#endregion
//#region src/surfaces/change/Gates.svelte
var root$13 = from_html(`; <!>`, 1);
var root_1$12 = from_html(`<p class="svelte-wqjyvl"><!> <b> </b> in <button class="line svelte-wqjyvl"><code> </code></button> — <!><!> </p>`);
var root_2$12 = from_html(`<section class="weakened svelte-wqjyvl" aria-label="checks this change weakened"></section>`);
var root_3$11 = from_html(`<p class="quiet svelte-wqjyvl">No gate has run against this change yet. The gates run when the agent says it has finished, or when you press <b>Run gates</b>.</p>`);
var root_4$9 = from_html(`<code class="tree svelte-wqjyvl" title="the tree this ran against"> </code>`);
var root_5$9 = from_html(`<span class="tests svelte-wqjyvl"> </span>`);
var root_6$7 = from_html(`<li class="svelte-wqjyvl"> </li>`);
var root_7$7 = from_html(`<ul class="fails svelte-wqjyvl"></ul>`);
var root_8$6 = from_html(`<pre class="out svelte-wqjyvl"> </pre> <span class="bytes svelte-wqjyvl"> </span>`, 1);
var root_9$5 = from_html(`<li><div class="row svelte-wqjyvl"><!> <code class="cmd svelte-wqjyvl"> </code> <span class="exit svelte-wqjyvl"> </span> <!> <span class="dur svelte-wqjyvl"> </span></div> <!> <!></li>`);
var root_10$4 = from_html(`<ol class="cmds svelte-wqjyvl"></ol>`);
var root_11$4 = from_html(`<section><button class="head svelte-wqjyvl"><!> <!> <b> </b> <span> </span> <span class="verdict svelte-wqjyvl"> </span> <span class="meta svelte-wqjyvl"> </span> <!></button> <!></section>`);
var root_12$4 = from_html(`<button class="svelte-wqjyvl"><!> Run base checks</button>`);
var root_13$4 = from_html(`<li> </li>`);
var root_14$3 = from_html(`<p class="quiet svelte-wqjyvl"> <code> </code> lists every one</p>`);
var root_15 = from_html(`<ul class="names svelte-wqjyvl" aria-label="new tests that fail on the base and pass here"></ul> <!>`, 1);
var root_16 = from_html(`<p> </p> <!>`, 1);
var root_17 = from_html(`<p class="w-warn svelte-wqjyvl"> </p>`);
var root_18 = from_html(`<p class="said svelte-wqjyvl"> </p>`);
var root_19 = from_html(`<p class="quiet svelte-wqjyvl"> </p>`);
var root_20 = from_html(`<code class="svelte-wqjyvl"> </code> `, 1);
var root_21 = from_html(`<p class="files svelte-wqjyvl">Put back as the base has them: <!></p>`);
var root_22 = from_html(`<p class="files svelte-wqjyvl">Removed, as the base lacks them: <!></p>`);
var root_23 = from_html(`<p class="cmdline svelte-wqjyvl"><code class="svelte-wqjyvl"> </code> </p>`);
var root_24 = from_html(`<ul class="names svelte-wqjyvl"></ul> <!>`, 1);
var root_25 = from_html(`<p class="svelte-wqjyvl"> </p> <ul class="names svelte-wqjyvl"></ul> <!>`, 1);
var root_26 = from_html(`<h3 class="svelte-wqjyvl">The base's tests on this change</h3> <p> </p> <!> <!> <!> <!> <!> <!>`, 1);
var root_27 = from_html(`<span class="quiet svelte-wqjyvl"> </span>`);
var root_28 = from_html(`<code class="svelte-wqjyvl"> </code><!> `, 1);
var root_29 = from_html(`<p class="files svelte-wqjyvl">Placed: <!></p>`);
var root_30 = from_html(`<p class="files svelte-wqjyvl">Not separable: <!></p>`);
var root_31 = from_html(`<p class="svelte-wqjyvl">Passing without the change:</p><ul class="names svelte-wqjyvl"></ul>`, 1);
var root_32 = from_html(`<h3 class="svelte-wqjyvl">This change's new tests on the base</h3> <p> </p> <!> <!> <!> <!> <!>`, 1);
var root_33 = from_html(`<section class="base svelte-wqjyvl"><header class="svelte-wqjyvl"><h2 class="svelte-wqjyvl"><!> Base checks</h2> <!></header> <!> <p class="counts svelte-wqjyvl"> </p> <!> <!> <!> <!> <!></section>`);
var root_34 = from_html(`<button class="svelte-wqjyvl"><!> Copy as markdown</button>`);
var root_35 = from_html(`<p class="said svelte-wqjyvl"><!></p>`);
var root_36 = from_html(`<pre class="svelte-wqjyvl"> </pre>`);
var root_37 = from_html(`<h3 class="svelte-wqjyvl"><!></h3>`);
var root_38 = from_html(`<p class="li svelte-wqjyvl">• <!></p>`);
var root_39 = from_html(`<p class="row svelte-wqjyvl"><!></p>`);
var root_40 = from_html(`<p class="svelte-wqjyvl"><!></p>`);
var root_41 = from_html(`<div class="md svelte-wqjyvl"></div>`);
var root_42 = from_html(`<div class="gates svelte-wqjyvl"><!> <!> <!> <section class="cert svelte-wqjyvl"><header class="svelte-wqjyvl"><h2 class="svelte-wqjyvl"><!> Certificate</h2> <!></header> <!> <!></section></div>`);
function Gates($$anchor, $$props) {
	push($$props, true);
	let reload = prop($$props, "reload", 3, () => {}), certificate = prop($$props, "certificate", 3, null), weakened = prop($$props, "weakened", 19, () => []), toLine = prop($$props, "toLine", 3, null);
	const bc = user_derived(() => $$props.d.base_checks ?? null);
	let asked = state("");
	let asking = state(false);
	async function runBaseChecks() {
		if (get(asking)) return;
		set(asking, true);
		try {
			const r = await api(`/api/changes/${encodeURIComponent($$props.id)}/base-checks`, { method: "POST" });
			set(asked, r?.current ? "Already current for this tree — nothing ran." : "Base checks started — the base's tests on this change, then this change's new tests on the base.", true);
			await reload()();
		} catch (e) {
			const body = e instanceof Refused ? e.body : null;
			set(asked, body?.says ?? `That did not land: ${failure(e).says}`, true);
		} finally {
			set(asking, false);
		}
	}
	const attempts = user_derived(() => [...$$props.d.gates ?? []].reverse());
	let open = state(proxy({}));
	const passed = (g) => g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);
	const key = user_derived(() => $$props.id);
	const read = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}/certificate` : null, { tell: () => `devplane change export ${get(key)}` });
	const cert = user_derived(() => read.data ?? certificate());
	let said = state("");
	const newest = user_derived(() => $$props.d.gates?.[$$props.d.gates.length - 1]?.at ?? "");
	let seenNewest = null;
	user_effect(() => {
		const n = get(newest);
		untrack(() => {
			if (seenNewest !== null && n !== seenNewest) read.reload();
			seenNewest = n;
		});
	});
	async function copy() {
		if (!get(cert)?.markdown) return;
		set(said, await copyText(get(cert).markdown) ? "Copied — paste it into the pull request." : `Nothing was copied — this page has no clipboard. Run: devplane change export ${get(key)}`, true);
	}
	var div = root_42();
	var node = child(div);
	var consequent_1 = ($$anchor) => {
		var section = root_2$12();
		each(section, 21, weakened, (w) => w.path + w.matched, ($$anchor, w) => {
			var p = root_1$12();
			var node_1 = child(p);
			Icon(node_1, {
				name: "alert",
				size: 13
			});
			var b_1 = sibling(node_1, 2);
			var text = only_child(b_1, true);
			var button = sibling(b_1, 2);
			var code = child(button);
			var text_1 = only_child(code, true);
			reset(button);
			var node_2 = sibling(button, 2);
			Inline(node_2, { get text() {
				return get(w).why;
			} });
			var node_3 = sibling(node_2);
			var consequent = ($$anchor) => {
				var fragment = root$13();
				var node_4 = sibling(first_child(fragment));
				Inline(node_4, { get text() {
					return get(w).gate_says;
				} });
				append($$anchor, fragment);
			};
			if_block(node_3, ($$render) => {
				if (get(w).gate_says) $$render(consequent);
			});
			var text_2 = sibling(node_3);
			reset(p);
			template_effect(() => {
				set_text(text, weakKindSays[get(w).kind ?? "skip"] ?? "check weakened");
				set_text(text_1, get(w).path);
				set_text(text_2, ` · ${get(w).seen ? "read" : "not yet read"}`);
			});
			delegated("click", button, () => toLine()?.(get(w).path));
			append($$anchor, p);
		});
		reset(section);
		append($$anchor, section);
	};
	if_block(node, ($$render) => {
		if (weakened().length) $$render(consequent_1);
	});
	var node_5 = sibling(node, 2);
	var consequent_2 = ($$anchor) => {
		NoCheck($$anchor, { get project() {
			return $$props.d.project_id;
		} });
	};
	var d_1 = user_derived(() => declaresNoCheck($$props.d));
	var consequent_3 = ($$anchor) => {
		var p_1 = root_3$11();
		append($$anchor, p_1);
	};
	var alternate = ($$anchor) => {
		var fragment_2 = comment();
		var node_6 = first_child(fragment_2);
		each(node_6, 19, () => get(attempts), (g) => g.at, ($$anchor, g, i) => {
			const ok = user_derived(() => passed(get(g)));
			const shown = user_derived(() => get(open)[get(i)] ?? get(i) === 0);
			var section_1 = root_11$4();
			let classes;
			var button_1 = child(section_1);
			var node_7 = child(button_1);
			{
				let $0 = user_derived(() => get(shown) ? "down" : "right");
				Icon(node_7, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
			var node_8 = sibling(node_7, 2);
			{
				let $0 = user_derived(() => get(ok) ? "check" : "x");
				Icon(node_8, {
					get name() {
						return get($0);
					},
					size: 15
				});
			}
			var b_2 = sibling(node_8, 2);
			var text_3 = only_child(b_2, true);
			var span = sibling(b_2, 2);
			var text_4 = only_child(span);
			var span_1 = sibling(span, 2);
			var text_5 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var text_6 = only_child(span_2);
			var node_9 = sibling(span_2, 2);
			var consequent_4 = ($$anchor) => {
				var code_1 = root_4$9();
				var text_7 = only_child(code_1);
				template_effect(($0) => set_text(text_7, `tree ${$0 ?? ""}`), [() => get(g).commit.tree.slice(0, 10)]);
				append($$anchor, code_1);
			};
			if_block(node_9, ($$render) => {
				if (get(g).commit?.tree) $$render(consequent_4);
			});
			reset(button_1);
			var node_10 = sibling(button_1, 2);
			var consequent_8 = ($$anchor) => {
				var ol = root_10$4();
				each(ol, 21, () => get(g).commands, index, ($$anchor, c) => {
					const cok = user_derived(() => get(c).outcome.outcome === "exited" && get(c).outcome.code === 0);
					var li = root_9$5();
					let classes_1;
					var div_1 = child(li);
					var node_11 = child(div_1);
					{
						let $0 = user_derived(() => get(cok) ? "check" : "x");
						Icon(node_11, {
							get name() {
								return get($0);
							},
							size: 13
						});
					}
					var code_2 = sibling(node_11, 2);
					var text_8 = only_child(code_2, true);
					var span_3 = sibling(code_2, 2);
					var text_9 = only_child(span_3, true);
					var node_12 = sibling(span_3, 2);
					var consequent_5 = ($$anchor) => {
						var span_4 = root_5$9();
						var text_10 = only_child(span_4, true);
						template_effect(($0) => set_text(text_10, $0), [() => countSays(get(c).tests)]);
						append($$anchor, span_4);
					};
					if_block(node_12, ($$render) => {
						if (get(c).tests) $$render(consequent_5);
					});
					var span_5 = sibling(node_12, 2);
					var text_11 = only_child(span_5, true);
					reset(div_1);
					var node_13 = sibling(div_1, 2);
					var consequent_6 = ($$anchor) => {
						var ul = root_7$7();
						each(ul, 20, () => get(c).failures, (f) => f, ($$anchor, f) => {
							var li_1 = root_6$7();
							var text_12 = only_child(li_1, true);
							template_effect(() => set_text(text_12, f));
							append($$anchor, li_1);
						});
						reset(ul);
						append($$anchor, ul);
					};
					if_block(node_13, ($$render) => {
						if (get(c).failures?.length) $$render(consequent_6);
					});
					var node_14 = sibling(node_13, 2);
					var consequent_7 = ($$anchor) => {
						var fragment_3 = root_8$6();
						var pre = first_child(fragment_3);
						var text_13 = only_child(pre, true);
						var span_6 = sibling(pre, 2);
						var text_14 = only_child(span_6);
						template_effect(() => {
							set_text(text_13, get(c).output_tail);
							set_text(text_14, `${get(c).output_bytes ?? "?" ?? ""} bytes · digest ${get(c).output_digest ?? "—" ?? ""}`);
						});
						append($$anchor, fragment_3);
					};
					if_block(node_14, ($$render) => {
						if (get(c).output_tail) $$render(consequent_7);
					});
					reset(li);
					template_effect(($0, $1) => {
						classes_1 = set_class(li, 1, "svelte-wqjyvl", null, classes_1, { cbad: !get(cok) });
						set_text(text_8, get(c).command);
						set_text(text_9, $0);
						set_text(text_11, $1);
					}, [() => exit(get(c).outcome), () => took(get(c).duration_ms)]);
					append($$anchor, li);
				});
				reset(ol);
				append($$anchor, ol);
			};
			if_block(node_10, ($$render) => {
				if (get(shown)) $$render(consequent_8);
			});
			reset(section_1);
			template_effect(($0, $1) => {
				classes = set_class(section_1, 1, "attempt svelte-wqjyvl", null, classes, {
					ok: get(ok),
					bad: !get(ok)
				});
				set_attribute(button_1, "aria-expanded", get(shown));
				set_text(text_3, get(g).gate);
				set_text(text_4, `attempt ${get(g).attempt ?? ""}`);
				set_text(text_5, get(ok) ? "passed" : "failed");
				set_text(text_6, `${get(g).commands.length ?? ""} commands · ${$0 ?? ""} · ${$1 ?? ""}`);
			}, [() => took(get(g).duration_ms), () => ago(get(g).at)]);
			delegated("click", button_1, () => set(open, {
				...get(open),
				[get(i)]: !get(shown)
			}, true));
			append($$anchor, section_1);
		});
		append($$anchor, fragment_2);
	};
	if_block(node_5, ($$render) => {
		if (get(d_1)) $$render(consequent_2);
		else if (get(attempts).length === 0) $$render(consequent_3, 1);
		else $$render(alternate, -1);
	});
	var node_15 = sibling(node_5, 2);
	var consequent_28 = ($$anchor) => {
		var section_2 = root_33();
		var header = child(section_2);
		var h2 = child(header);
		var node_16 = child(h2);
		Icon(node_16, {
			name: "gate",
			size: 14
		});
		next();
		reset(h2);
		var node_17 = sibling(h2, 2);
		var consequent_9 = ($$anchor) => {
			var button_2 = root_12$4();
			var node_18 = child(button_2);
			Icon(node_18, {
				name: "play",
				size: 13
			});
			next();
			reset(button_2);
			template_effect(() => button_2.disabled = get(asking));
			delegated("click", button_2, runBaseChecks);
			append($$anchor, button_2);
		};
		if_block(node_17, ($$render) => {
			if (get(bc).can_run) $$render(consequent_9);
		});
		reset(header);
		var node_19 = sibling(header, 2);
		var consequent_12 = ($$anchor) => {
			var fragment_4 = root_16();
			var p_2 = first_child(fragment_4);
			var text_15 = only_child(p_2, true);
			var node_20 = sibling(p_2, 2);
			var consequent_11 = ($$anchor) => {
				var fragment_5 = root_15();
				var ul_1 = first_child(fragment_5);
				each(ul_1, 21, () => get(bc).headline.named, index, ($$anchor, n) => {
					var li_2 = root_13$4();
					var text_16 = only_child(li_2, true);
					template_effect(() => set_text(text_16, get(n)));
					append($$anchor, li_2);
				});
				reset(ul_1);
				var node_21 = sibling(ul_1, 2);
				var consequent_10 = ($$anchor) => {
					var p_3 = root_14$3();
					var text_17 = child(p_3);
					var code_3 = sibling(text_17);
					var text_18 = only_child(code_3);
					next();
					reset(p_3);
					template_effect(() => {
						set_text(text_17, `and ${get(bc).headline.named_more ?? ""} more — `);
						set_text(text_18, `devplane change show ${$$props.id ?? ""} --json`);
					});
					append($$anchor, p_3);
				};
				if_block(node_21, ($$render) => {
					if (get(bc).headline.named_more) $$render(consequent_10);
				});
				append($$anchor, fragment_5);
			};
			if_block(node_20, ($$render) => {
				if (get(bc).headline.named.length) $$render(consequent_11);
			});
			template_effect(() => {
				set_class(p_2, 1, `headline w-${get(bc).headline.weight ?? ""}`, "svelte-wqjyvl");
				set_text(text_15, get(bc).headline.says);
			});
			append($$anchor, fragment_4);
		};
		if_block(node_19, ($$render) => {
			if (get(bc).headline) $$render(consequent_12);
		});
		var p_4 = sibling(node_19, 2);
		var text_19 = only_child(p_4, true);
		var node_22 = sibling(p_4, 2);
		each(node_22, 17, () => get(bc).junit_notes, index, ($$anchor, n) => {
			var p_5 = root_17();
			var text_20 = only_child(p_5, true);
			template_effect(() => set_text(text_20, get(n)));
			append($$anchor, p_5);
		});
		var node_23 = sibling(node_22, 2);
		var consequent_13 = ($$anchor) => {
			var p_6 = root_18();
			var text_21 = only_child(p_6, true);
			template_effect(() => set_text(text_21, get(asked)));
			append($$anchor, p_6);
		};
		if_block(node_23, ($$render) => {
			if (get(asked)) $$render(consequent_13);
		});
		var node_24 = sibling(node_23, 2);
		var consequent_14 = ($$anchor) => {
			var p_7 = root_19();
			var text_22 = only_child(p_7);
			template_effect(($0) => set_text(text_22, `These runs describe tree ${$0 ?? ""}, not this one.`), [() => get(bc).tree?.slice(0, 10)]);
			append($$anchor, p_7);
		};
		if_block(node_24, ($$render) => {
			if (get(bc).stale && get(bc).on_change) $$render(consequent_14);
		});
		var node_25 = sibling(node_24, 2);
		var consequent_21 = ($$anchor) => {
			var fragment_6 = root_26();
			var p_8 = sibling(first_child(fragment_6), 2);
			var text_23 = only_child(p_8, true);
			var node_26 = sibling(p_8, 2);
			var consequent_15 = ($$anchor) => {
				var p_9 = root_21();
				var node_27 = sibling(child(p_9));
				each(node_27, 17, () => get(bc).on_change.restored, index, ($$anchor, f) => {
					var fragment_7 = root_20();
					var code_4 = first_child(fragment_7);
					var text_24 = only_child(code_4, true);
					var text_25 = sibling(code_4, 1, true);
					text_25.nodeValue = " ";
					template_effect(() => set_text(text_24, get(f)));
					append($$anchor, fragment_7);
				});
				reset(p_9);
				append($$anchor, p_9);
			};
			if_block(node_26, ($$render) => {
				if (get(bc).on_change.restored.length) $$render(consequent_15);
			});
			var node_28 = sibling(node_26, 2);
			var consequent_16 = ($$anchor) => {
				var p_10 = root_22();
				var node_29 = sibling(child(p_10));
				each(node_29, 17, () => get(bc).on_change.removed, index, ($$anchor, f) => {
					var fragment_8 = root_20();
					var code_5 = first_child(fragment_8);
					var text_26 = only_child(code_5, true);
					var text_27 = sibling(code_5, 1, true);
					text_27.nodeValue = " ";
					template_effect(() => set_text(text_26, get(f)));
					append($$anchor, fragment_8);
				});
				reset(p_10);
				append($$anchor, p_10);
			};
			if_block(node_28, ($$render) => {
				if (get(bc).on_change.removed.length) $$render(consequent_16);
			});
			var node_30 = sibling(node_28, 2);
			each(node_30, 17, () => get(bc).on_change.junit_notes, index, ($$anchor, n) => {
				var p_11 = root_19();
				var text_28 = only_child(p_11, true);
				template_effect(() => set_text(text_28, get(n)));
				append($$anchor, p_11);
			});
			var node_31 = sibling(node_30, 2);
			each(node_31, 17, () => get(bc).on_change.commands, index, ($$anchor, c) => {
				var p_12 = root_23();
				var code_6 = child(p_12);
				var text_29 = only_child(code_6, true);
				var text_30 = sibling(code_6);
				reset(p_12);
				template_effect(($0, $1) => {
					set_text(text_29, get(c).command);
					set_text(text_30, ` ${$0 ?? ""} · ${$1 ?? ""}`);
				}, [() => exit(get(c).outcome), () => countSays(get(c).tests)]);
				append($$anchor, p_12);
			});
			var node_32 = sibling(node_31, 2);
			var consequent_18 = ($$anchor) => {
				var fragment_9 = root_24();
				var ul_2 = first_child(fragment_9);
				each(ul_2, 21, () => get(bc).on_change.failing, index, ($$anchor, n) => {
					var li_3 = root_13$4();
					var text_31 = only_child(li_3, true);
					template_effect(() => set_text(text_31, get(n)));
					append($$anchor, li_3);
				});
				reset(ul_2);
				var node_33 = sibling(ul_2, 2);
				var consequent_17 = ($$anchor) => {
					var p_13 = root_14$3();
					var text_32 = child(p_13);
					var code_7 = sibling(text_32);
					var text_33 = only_child(code_7);
					next();
					reset(p_13);
					template_effect(() => {
						set_text(text_32, `and ${get(bc).on_change.failing_more ?? ""} more — `);
						set_text(text_33, `devplane change show ${$$props.id ?? ""} --json`);
					});
					append($$anchor, p_13);
				};
				if_block(node_33, ($$render) => {
					if (get(bc).on_change.failing_more) $$render(consequent_17);
				});
				append($$anchor, fragment_9);
			};
			if_block(node_32, ($$render) => {
				if (get(bc).on_change.failing.length) $$render(consequent_18);
			});
			var node_34 = sibling(node_32, 2);
			var consequent_20 = ($$anchor) => {
				var fragment_10 = root_25();
				var p_14 = first_child(fragment_10);
				var text_34 = only_child(p_14, true);
				var ul_3 = sibling(p_14, 2);
				each(ul_3, 21, () => get(bc).on_change.named.not_run_on_change, index, ($$anchor, n) => {
					var li_4 = root_13$4();
					var text_35 = only_child(li_4, true);
					template_effect(() => set_text(text_35, get(n)));
					append($$anchor, li_4);
				});
				reset(ul_3);
				var node_35 = sibling(ul_3, 2);
				var consequent_19 = ($$anchor) => {
					var p_15 = root_14$3();
					var text_36 = child(p_15);
					var code_8 = sibling(text_36);
					var text_37 = only_child(code_8);
					next();
					reset(p_15);
					template_effect(() => {
						set_text(text_36, `and ${get(bc).on_change.named.not_run_on_change_more ?? ""} more — `);
						set_text(text_37, `devplane change show ${$$props.id ?? ""} --json`);
					});
					append($$anchor, p_15);
				};
				if_block(node_35, ($$render) => {
					if (get(bc).on_change.named.not_run_on_change_more) $$render(consequent_19);
				});
				template_effect(() => set_text(text_34, get(bc).on_change.named.says));
				append($$anchor, fragment_10);
			};
			if_block(node_34, ($$render) => {
				if (get(bc).on_change.named?.not_run_on_change.length) $$render(consequent_20);
			});
			template_effect(() => {
				set_class(p_8, 1, `w-${get(bc).on_change.weight ?? ""}`, "svelte-wqjyvl");
				set_text(text_23, get(bc).on_change.says);
			});
			append($$anchor, fragment_6);
		};
		if_block(node_25, ($$render) => {
			if (get(bc).on_change) $$render(consequent_21);
		});
		var node_36 = sibling(node_25, 2);
		var consequent_27 = ($$anchor) => {
			var fragment_11 = root_32();
			var p_16 = sibling(first_child(fragment_11), 2);
			var text_38 = only_child(p_16, true);
			var node_37 = sibling(p_16, 2);
			var consequent_23 = ($$anchor) => {
				var p_17 = root_29();
				var node_38 = sibling(child(p_17));
				each(node_38, 17, () => get(bc).on_base.placed, index, ($$anchor, f) => {
					var fragment_12 = root_28();
					var code_9 = first_child(fragment_12);
					var text_39 = only_child(code_9, true);
					var node_39 = sibling(code_9);
					var consequent_22 = ($$anchor) => {
						var span_7 = root_27();
						var text_40 = only_child(span_7);
						template_effect(($0) => set_text(text_40, `blob ${$0 ?? ""}`), [() => get(f).blob.slice(0, 10)]);
						append($$anchor, span_7);
					};
					if_block(node_39, ($$render) => {
						if (get(f).blob) $$render(consequent_22);
					});
					var text_41 = sibling(node_39, 1, true);
					text_41.nodeValue = " ";
					template_effect(() => set_text(text_39, get(f).path));
					append($$anchor, fragment_12);
				});
				reset(p_17);
				append($$anchor, p_17);
			};
			if_block(node_37, ($$render) => {
				if (get(bc).on_base.placed.length) $$render(consequent_23);
			});
			var node_40 = sibling(node_37, 2);
			var consequent_24 = ($$anchor) => {
				var p_18 = root_30();
				var node_41 = sibling(child(p_18));
				each(node_41, 17, () => get(bc).on_base.not_separable, index, ($$anchor, f) => {
					var fragment_13 = root_20();
					var code_10 = first_child(fragment_13);
					var text_42 = only_child(code_10, true);
					var text_43 = sibling(code_10, 1, true);
					text_43.nodeValue = " ";
					template_effect(() => set_text(text_42, get(f)));
					append($$anchor, fragment_13);
				});
				reset(p_18);
				append($$anchor, p_18);
			};
			if_block(node_40, ($$render) => {
				if (get(bc).on_base.not_separable.length) $$render(consequent_24);
			});
			var node_42 = sibling(node_40, 2);
			each(node_42, 17, () => get(bc).on_base.commands, index, ($$anchor, c) => {
				var p_19 = root_23();
				var code_11 = child(p_19);
				var text_44 = only_child(code_11, true);
				var text_45 = sibling(code_11);
				reset(p_19);
				template_effect(($0, $1) => {
					set_text(text_44, get(c).command);
					set_text(text_45, ` ${$0 ?? ""} · ${$1 ?? ""}`);
				}, [() => exit(get(c).outcome), () => countSays(get(c).tests)]);
				append($$anchor, p_19);
			});
			var node_43 = sibling(node_42, 2);
			var consequent_25 = ($$anchor) => {
				var fragment_14 = root_31();
				var ul_4 = sibling(first_child(fragment_14));
				each(ul_4, 21, () => get(bc).on_base.passing, index, ($$anchor, n) => {
					var li_5 = root_13$4();
					var text_46 = only_child(li_5, true);
					template_effect(() => set_text(text_46, get(n)));
					append($$anchor, li_5);
				});
				reset(ul_4);
				append($$anchor, fragment_14);
			};
			if_block(node_43, ($$render) => {
				if (get(bc).on_base.passing.length) $$render(consequent_25);
			});
			var node_44 = sibling(node_43, 2);
			var consequent_26 = ($$anchor) => {
				var p_20 = root_14$3();
				var text_47 = child(p_20);
				var code_12 = sibling(text_47);
				var text_48 = only_child(code_12);
				next();
				reset(p_20);
				template_effect(() => {
					set_text(text_47, `and ${get(bc).on_base.passing_more ?? ""} more — `);
					set_text(text_48, `devplane change show ${$$props.id ?? ""} --json`);
				});
				append($$anchor, p_20);
			};
			if_block(node_44, ($$render) => {
				if (get(bc).on_base.passing_more) $$render(consequent_26);
			});
			template_effect(() => {
				set_class(p_16, 1, `w-${get(bc).on_base.weight ?? ""}`, "svelte-wqjyvl");
				set_text(text_38, get(bc).on_base.says);
			});
			append($$anchor, fragment_11);
		};
		if_block(node_36, ($$render) => {
			if (get(bc).on_base) $$render(consequent_27);
		});
		reset(section_2);
		template_effect(() => set_text(text_19, get(bc).counts_says));
		append($$anchor, section_2);
	};
	if_block(node_15, ($$render) => {
		if (get(bc)) $$render(consequent_28);
	});
	var section_3 = sibling(node_15, 2);
	var header_1 = child(section_3);
	var h2_1 = child(header_1);
	var node_45 = child(h2_1);
	Icon(node_45, {
		name: "shield",
		size: 14
	});
	next();
	reset(h2_1);
	var node_46 = sibling(h2_1, 2);
	var consequent_29 = ($$anchor) => {
		var button_3 = root_34();
		var node_47 = child(button_3);
		Icon(node_47, {
			name: "file",
			size: 13
		});
		next();
		reset(button_3);
		delegated("click", button_3, copy);
		append($$anchor, button_3);
	};
	if_block(node_46, ($$render) => {
		if (get(cert)?.markdown) $$render(consequent_29);
	});
	reset(header_1);
	var node_48 = sibling(header_1, 2);
	var consequent_30 = ($$anchor) => {
		var p_21 = root_35();
		var node_49 = child(p_21);
		Inline(node_49, { get text() {
			return get(said);
		} });
		reset(p_21);
		append($$anchor, p_21);
	};
	if_block(node_48, ($$render) => {
		if (get(said)) $$render(consequent_30);
	});
	var node_50 = sibling(node_48, 2);
	var consequent_31 = ($$anchor) => {
		Failed($$anchor, {
			what: "the certificate",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_32 = ($$anchor) => {
		Skeleton($$anchor, { rows: 3 });
	};
	var alternate_2 = ($$anchor) => {
		var div_2 = root_41();
		each(div_2, 21, () => blocks(get(cert).markdown), index, ($$anchor, b) => {
			var fragment_17 = comment();
			var node_51 = first_child(fragment_17);
			var consequent_33 = ($$anchor) => {
				var pre_1 = root_36();
				var text_49 = only_child(pre_1, true);
				template_effect(() => set_text(text_49, get(b).text));
				append($$anchor, pre_1);
			};
			var consequent_34 = ($$anchor) => {
				var h3 = root_37();
				var node_52 = child(h3);
				Inline(node_52, { get segs() {
					return get(b).segs;
				} });
				reset(h3);
				append($$anchor, h3);
			};
			var consequent_35 = ($$anchor) => {
				var p_22 = root_38();
				var node_53 = sibling(child(p_22));
				Inline(node_53, { get segs() {
					return get(b).segs;
				} });
				reset(p_22);
				append($$anchor, p_22);
			};
			var consequent_36 = ($$anchor) => {
				var p_23 = root_39();
				var node_54 = child(p_23);
				Inline(node_54, { get segs() {
					return get(b).segs;
				} });
				reset(p_23);
				append($$anchor, p_23);
			};
			var alternate_1 = ($$anchor) => {
				var p_24 = root_40();
				var node_55 = child(p_24);
				Inline(node_55, { get segs() {
					return get(b).segs;
				} });
				reset(p_24);
				append($$anchor, p_24);
			};
			if_block(node_51, ($$render) => {
				if (get(b).kind === "pre") $$render(consequent_33);
				else if (get(b).kind === "h") $$render(consequent_34, 1);
				else if (get(b).kind === "li") $$render(consequent_35, 2);
				else if (get(b).kind === "row") $$render(consequent_36, 3);
				else $$render(alternate_1, -1);
			});
			append($$anchor, fragment_17);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_50, ($$render) => {
		if (!get(cert) && read.failure) $$render(consequent_31);
		else if (!get(cert)) $$render(consequent_32, 1);
		else $$render(alternate_2, -1);
	});
	reset(section_3);
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Ledger.svelte
var root$12 = from_html(`<button> <span class="svelte-6p9ro4"> </span></button>`);
var root_1$11 = from_html(`<button class="svelte-6p9ro4"> </button>`);
var root_2$11 = from_html(`<p class="quiet svelte-6p9ro4"> <code> </code> has every one.</p>`);
var root_3$10 = from_html(`<span> </span>`);
var root_4$8 = from_html(`<span class="why svelte-6p9ro4"> </span>`);
var root_5$8 = from_html(`<p class="quiet svelte-6p9ro4"> </p>`);
var root_6$6 = from_html(`<p><!></p>`);
var root_7$6 = from_html(`<aside class="detail svelte-6p9ro4"><b> </b> <pre class="svelte-6p9ro4"> </pre> <!></aside>`);
var root_8$5 = from_html(`<div class="frame svelte-6p9ro4"><!></div> <!>`, 1);
var root_9$4 = from_html(`<div class="ledger svelte-6p9ro4"><div class="chips svelte-6p9ro4" role="group" aria-label="by authority"><button>everything <span class="svelte-6p9ro4"> </span></button> <!> <!></div> <!> <!></div>`);
function Ledger($$anchor, $$props) {
	push($$props, true);
	const LIMIT = 500;
	const key = user_derived(() => $$props.id);
	const read = resource(() => get(key) ? `/api/decisions?about=${encodeURIComponent(get(key))}&limit=${LIMIT}` : null, { tell: () => `devplane audit ${get(key)}` });
	const rows = user_derived(() => Array.isArray(read.data) ? read.data : null);
	let only = state(null);
	let selected = state(null);
	const AUTHORITIES = [
		"person",
		"rule",
		"timer",
		"nobody",
		"devplane"
	];
	const counts = user_derived(() => AUTHORITIES.map((a) => [a, (get(rows) ?? []).filter((r) => r.authority === a).length]));
	const shown = user_derived(() => (get(rows) ?? []).filter((r) => !get(only) || r.authority === get(only)));
	const columns = [
		{
			key: "at",
			label: "When",
			width: 110,
			sort: (r) => r.at
		},
		{
			key: "authority",
			label: "Authority",
			width: 120,
			sort: (r) => r.authority
		},
		{
			key: "action",
			label: "Action",
			width: 150,
			sort: (r) => r.action,
			mono: true
		},
		{
			key: "outcome",
			label: "Outcome",
			width: 110,
			sort: (r) => r.outcome
		},
		{
			key: "subject",
			label: "About",
			width: 320,
			mono: true
		},
		{
			key: "reason",
			label: "Why"
		}
	];
	const pick = user_derived(() => get(shown).find((r) => r.id === get(selected)) ?? null);
	let allChips = state(false);
	const zero = user_derived(() => get(counts).filter(([, n]) => n === 0));
	const chips = user_derived(() => get(allChips) || !get(rows) ? get(counts) : get(counts).filter(([a, n]) => n > 0 || get(only) === a));
	var div = root_9$4();
	var div_1 = child(div);
	var button = child(div_1);
	let classes;
	var span = sibling(child(button));
	var text$2 = only_child(span, true);
	reset(button);
	var node = sibling(button, 2);
	each(node, 17, () => get(chips), ([a, n]) => a, ($$anchor, $$item) => {
		var $$array = user_derived(() => to_array(get($$item), 2));
		let a = () => get($$array)[0];
		let n = () => get($$array)[1];
		var button_1 = root$12();
		let classes_1;
		var text_1 = child(button_1);
		var span_1 = sibling(text_1);
		var text_2 = only_child(span_1, true);
		reset(button_1);
		template_effect(() => {
			button_1.disabled = n() === 0;
			classes_1 = set_class(button_1, 1, "svelte-6p9ro4", null, classes_1, { on: get(only) === a() });
			set_text(text_1, `${a() ?? ""} `);
			set_text(text_2, get(rows) ? n() : "");
		});
		delegated("click", button_1, () => set(only, get(only) === a() ? null : a(), true));
		append($$anchor, button_1);
	});
	var node_1 = sibling(node, 2);
	var consequent = ($$anchor) => {
		var button_2 = root_1$11();
		var text_3 = only_child(button_2);
		template_effect(($0) => {
			set_attribute(button_2, "title", `authorities with nothing recorded: ${$0 ?? ""}`);
			set_text(text_3, `+${get(zero).length ?? ""}`);
		}, [() => get(zero).map(([a]) => a).join(", ")]);
		delegated("click", button_2, () => set(allChips, true));
		append($$anchor, button_2);
	};
	if_block(node_1, ($$render) => {
		if (get(rows) && !get(allChips) && get(zero).length > 0) $$render(consequent);
	});
	reset(div_1);
	var node_2 = sibling(div_1, 2);
	var consequent_1 = ($$anchor) => {
		var p = root_2$11();
		var text_4 = child(p);
		text_4.nodeValue = "The newest 500 are shown; ";
		var code = sibling(text_4);
		var text_5 = only_child(code);
		next();
		reset(p);
		template_effect(() => set_text(text_5, `devplane audit ${get(key) ?? ""}`));
		append($$anchor, p);
	};
	if_block(node_2, ($$render) => {
		if (get(rows) && get(rows).length >= LIMIT) $$render(consequent_1);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_2 = ($$anchor) => {
		{
			let $0 = user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the decisions",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	var alternate_2 = ($$anchor) => {
		var fragment_1 = root_8$5();
		var div_2 = first_child(fragment_1);
		var node_4 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_5 = first_child(fragment_2);
				var consequent_3 = ($$anchor) => {
					var span_2 = root_3$10();
					var text_6 = only_child(span_2, true);
					template_effect(($0) => {
						set_attribute(span_2, "title", r().at);
						set_text(text_6, $0);
					}, [() => ago(r().at)]);
					append($$anchor, span_2);
				};
				var consequent_4 = ($$anchor) => {
					{
						let $0 = user_derived(() => r().authority === "person" ? "work" : r().authority === "rule" ? "wait" : r().authority === "devplane" ? "none" : "fail");
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
				var consequent_5 = ($$anchor) => {
					Pill($$anchor, {
						get word() {
							return r().outcome;
						},
						dot: false
					});
				};
				var consequent_6 = ($$anchor) => {
					var text_7 = text();
					template_effect(() => set_text(text_7, r().action));
					append($$anchor, text_7);
				};
				var consequent_7 = ($$anchor) => {
					var text_8 = text();
					template_effect(() => set_text(text_8, r().subject));
					append($$anchor, text_8);
				};
				var alternate = ($$anchor) => {
					var span_3 = root_4$8();
					var text_9 = only_child(span_3, true);
					template_effect(() => set_text(text_9, r().reason ?? ""));
					append($$anchor, span_3);
				};
				if_block(node_5, ($$render) => {
					if (c().key === "at") $$render(consequent_3);
					else if (c().key === "authority") $$render(consequent_4, 1);
					else if (c().key === "outcome") $$render(consequent_5, 2);
					else if (c().key === "action") $$render(consequent_6, 3);
					else if (c().key === "subject") $$render(consequent_7, 4);
					else $$render(alternate, -1);
				});
				append($$anchor, fragment_2);
			};
			const empty = ($$anchor) => {
				var fragment_7 = comment();
				var node_6 = first_child(fragment_7);
				var consequent_8 = ($$anchor) => {
					Skeleton($$anchor, {});
				};
				var alternate_1 = ($$anchor) => {
					var p_1 = root_5$8();
					var text_10 = only_child(p_1, true);
					template_effect(() => set_text(text_10, get(only) ? `Nothing about this change was decided by ${get(only)}.` : "Nothing has been decided about this change."));
					append($$anchor, p_1);
				};
				if_block(node_6, ($$render) => {
					if (get(rows) === null) $$render(consequent_8);
					else $$render(alternate_1, -1);
				});
				append($$anchor, fragment_7);
			};
			Grid(node_4, {
				id: "change-ledger",
				get columns() {
					return columns;
				},
				get rows() {
					return get(shown);
				},
				key: (r) => r.id,
				label: "decisions about this change",
				get selected() {
					return get(selected);
				},
				set selected($$value) {
					set(selected, $$value, true);
				},
				cell,
				empty,
				$$slots: {
					cell: true,
					empty: true
				}
			});
		}
		reset(div_2);
		var node_7 = sibling(div_2, 2);
		var consequent_10 = ($$anchor) => {
			var aside = root_7$6();
			var b = child(aside);
			var text_11 = only_child(b, true);
			var text_12 = sibling(b);
			var pre = sibling(text_12);
			var text_13 = only_child(pre, true);
			var node_8 = sibling(pre, 2);
			var consequent_9 = ($$anchor) => {
				var p_2 = root_6$6();
				var node_9 = child(p_2);
				Inline(node_9, { get text() {
					return get(pick).reason;
				} });
				reset(p_2);
				append($$anchor, p_2);
			};
			if_block(node_8, ($$render) => {
				if (get(pick).reason) $$render(consequent_9);
			});
			reset(aside);
			template_effect(($0) => {
				set_text(text_11, get(pick).action);
				set_text(text_12, ` · ${get(pick).outcome ?? ""} · by ${get(pick).authority ?? ""} · ${$0 ?? ""} `);
				set_text(text_13, get(pick).subject);
			}, [() => new Date(get(pick).at).toLocaleString()]);
			append($$anchor, aside);
		};
		if_block(node_7, ($$render) => {
			if (get(pick)) $$render(consequent_10);
		});
		append($$anchor, fragment_1);
	};
	if_block(node_3, ($$render) => {
		if (read.failure) $$render(consequent_2);
		else $$render(alternate_2, -1);
	});
	reset(div);
	template_effect(() => {
		classes = set_class(button, 1, "svelte-6p9ro4", null, classes, { on: get(only) === null });
		set_text(text$2, get(rows) ? get(rows).length : "");
	});
	delegated("click", button, () => set(only, null));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/ui/Md.svelte
var root$11 = from_html(`<pre class="svelte-ozut7t"> </pre>`);
var root_1$10 = from_html(`<p class="h svelte-ozut7t"><!></p>`);
var root_2$10 = from_html(`<p class="li svelte-ozut7t"><span aria-hidden="true">•</span> <!></p>`);
var root_3$9 = from_html(`<p class="row svelte-ozut7t"><!></p>`);
var root_4$7 = from_html(`<p class="svelte-ozut7t"><!></p>`);
var root_5$7 = from_html(`<div class="md svelte-ozut7t"></div>`);
function Md($$anchor, $$props) {
	push($$props, true);
	let text = prop($$props, "text", 3, "");
	const parts = user_derived(() => blocks(text()));
	var div = root_5$7();
	each(div, 21, () => get(parts), index, ($$anchor, b) => {
		var fragment = comment();
		var node = first_child(fragment);
		var consequent = ($$anchor) => {
			var pre = root$11();
			var text_1 = only_child(pre, true);
			template_effect(() => set_text(text_1, get(b).text));
			append($$anchor, pre);
		};
		var consequent_1 = ($$anchor) => {
			var p = root_1$10();
			var node_1 = child(p);
			Inline(node_1, { get segs() {
				return get(b).segs;
			} });
			reset(p);
			append($$anchor, p);
		};
		var consequent_2 = ($$anchor) => {
			var p_1 = root_2$10();
			var node_2 = sibling(child(p_1), 2);
			Inline(node_2, { get segs() {
				return get(b).segs;
			} });
			reset(p_1);
			append($$anchor, p_1);
		};
		var consequent_3 = ($$anchor) => {
			var p_2 = root_3$9();
			var node_3 = child(p_2);
			Inline(node_3, { get segs() {
				return get(b).segs;
			} });
			reset(p_2);
			append($$anchor, p_2);
		};
		var alternate = ($$anchor) => {
			var p_3 = root_4$7();
			var node_4 = child(p_3);
			Inline(node_4, { get segs() {
				return get(b).segs;
			} });
			reset(p_3);
			append($$anchor, p_3);
		};
		if_block(node, ($$render) => {
			if (get(b).kind === "pre") $$render(consequent);
			else if (get(b).kind === "h") $$render(consequent_1, 1);
			else if (get(b).kind === "li") $$render(consequent_2, 2);
			else if (get(b).kind === "row") $$render(consequent_3, 3);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/change/Agent.svelte
var root$10 = from_html(`<p class="quiet svelte-1w0chr6">No agent has run on this change yet.</p>`);
var root_1$9 = from_html(`<span class="quiet svelte-1w0chr6"> </span>`);
var root_2$9 = from_html(`<div class="fail svelte-1w0chr6"><!></div>`);
var root_3$8 = from_html(`<li class="quiet svelte-1w0chr6" aria-busy="true">…</li>`);
var root_4$6 = from_html(`<li class="quiet svelte-1w0chr6">Nothing has been said yet.</li>`);
var root_5$6 = from_html(`<li class="quiet svelte-1w0chr6"> <code class="svelte-1w0chr6"> </code> has every one.</li>`);
var root_6$5 = from_html(`<li><span class="who svelte-1w0chr6"> </span> <div class="text svelte-1w0chr6"><!></div></li>`);
var root_7$5 = from_html(`<span class="warn svelte-1w0chr6"><!></span> <button type="button" class="danger svelte-1w0chr6">Stop the run</button> <button type="button" class="svelte-1w0chr6">Keep it running</button>`, 1);
var root_8$4 = from_html(`<button type="button" class="ghost svelte-1w0chr6"><!> Stop…</button>`);
var root_9$3 = from_html(`<p class="said svelte-1w0chr6" role="status"><!></p>`);
var root_10$3 = from_html(`<li class="svelte-1w0chr6"><!><code class="svelte-1w0chr6"> </code></li>`);
var root_11$3 = from_html(`<li class="quiet svelte-1w0chr6"> </li>`);
var root_12$3 = from_html(`<code class="svelte-1w0chr6"> </code>`);
var root_13$3 = from_html(`<li class="svelte-1w0chr6"><!><span> </span><!></li>`);
var root_14$2 = from_html(`<div class="agent svelte-1w0chr6"><section class="convo svelte-1w0chr6"><header class="bar svelte-1w0chr6"><!> <b> </b> <!> <!> <code class="quiet svelte-1w0chr6"> </code> <!></header> <!> <ol class="turns svelte-1w0chr6"><!> <!></ol> <form class="composer svelte-1w0chr6"><textarea rows="3" placeholder="Another turn for the agent — it is queued if one is under way" aria-label="another turn for the agent" class="svelte-1w0chr6"></textarea> <div class="send svelte-1w0chr6"><span class="quiet svelte-1w0chr6">⌘↵ to send</span> <!> <button type="submit" class="primary svelte-1w0chr6"><!> Send</button></div></form> <!></section> <aside class="record svelte-1w0chr6"><h2 class="svelte-1w0chr6">Files written <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul> <h2 class="svelte-1w0chr6">Running now <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul></aside></div>`);
function Agent($$anchor, $$props) {
	push($$props, true);
	function runState(s) {
		if (typeof s === "string") return { word: s.replace(/_/g, " ") };
		if (s && typeof s === "object" && "waiting" in s) {
			const w = s.waiting;
			if (w === "permission") return {
				word: "waiting on you — permission",
				tone: "wait"
			};
			if (w === "question") return {
				word: "waiting on you — question",
				tone: "wait"
			};
			if (w === "idle") return {
				word: "idle — waiting for the next turn",
				tone: "none"
			};
			if (w === "job") return {
				word: "waiting on a command it started",
				tone: "work"
			};
			if (w && typeof w === "object" && "other" in w) return {
				word: `waiting on you — ${String(w.other)}`,
				tone: "wait"
			};
			return {
				word: "waiting on you",
				tone: "wait"
			};
		}
		return s == null ? null : {
			word: "in a state this page does not know",
			tone: "none"
		};
	}
	const LIMIT = 200;
	const EVERY_MS = 2e3;
	const key = user_derived(() => $$props.run);
	const runRead = resource(() => get(key) ? `/api/runs/${encodeURIComponent(get(key))}` : null, {
		every: EVERY_MS,
		tell: () => `devplane run show ${get(key)}`
	});
	const said_ = resource(() => get(key) ? `/api/runs/${encodeURIComponent(get(key))}/messages?limit=${LIMIT}` : null, {
		every: EVERY_MS,
		tell: () => `devplane run show ${get(key)}`
	});
	const detail = user_derived(() => runRead.data);
	const messages = user_derived(() => Array.isArray(said_.data) ? said_.data : []);
	let said = state("");
	let draft = state("");
	let sending = state(false);
	let survives = state("");
	user_effect(() => {
		get(key);
		set(survives, "");
		set(said, "");
	});
	const shownState = user_derived(() => runState(get(detail)?.state));
	const running = user_derived(() => (get(detail)?.recent_tools ?? []).filter((t) => t.ok == null).map((t) => ({
		tool: t.tool,
		what: t.input?.command ?? t.input?.file_path ?? ""
	})));
	async function send() {
		const text = get(draft).trim();
		if (!text || !$$props.run || get(sending)) return;
		set(sending, true);
		try {
			const r = await api(`/api/runs/${encodeURIComponent($$props.run)}/prompt`, {
				method: "POST",
				body: JSON.stringify({ text })
			});
			set(draft, "");
			set(said, r?.says ?? "Sent.", true);
			said_.reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		} finally {
			set(sending, false);
		}
	}
	async function askStop() {
		try {
			const r = await api(`/api/runs/${encodeURIComponent($$props.run)}/stop`);
			set(survives, r?.says ?? "Stopping ends this run.", true);
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	async function stop() {
		try {
			await api(`/api/runs/${encodeURIComponent($$props.run)}/stop`, { method: "POST" });
			set(survives, "");
			set(said, "Stopped.");
			runRead.reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	function keydown(e) {
		if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			send();
		}
	}
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var p = root$10();
		append($$anchor, p);
	};
	var alternate_1 = ($$anchor) => {
		var div = root_14$2();
		var section = child(div);
		var header = child(section);
		var node_1 = child(header);
		Icon(node_1, {
			name: "agent",
			size: 15
		});
		var b = sibling(node_1, 2);
		var text_1 = only_child(b, true);
		var node_2 = sibling(b, 2);
		var consequent_1 = ($$anchor) => {
			var span = root_1$9();
			var text_2 = only_child(span, true);
			template_effect(() => set_text(text_2, get(detail).model));
			append($$anchor, span);
		};
		if_block(node_2, ($$render) => {
			if (get(detail)?.model) $$render(consequent_1);
		});
		var node_3 = sibling(node_2, 2);
		var consequent_2 = ($$anchor) => {
			Pill($$anchor, {
				get word() {
					return get(shownState).word;
				},
				get as() {
					return get(shownState).tone;
				}
			});
		};
		if_block(node_3, ($$render) => {
			if (get(shownState)) $$render(consequent_2);
		});
		var code = sibling(node_3, 2);
		var text_3 = only_child(code, true);
		var node_4 = sibling(code, 2);
		var consequent_3 = ($$anchor) => {
			var span_1 = root_1$9();
			var text_4 = only_child(span_1);
			template_effect(() => set_text(text_4, `· latest of ${$$props.d.runs.length ?? ""} runs`));
			append($$anchor, span_1);
		};
		if_block(node_4, ($$render) => {
			if ($$props.d.runs && $$props.d.runs.length > 1) $$render(consequent_3);
		});
		reset(header);
		var node_5 = sibling(header, 2);
		var consequent_4 = ($$anchor) => {
			var div_1 = root_2$9();
			var node_6 = child(div_1);
			{
				let $0 = user_derived(() => said_.data !== null);
				Failed(node_6, {
					what: "the conversation",
					get failure() {
						return said_.failure;
					},
					get at() {
						return said_.at;
					},
					get stale() {
						return get($0);
					}
				});
			}
			reset(div_1);
			append($$anchor, div_1);
		};
		var consequent_5 = ($$anchor) => {
			var div_2 = root_2$9();
			var node_7 = child(div_2);
			{
				let $0 = user_derived(() => runRead.data !== null);
				Failed(node_7, {
					what: "the run",
					get failure() {
						return runRead.failure;
					},
					get at() {
						return runRead.at;
					},
					get stale() {
						return get($0);
					}
				});
			}
			reset(div_2);
			append($$anchor, div_2);
		};
		if_block(node_5, ($$render) => {
			if (said_.failure) $$render(consequent_4);
			else if (runRead.failure) $$render(consequent_5, 1);
		});
		var ol = sibling(node_5, 2);
		var node_8 = child(ol);
		var consequent_6 = ($$anchor) => {
			var li = root_3$8();
			append($$anchor, li);
		};
		var consequent_7 = ($$anchor) => {
			var li_1 = root_4$6();
			append($$anchor, li_1);
		};
		var consequent_8 = ($$anchor) => {
			var li_2 = root_5$6();
			var text_5 = child(li_2);
			text_5.nodeValue = "Only 200 turns are shown here; ";
			var code_1 = sibling(text_5);
			var text_6 = only_child(code_1);
			next();
			reset(li_2);
			template_effect(() => set_text(text_6, `devplane run show ${$$props.run ?? ""}`));
			append($$anchor, li_2);
		};
		if_block(node_8, ($$render) => {
			if (said_.phase === "loading") $$render(consequent_6);
			else if (said_.data !== null && get(messages).length === 0) $$render(consequent_7, 1);
			else if (get(messages).length >= LIMIT) $$render(consequent_8, 2);
		});
		var node_9 = sibling(node_8, 2);
		each(node_9, 17, () => get(messages), (m) => m.id, ($$anchor, m) => {
			var li_3 = root_6$5();
			var span_2 = child(li_3);
			var text_7 = only_child(span_2, true);
			var div_3 = sibling(span_2, 2);
			Md(child(div_3), { get text() {
				return get(m).text;
			} });
			reset(div_3);
			reset(li_3);
			template_effect(() => {
				set_class(li_3, 1, `turn ${get(m).role ?? ""}`, "svelte-1w0chr6");
				set_text(text_7, get(m).role === "user" ? "you" : get(m).role);
			});
			append($$anchor, li_3);
		});
		reset(ol);
		var form = sibling(ol, 2);
		var textarea = child(form);
		remove_textarea_child(textarea);
		var div_4 = sibling(textarea, 2);
		var node_11 = sibling(child(div_4), 2);
		var consequent_9 = ($$anchor) => {
			var fragment_2 = root_7$5();
			var span_3 = first_child(fragment_2);
			var node_12 = child(span_3);
			Inline(node_12, { get text() {
				return get(survives);
			} });
			reset(span_3);
			var button = sibling(span_3, 2);
			var button_1 = sibling(button, 2);
			delegated("click", button, stop);
			delegated("click", button_1, () => set(survives, ""));
			append($$anchor, fragment_2);
		};
		var alternate = ($$anchor) => {
			var button_2 = root_8$4();
			var node_13 = child(button_2);
			Icon(node_13, {
				name: "stop",
				size: 12
			});
			next();
			reset(button_2);
			delegated("click", button_2, askStop);
			append($$anchor, button_2);
		};
		if_block(node_11, ($$render) => {
			if (get(survives)) $$render(consequent_9);
			else $$render(alternate, -1);
		});
		var button_3 = sibling(node_11, 2);
		var node_14 = child(button_3);
		Icon(node_14, {
			name: "play",
			size: 12
		});
		next();
		reset(button_3);
		reset(div_4);
		reset(form);
		var node_15 = sibling(form, 2);
		var consequent_10 = ($$anchor) => {
			var p_1 = root_9$3();
			var node_16 = child(p_1);
			Inline(node_16, { get text() {
				return get(said);
			} });
			reset(p_1);
			append($$anchor, p_1);
		};
		if_block(node_15, ($$render) => {
			if (get(said)) $$render(consequent_10);
		});
		reset(section);
		var aside = sibling(section, 2);
		var h2 = child(aside);
		var span_4 = sibling(child(h2));
		var text_8 = only_child(span_4, true);
		reset(h2);
		var ul = sibling(h2, 2);
		each(ul, 20, () => get(detail)?.wrote ?? [], (f) => f, ($$anchor, f) => {
			var li_4 = root_10$3();
			var node_17 = child(li_4);
			Icon(node_17, {
				name: "file",
				size: 12
			});
			var code_2 = sibling(node_17);
			var text_9 = only_child(code_2, true);
			reset(li_4);
			template_effect(() => set_text(text_9, f));
			append($$anchor, li_4);
		}, ($$anchor) => {
			var li_5 = root_11$3();
			var text_10 = only_child(li_5, true);
			template_effect(() => set_text(text_10, get(detail) ? "none yet" : runRead.failure ? "not read" : "…"));
			append($$anchor, li_5);
		});
		reset(ul);
		var h2_1 = sibling(ul, 2);
		var span_5 = sibling(child(h2_1));
		var text_11 = only_child(span_5, true);
		reset(h2_1);
		var ul_1 = sibling(h2_1, 2);
		each(ul_1, 21, () => get(running), index, ($$anchor, c) => {
			var li_6 = root_13$3();
			var node_18 = child(li_6);
			Icon(node_18, {
				name: "terminal",
				size: 12
			});
			var span_6 = sibling(node_18);
			var text_12 = only_child(span_6, true);
			var node_19 = sibling(span_6);
			var consequent_11 = ($$anchor) => {
				var code_3 = root_12$3();
				var text_13 = only_child(code_3, true);
				template_effect(() => set_text(text_13, get(c).what));
				append($$anchor, code_3);
			};
			if_block(node_19, ($$render) => {
				if (get(c).what) $$render(consequent_11);
			});
			reset(li_6);
			template_effect(() => set_text(text_12, get(c).tool));
			append($$anchor, li_6);
		}, ($$anchor) => {
			var li_7 = root_11$3();
			var text_14 = only_child(li_7, true);
			template_effect(() => set_text(text_14, get(detail) ? "no tool call in flight" : runRead.failure ? "not read" : "…"));
			append($$anchor, li_7);
		});
		reset(ul_1);
		reset(aside);
		reset(div);
		template_effect(($0, $1) => {
			set_text(text_1, get(detail)?.agent ?? "agent");
			set_text(text_3, $0);
			button_3.disabled = $1;
			set_text(text_8, get(detail) ? get(detail).wrote?.length ?? 0 : "");
			set_text(text_11, get(detail) ? get(running).length : "");
		}, [() => $$props.run.slice(0, 18), () => !get(draft).trim() || get(sending)]);
		event("submit", form, (e) => {
			e.preventDefault();
			send();
		});
		delegated("keydown", textarea, keydown);
		bind_value(textarea, () => get(draft), ($$value) => set(draft, $$value));
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (!$$props.run) $$render(consequent);
		else $$render(alternate_1, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
//#region src/surfaces/change/pair.ts
function names(step, task) {
	const key = fold(task.text);
	return key.length > 0 && fold(step.content).includes(key);
}
function fold(s) {
	return s.toLowerCase().replace(/\s+/g, " ").trim();
}
function pair(tasks, steps) {
	const taken = /* @__PURE__ */ new Set();
	const rows = tasks.map((task) => {
		const at = steps.findIndex((s, i) => !taken.has(i) && names(s, task));
		if (at === -1) return {
			task,
			step: null
		};
		taken.add(at);
		return {
			task,
			step: steps[at]
		};
	});
	steps.forEach((step, i) => {
		if (!taken.has(i)) rows.push({
			task: null,
			step
		});
	});
	return rows;
}
//#endregion
//#region src/surfaces/change/Plan.svelte
var root$9 = from_html(`<p class="dim svelte-rag4cy">No tasks were sent and no plan was reported.</p>`);
var root_1$8 = from_html(`<span class="where svelte-rag4cy"> </span>`);
var root_2$8 = from_html(`<span class="text svelte-rag4cy"> </span><!>`, 1);
var root_3$7 = from_html(`<span class="status svelte-rag4cy"> </span><span class="text svelte-rag4cy"> </span>`, 1);
var root_4$5 = from_html(`<li><span class="task svelte-rag4cy"><!></span> <span class="step svelte-rag4cy"><!></span></li>`);
var root_5$5 = from_html(`<p class="dim tally svelte-rag4cy"> </p>`);
var root_6$4 = from_html(`<ol class="rows svelte-rag4cy"></ol> <!>`, 1);
var root_7$4 = from_html(`<section class="plan" aria-label="tasks and the agent's plan"><div class="heads svelte-rag4cy"><h3 class="svelte-rag4cy">Tasks sent <span class="n svelte-rag4cy"> </span></h3> <h3 class="svelte-rag4cy">The agent's plan <span class="n svelte-rag4cy"> </span></h3></div> <!></section>`);
function Plan($$anchor, $$props) {
	push($$props, true);
	let tasks = prop($$props, "tasks", 19, () => []), steps = prop($$props, "steps", 19, () => []);
	const rows = user_derived(() => pair(tasks(), steps()));
	const matched = user_derived(() => get(rows).filter((r) => r.task && r.step).length);
	const n = (k, one, many) => `${k} ${k === 1 ? one : many}`;
	const tally = user_derived(() => [
		get(matched) ? `${n(get(matched), "task", "tasks")} matched to a plan step` : "",
		tasks().length - get(matched) ? `${n(tasks().length - get(matched), "task", "tasks")} not linked to a plan step` : "",
		steps().length - get(matched) ? `${n(steps().length - get(matched), "plan step", "plan steps")} not linked to a task` : ""
	].filter(Boolean).join(" · "));
	var section = root_7$4();
	var div = child(section);
	var h3 = child(div);
	var span = sibling(child(h3));
	var text = only_child(span, true);
	reset(h3);
	var h3_1 = sibling(h3, 2);
	var span_1 = sibling(child(h3_1));
	var text_1 = only_child(span_1, true);
	reset(h3_1);
	reset(div);
	var node = sibling(div, 2);
	var consequent = ($$anchor) => {
		var p = root$9();
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var fragment = root_6$4();
		var ol = first_child(fragment);
		each(ol, 21, () => get(rows), index, ($$anchor, r) => {
			var li = root_4$5();
			let classes;
			var span_2 = child(li);
			var node_1 = child(span_2);
			var consequent_2 = ($$anchor) => {
				var fragment_1 = root_2$8();
				var span_3 = first_child(fragment_1);
				var text_2 = only_child(span_3, true);
				var node_2 = sibling(span_3);
				var consequent_1 = ($$anchor) => {
					var span_4 = root_1$8();
					var text_3 = only_child(span_4);
					template_effect(() => set_text(text_3, `${get(r).task.path ?? ""}${get(r).task.line ? `:${get(r).task.line}` : ""}`));
					append($$anchor, span_4);
				};
				if_block(node_2, ($$render) => {
					if (get(r).task.path) $$render(consequent_1);
				});
				template_effect(() => set_text(text_2, get(r).task.text));
				append($$anchor, fragment_1);
			};
			if_block(node_1, ($$render) => {
				if (get(r).task) $$render(consequent_2);
			});
			reset(span_2);
			var span_5 = sibling(span_2, 2);
			var node_3 = child(span_5);
			var consequent_3 = ($$anchor) => {
				var fragment_2 = root_3$7();
				var span_6 = first_child(fragment_2);
				var text_4 = only_child(span_6, true);
				var span_7 = sibling(span_6);
				var text_5 = only_child(span_7, true);
				template_effect(($0) => {
					set_text(text_4, $0);
					set_text(text_5, get(r).step.content);
				}, [() => get(r).step.status.replace(/_/g, " ")]);
				append($$anchor, fragment_2);
			};
			if_block(node_3, ($$render) => {
				if (get(r).step) $$render(consequent_3);
			});
			reset(span_5);
			reset(li);
			template_effect(() => classes = set_class(li, 1, "row svelte-rag4cy", null, classes, { pair: !!(get(r).task && get(r).step) }));
			append($$anchor, li);
		});
		reset(ol);
		var node_4 = sibling(ol, 2);
		var consequent_4 = ($$anchor) => {
			var p_1 = root_5$5();
			var text_6 = only_child(p_1, true);
			template_effect(() => set_text(text_6, get(tally)));
			append($$anchor, p_1);
		};
		if_block(node_4, ($$render) => {
			if (get(tally)) $$render(consequent_4);
		});
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (tasks().length === 0 && steps().length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => {
		set_text(text, tasks().length);
		set_text(text_1, steps().length);
	});
	append($$anchor, section);
	pop();
}
//#endregion
//#region src/surfaces/change/Tasks.svelte
var root$8 = from_html(`<li> </li>`);
var root_1$7 = from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> Ticked, but no run was sent them <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_2$7 = from_html(`<section class="note svelte-6i9c4j"><h3 class="svelte-6i9c4j">Sent, not ticked <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_3$6 = from_html(`<p class="counts svelte-6i9c4j"> </p> <!> <!>`, 1);
var root_4$4 = from_html(`<p class="quiet svelte-6i9c4j"><!></p>`);
var root_5$4 = from_html(`<div class="drift svelte-6i9c4j"><p class="svelte-6i9c4j"><!></p> <button>Tell the run</button> <button>Accept what it saw</button></div>`);
var root_6$3 = from_html(`<p class="said svelte-6i9c4j" role="status"> </p>`);
var root_7$3 = from_html(`<p class="said svelte-6i9c4j" role="status"><!></p>`);
var root_8$3 = from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> The specification changed under a run</h3> <!> <!></section>`);
var root_9$2 = from_html(`<tr><td class="mono svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td> </td></tr>`);
var root_10$2 = from_html(`<section><h2 class="svelte-6i9c4j">Requirements and the tasks that cite them</h2> <table class="svelte-6i9c4j"><thead><tr><th class="svelte-6i9c4j">requirement</th><th class="svelte-6i9c4j">tasks</th><th class="svelte-6i9c4j">ticked</th><th class="svelte-6i9c4j">seen by a passing check</th></tr></thead><tbody></tbody></table></section>`);
var root_11$2 = from_html(`<li><code class="svelte-6i9c4j"> </code> <!></li>`);
var root_12$2 = from_html(`<section><h2 class="svelte-6i9c4j">Every run</h2> <ul class="runs svelte-6i9c4j"></ul></section>`);
var root_13$2 = from_html(`<section><h2 class="svelte-6i9c4j"> </h2> <ol class="outline svelte-6i9c4j"></ol></section>`);
var root_14$1 = from_html(`<div class="tasks svelte-6i9c4j"><!> <!> <!> <section><h2 class="svelte-6i9c4j">Sent to the latest run</h2> <!> <!></section> <!> <!></div>`);
function Tasks($$anchor, $$props) {
	push($$props, true);
	let reload = prop($$props, "reload", 3, async () => {});
	const run = user_derived(() => $$props.lastRun);
	const read = resource(() => get(run) ? `/api/runs/${encodeURIComponent(get(run))}` : null, { tell: () => `devplane run show ${get(run)}` });
	const steps = user_derived(() => read.data?.plan ?? []);
	const planKnown = user_derived(() => !get(run) || read.phase !== "loading");
	let said = state("");
	const pending = writer();
	const sent = user_derived(() => ($$props.d.run_rows ?? []).find((r) => r.id === $$props.lastRun)?.sent ?? $$props.d.run_rows?.[$$props.d.run_rows.length - 1]?.sent ?? []);
	async function decide(verb, run) {
		set(said, "");
		await pending.run(verb === "tell" ? "Telling the run" : "Accepting", async () => {
			try {
				const id = encodeURIComponent($$props.d.id);
				const route = verb === "tell" ? `/api/changes/${id}/drift/tell` : `/api/changes/${id}/drift/accept`;
				await api(route, {
					method: "POST",
					body: JSON.stringify({ run })
				});
				set(said, verb === "tell" ? "Told — the run was handed the files that changed." : "Accepted — the change now works to what the run saw.", true);
				await reload()();
			} catch (e) {
				set(said, `That did not land: ${failure(e).says}`);
			}
		});
	}
	var div = root_14$1();
	var node = child(div);
	var consequent_2 = ($$anchor) => {
		var fragment = root_3$6();
		var p = first_child(fragment);
		var text = only_child(p, true);
		var node_1 = sibling(p, 2);
		var consequent = ($$anchor) => {
			var section = root_1$7();
			var h3 = child(section);
			var node_2 = child(h3);
			Icon(node_2, {
				name: "alert",
				size: 14
			});
			var span = sibling(node_2, 2);
			var text_1 = only_child(span, true);
			reset(h3);
			var ul = sibling(h3, 2);
			each(ul, 20, () => $$props.d.counts.ticked_unsent, (t) => t, ($$anchor, t) => {
				var li = root$8();
				var text_2 = only_child(li, true);
				template_effect(() => set_text(text_2, t));
				append($$anchor, li);
			});
			reset(ul);
			reset(section);
			template_effect(() => set_text(text_1, $$props.d.counts.ticked_unsent.length));
			append($$anchor, section);
		};
		if_block(node_1, ($$render) => {
			if ($$props.d.counts.ticked_unsent?.length) $$render(consequent);
		});
		var node_3 = sibling(node_1, 2);
		var consequent_1 = ($$anchor) => {
			var section_1 = root_2$7();
			var h3_1 = child(section_1);
			var span_1 = sibling(child(h3_1));
			var text_3 = only_child(span_1, true);
			reset(h3_1);
			var ul_1 = sibling(h3_1, 2);
			each(ul_1, 20, () => $$props.d.counts.sent_unticked, (t) => t, ($$anchor, t) => {
				var li_1 = root$8();
				var text_4 = only_child(li_1, true);
				template_effect(() => set_text(text_4, t));
				append($$anchor, li_1);
			});
			reset(ul_1);
			reset(section_1);
			template_effect(() => set_text(text_3, $$props.d.counts.sent_unticked.length));
			append($$anchor, section_1);
		};
		if_block(node_3, ($$render) => {
			if ($$props.d.counts.sent_unticked?.length) $$render(consequent_1);
		});
		template_effect(($0) => set_text(text, $0), [() => taskLine($$props.d)]);
		append($$anchor, fragment);
	};
	var alternate_1 = ($$anchor) => {
		var p_1 = root_4$4();
		var node_4 = child(p_1);
		var consequent_3 = ($$anchor) => {
			Inline($$anchor, { get text() {
				return $$props.d.counts_says;
			} });
		};
		var alternate = ($$anchor) => {
			var text_5 = text();
			template_effect(() => set_text(text_5, $$props.d.spec ? `${$$props.d.spec} has no task file this change can read.` : "No specification is attached to this change, so there are no tasks to trace."));
			append($$anchor, text_5);
		};
		if_block(node_4, ($$render) => {
			if ($$props.d.counts_says) $$render(consequent_3);
			else $$render(alternate, -1);
		});
		reset(p_1);
		append($$anchor, p_1);
	};
	if_block(node, ($$render) => {
		if ($$props.d.counts) $$render(consequent_2);
		else $$render(alternate_1, -1);
	});
	var node_5 = sibling(node, 2);
	var consequent_6 = ($$anchor) => {
		var section_2 = root_8$3();
		var h3_2 = child(section_2);
		var node_6 = child(h3_2);
		Icon(node_6, {
			name: "alert",
			size: 14
		});
		next();
		reset(h3_2);
		var node_7 = sibling(h3_2, 2);
		each(node_7, 17, () => $$props.d.drifts ?? [], index, ($$anchor, x) => {
			var div_1 = root_5$4();
			var p_2 = child(div_1);
			var node_8 = child(p_2);
			Inline(node_8, { get text() {
				return get(x).says;
			} });
			reset(p_2);
			var button = sibling(p_2, 2);
			var button_1 = sibling(button, 2);
			reset(div_1);
			template_effect(() => {
				button.disabled = !!pending.busy;
				button_1.disabled = !!pending.busy;
			});
			delegated("click", button, () => decide("tell", get(x).run));
			delegated("click", button_1, () => decide("accept", get(x).run));
			append($$anchor, div_1);
		});
		var node_9 = sibling(node_7, 2);
		var consequent_4 = ($$anchor) => {
			var p_3 = root_6$3();
			var text_6 = only_child(p_3);
			template_effect(() => set_text(text_6, `${pending.busy ?? ""}… ${pending.elapsed ?? ""}`));
			append($$anchor, p_3);
		};
		var consequent_5 = ($$anchor) => {
			var p_4 = root_7$3();
			var node_10 = child(p_4);
			Inline(node_10, { get text() {
				return get(said);
			} });
			reset(p_4);
			append($$anchor, p_4);
		};
		if_block(node_9, ($$render) => {
			if (pending.busy) $$render(consequent_4);
			else if (get(said)) $$render(consequent_5, 1);
		});
		reset(section_2);
		append($$anchor, section_2);
	};
	if_block(node_5, ($$render) => {
		if (($$props.d.drifts ?? []).length > 0) $$render(consequent_6);
	});
	var node_11 = sibling(node_5, 2);
	var consequent_7 = ($$anchor) => {
		var section_3 = root_10$2();
		var table = sibling(child(section_3), 2);
		var tbody = sibling(child(table));
		each(tbody, 21, () => $$props.d.token_rows ?? [], (r) => r.token, ($$anchor, r) => {
			var tr = root_9$2();
			var td = child(tr);
			var text_7 = only_child(td, true);
			var td_1 = sibling(td);
			var text_8 = only_child(td_1, true);
			var td_2 = sibling(td_1);
			var text_9 = only_child(td_2, true);
			var td_3 = sibling(td_2);
			let classes;
			var text_10 = only_child(td_3, true);
			reset(tr);
			template_effect(() => {
				set_text(text_7, get(r).token);
				set_text(text_8, get(r).tasks);
				set_text(text_9, get(r).ticked);
				classes = set_class(td_3, 1, "svelte-6i9c4j", null, classes, { quiet: get(r).seen_by_pass === null });
				set_text(text_10, get(r).seen_by_pass === null ? "no gates declared" : get(r).seen_by_pass ?? "");
			});
			append($$anchor, tr);
		});
		reset(tbody);
		reset(table);
		reset(section_3);
		append($$anchor, section_3);
	};
	if_block(node_11, ($$render) => {
		if (($$props.d.token_rows ?? []).length > 0) $$render(consequent_7);
	});
	var section_4 = sibling(node_11, 2);
	var node_12 = sibling(child(section_4), 2);
	var consequent_8 = ($$anchor) => {
		{
			let $0 = user_derived(() => read.data !== null);
			Failed($$anchor, {
				what: "the run's own plan",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				get stale() {
					return get($0);
				}
			});
		}
	};
	if_block(node_12, ($$render) => {
		if (read.failure) $$render(consequent_8);
	});
	var node_13 = sibling(node_12, 2);
	var consequent_9 = ($$anchor) => {
		{
			let $0 = user_derived(() => get(sent).map((t) => ({
				text: t.text,
				path: t.path,
				line: t.line
			})));
			Plan($$anchor, {
				get tasks() {
					return get($0);
				},
				get steps() {
					return get(steps);
				}
			});
		}
	};
	var alternate_2 = ($$anchor) => {
		Skeleton($$anchor, { rows: 3 });
	};
	if_block(node_13, ($$render) => {
		if (get(planKnown)) $$render(consequent_9);
		else $$render(alternate_2, -1);
	});
	reset(section_4);
	var node_14 = sibling(section_4, 2);
	var consequent_10 = ($$anchor) => {
		var section_5 = root_12$2();
		var ul_2 = sibling(child(section_5), 2);
		each(ul_2, 21, () => $$props.d.run_rows ?? [], (r) => r.id, ($$anchor, r) => {
			var li_2 = root_11$2();
			var code = child(li_2);
			var text_11 = only_child(code, true);
			var node_15 = sibling(code, 2);
			Inline(node_15, { get text() {
				return get(r).sent_says;
			} });
			reset(li_2);
			template_effect(($0) => set_text(text_11, $0), [() => get(r).id.slice(0, 16)]);
			append($$anchor, li_2);
		});
		reset(ul_2);
		reset(section_5);
		append($$anchor, section_5);
	};
	if_block(node_14, ($$render) => {
		if (($$props.d.run_rows ?? []).length > 1) $$render(consequent_10);
	});
	var node_16 = sibling(node_14, 2);
	var consequent_11 = ($$anchor) => {
		var section_6 = root_13$2();
		var h2 = child(section_6);
		var text_12 = only_child(h2);
		var ol = sibling(h2, 2);
		each(ol, 21, () => $$props.d.plan.outline, index, ($$anchor, h) => {
			var li_3 = root$8();
			let styles;
			var text_13 = only_child(li_3, true);
			template_effect(($0) => {
				set_class(li_3, 1, `h${get(h).level ?? ""}`, "svelte-6i9c4j");
				styles = set_style(li_3, "", styles, { "padding-left": $0 });
				set_text(text_13, get(h).text);
			}, [() => `${Math.max(0, get(h).level - 1) * 1}rem`]);
			append($$anchor, li_3);
		});
		reset(ol);
		reset(section_6);
		template_effect(() => set_text(text_12, `Outline of ${$$props.d.plan.path ?? ""}`));
		append($$anchor, section_6);
	};
	if_block(node_16, ($$render) => {
		if ($$props.d.plan?.outline?.length) $$render(consequent_11);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/Files.svelte
var root$7 = from_html(`<span class="cov svelte-etvvkh"><!></span>`);
var root_1$6 = from_html(`<span> </span>`);
var root_2$6 = from_html(`<span class="moved svelte-etvvkh">changed since seen</span>`);
var root_3$5 = from_html(`<button class="file svelte-etvvkh"><!> <span class="name svelte-etvvkh"><span class="dir svelte-etvvkh"> </span><b class="svelte-etvvkh"> </b></span> <span class="delta svelte-etvvkh"><span class="add svelte-etvvkh"> </span> <span class="del svelte-etvvkh"> </span></span> <span class="meta svelte-etvvkh"><!> <!> <!></span></button>`);
var root_4$3 = from_html(`<button><!> <span><!></span> <span class="n svelte-etvvkh"> </span></button> <!>`, 1);
var root_5$3 = from_html(`<nav class="files svelte-etvvkh" aria-label="files in the change"></nav>`);
function Files($$anchor, $$props) {
	push($$props, true);
	let folded = state(proxy({}));
	const split = (p) => {
		const i = p.lastIndexOf("/");
		return i === -1 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
	};
	const statusIcon = (s) => s.startsWith("added") ? "plus" : s.startsWith("deleted") ? "x" : "file";
	var nav = root_5$3();
	each(nav, 21, () => $$props.sections, (s) => s.says, ($$anchor, s) => {
		var fragment = root_4$3();
		var button = first_child(fragment);
		var node = child(button);
		{
			let $0 = user_derived(() => get(folded)[get(s).says] ? "right" : "down");
			Icon(node, {
				get name() {
					return get($0);
				},
				size: 12
			});
		}
		var span = sibling(node, 2);
		var node_1 = child(span);
		Inline(node_1, { get text() {
			return get(s).says;
		} });
		reset(span);
		var span_1 = sibling(span, 2);
		var text = only_child(span_1, true);
		reset(button);
		var node_2 = sibling(button, 2);
		var consequent_3 = ($$anchor) => {
			var fragment_1 = comment();
			var node_3 = first_child(fragment_1);
			each(node_3, 17, () => get(s).files, (f) => get(s).says + f.path, ($$anchor, f) => {
				const computed_const = user_derived(() => {
					const [dir, base] = split(get(f).path);
					return {
						dir,
						base
					};
				});
				const left = user_derived(() => $$props.unreadOf(get(f)));
				var button_1 = root_3$5();
				var node_4 = child(button_1);
				{
					let $0 = user_derived(() => statusIcon(get(f).status_says));
					Icon(node_4, {
						get name() {
							return get($0);
						},
						size: 13
					});
				}
				var span_2 = sibling(node_4, 2);
				var span_3 = child(span_2);
				var text_1 = only_child(span_3, true);
				var b = sibling(span_3);
				var text_2 = only_child(b, true);
				reset(span_2);
				var span_4 = sibling(span_2, 2);
				var span_5 = child(span_4);
				var text_3 = only_child(span_5);
				var span_6 = sibling(span_5, 2);
				var text_4 = only_child(span_6);
				reset(span_4);
				var span_7 = sibling(span_4, 2);
				var node_5 = child(span_7);
				var consequent = ($$anchor) => {
					var span_8 = root$7();
					var node_6 = child(span_8);
					Icon(node_6, {
						name: "shield",
						size: 11
					});
					reset(span_8);
					template_effect(() => set_attribute(span_8, "title", get(f).coverage_says ?? ""));
					append($$anchor, span_8);
				};
				if_block(node_5, ($$render) => {
					if (get(f).coverage?.coverage === "covered") $$render(consequent);
				});
				var node_7 = sibling(node_5, 2);
				var consequent_1 = ($$anchor) => {
					var span_9 = root_1$6();
					let classes;
					var text_5 = only_child(span_9, true);
					template_effect(() => {
						classes = set_class(span_9, 1, "seen svelte-etvvkh", null, classes, { all: get(left) === 0 });
						set_text(text_5, get(left) === 0 ? "all seen" : `${get(left)} unread`);
					});
					append($$anchor, span_9);
				};
				if_block(node_7, ($$render) => {
					if (get(f).hunks.length || get(f).unit_seen != null) $$render(consequent_1);
				});
				var node_8 = sibling(node_7, 2);
				var consequent_2 = ($$anchor) => {
					var span_10 = root_2$6();
					append($$anchor, span_10);
				};
				if_block(node_8, ($$render) => {
					if (get(f).changed_since_seen) $$render(consequent_2);
				});
				reset(span_7);
				reset(button_1);
				template_effect(() => {
					set_attribute(button_1, "aria-current", get(f).path === $$props.selected ? "true" : void 0);
					set_attribute(button_1, "title", get(f).path);
					set_text(text_1, get(computed_const).dir);
					set_text(text_2, get(computed_const).base);
					set_text(text_3, `+${get(f).added ?? ""}`);
					set_text(text_4, `−${get(f).removed ?? ""}`);
				});
				delegated("click", button_1, () => $$props.pick(get(f).path));
				append($$anchor, button_1);
			});
			append($$anchor, fragment_1);
		};
		if_block(node_2, ($$render) => {
			if (!get(folded)[get(s).says]) $$render(consequent_3);
		});
		template_effect(() => {
			set_class(button, 1, `sec ${get(s).tone ?? "" ?? ""}`, "svelte-etvvkh");
			set_attribute(button, "aria-expanded", !get(folded)[get(s).says]);
			set_text(text, get(s).files.length);
		});
		delegated("click", button, () => set(folded, {
			...get(folded),
			[get(s).says]: !get(folded)[get(s).says]
		}, true));
		append($$anchor, fragment);
	});
	reset(nav);
	append($$anchor, nav);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/types.ts
var hunkKey = (h, i) => h.digest ?? `${i}:${h.header}`;
//#endregion
//#region src/surfaces/change/review/Diff.svelte
var root$6 = from_html(`; <!>`, 1);
var root_1$5 = from_html(`<tr class="note weak svelte-7nsom7"><td class="svelte-7nsom7"><!> <b> </b> — <!><!> </td></tr>`);
var root_2$5 = from_html(`<span class="moved svelte-7nsom7"> </span>`);
var root_3$4 = from_html(`<span class="outcome svelte-7nsom7"> </span>`);
var root_4$2 = from_html(`<button class="del-c svelte-7nsom7" aria-label="delete this comment">delete</button>`);
var root_5$2 = from_html(`<tr><td class="svelte-7nsom7"><span class="who svelte-7nsom7"><!> </span> <span class="body svelte-7nsom7"> </span> <!> <!> <!></td></tr>`);
var root_6$2 = from_html(`<tr class="note draft svelte-7nsom7"><td class="svelte-7nsom7"><!></td></tr>`);
var root_7$2 = from_html(`<!> <!> <!>`, 1);
var root_8$2 = from_html(`<span class="mark seen svelte-7nsom7" title="marked seen; kept by Devplane until this hunk changes"><!> seen</span>`);
var root_9$1 = from_html(`<button class="folded svelte-7nsom7"><!> </button>`);
var root_10$1 = from_html(`<tr><td class="no svelte-7nsom7" title="comment on this line"> </td><td class="no svelte-7nsom7" title="comment on this line"> </td><td class="sg svelte-7nsom7"> </td><td class="tx svelte-7nsom7"> </td></tr> <!>`, 1);
var root_11$1 = from_html(`<table class="code unified svelte-7nsom7"><tbody></tbody></table>`);
var root_12$1 = from_html(`<tr><td data-side="old"> </td><td> </td><td> </td><td data-side="new"> </td><td> </td><td> </td></tr> <!> <!>`, 1);
var root_13$1 = from_html(`<table class="code split svelte-7nsom7"><tbody></tbody></table>`);
var root_14 = from_html(`<section><button class="hh svelte-7nsom7"><code class="svelte-7nsom7"> </code> <!></button> <!></section>`);
function Diff($$anchor, $$props) {
	push($$props, true);
	const notes = ($$anchor, l = noop, span = noop) => {
		const w = user_derived(() => weakOn(l()));
		var fragment = root_7$2();
		var node = first_child(fragment);
		var consequent_1 = ($$anchor) => {
			var tr = root_1$5();
			var td = child(tr);
			var node_1 = child(td);
			Icon(node_1, {
				name: "alert",
				size: 12
			});
			var b = sibling(node_1, 2);
			var text_1 = only_child(b);
			var node_2 = sibling(b, 2);
			Inline(node_2, { get text() {
				return get(w).why;
			} });
			var node_3 = sibling(node_2);
			var consequent = ($$anchor) => {
				var fragment_1 = root$6();
				var node_4 = sibling(first_child(fragment_1));
				Inline(node_4, { get text() {
					return get(w).gate_says;
				} });
				append($$anchor, fragment_1);
			};
			if_block(node_3, ($$render) => {
				if (get(w).gate_says) $$render(consequent);
			});
			var text_2 = sibling(node_3, 1, true);
			reset(td);
			reset(tr);
			template_effect(() => {
				set_attribute(td, "colspan", span());
				set_text(text_1, `${weakKindSays[get(w).kind] ?? ""} here`);
				set_text(text_2, get(w).seen ? " · read" : " · not yet read");
			});
			append($$anchor, tr);
		};
		if_block(node, ($$render) => {
			if (get(w)) $$render(consequent_1);
		});
		var node_5 = sibling(node, 2);
		each(node_5, 17, () => under(l()), (c) => c.id, ($$anchor, c) => {
			var tr_1 = root_5$2();
			let classes;
			var td_1 = child(tr_1);
			var span_1 = child(td_1);
			var node_6 = child(span_1);
			Icon(node_6, {
				name: "person",
				size: 12
			});
			var text_3 = sibling(node_6);
			reset(span_1);
			var span_2 = sibling(span_1, 2);
			var text_4 = only_child(span_2, true);
			var node_7 = sibling(span_2, 2);
			var consequent_2 = ($$anchor) => {
				var span_3 = root_2$5();
				var text_5 = only_child(span_3, true);
				template_effect(($0) => set_text(text_5, $0), [() => when(get(c))]);
				append($$anchor, span_3);
			};
			var d = user_derived(() => when(get(c)));
			if_block(node_7, ($$render) => {
				if (get(d)) $$render(consequent_2);
			});
			var node_8 = sibling(node_7, 2);
			var consequent_3 = ($$anchor) => {
				var span_4 = root_3$4();
				var text_6 = only_child(span_4, true);
				template_effect(() => set_text(text_6, get(c).hunk_changed ? "the agent changed this hunk since" : "not changed since"));
				append($$anchor, span_4);
			};
			if_block(node_8, ($$render) => {
				if (get(c).hunk_changed !== null && get(c).hunk_changed !== void 0) $$render(consequent_3);
			});
			var node_9 = sibling(node_8, 2);
			var consequent_4 = ($$anchor) => {
				var button = root_4$2();
				delegated("click", button, () => $$props.ondelete?.(get(c).id));
				append($$anchor, button);
			};
			if_block(node_9, ($$render) => {
				if (!get(c).sent_at && $$props.ondelete) $$render(consequent_4);
			});
			reset(td_1);
			reset(tr_1);
			template_effect(() => {
				classes = set_class(tr_1, 1, "note comment svelte-7nsom7", null, classes, { sent: !!get(c).sent_at });
				set_attribute(td_1, "colspan", span());
				set_text(text_3, ` ${get(c).sent_at ? "sent" : "pending"}`);
				set_text(text_4, get(c).body);
			});
			append($$anchor, tr_1);
		});
		var node_10 = sibling(node_5, 2);
		var consequent_5 = ($$anchor) => {
			var tr_2 = root_6$2();
			var td_2 = child(tr_2);
			var node_11 = child(td_2);
			snippet(node_11, () => $$props.composer);
			reset(td_2);
			reset(tr_2);
			template_effect(() => set_attribute(td_2, "colspan", span()));
			append($$anchor, tr_2);
		};
		var d_1 = user_derived(() => isDrafting(l()) && $$props.composer);
		if_block(node_10, ($$render) => {
			if (get(d_1)) $$render(consequent_5);
		});
		append($$anchor, fragment);
	};
	let mode = prop($$props, "mode", 3, "unified"), current = prop($$props, "current", 19, () => -1), expanded = prop($$props, "expanded", 3, false), at = prop($$props, "at", 3, 0), seen = prop($$props, "seen", 3, () => false), weak = prop($$props, "weak", 19, () => []), comments = prop($$props, "comments", 19, () => []), drafting = prop($$props, "drafting", 3, null);
	function numbered(h) {
		const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(h.header);
		let o = m ? Number(m[1]) : 0;
		let n = m ? Number(m[2]) : 0;
		return h.lines.map(([kind, text]) => {
			if (kind === "added") return {
				kind,
				text,
				old: null,
				new: n++
			};
			if (kind === "removed") return {
				kind,
				text,
				old: o++,
				new: null
			};
			return {
				kind,
				text,
				old: o++,
				new: n++
			};
		});
	}
	function paired(lines) {
		const out = [];
		let i = 0;
		while (i < lines.length) {
			const l = lines[i];
			if (l.kind === "context") {
				out.push({
					left: l,
					right: l
				});
				i++;
				continue;
			}
			const del = [];
			const add = [];
			while (i < lines.length && lines[i].kind === "removed") del.push(lines[i++]);
			while (i < lines.length && lines[i].kind === "added") add.push(lines[i++]);
			for (let k = 0; k < Math.max(del.length, add.length); k++) out.push({
				left: del[k] ?? null,
				right: add[k] ?? null
			});
		}
		return out;
	}
	const sign = (k) => k === "added" ? "+" : k === "removed" ? "−" : " ";
	const place = (l) => l.kind === "removed" ? {
		side: "old",
		line: l.old ?? 0
	} : {
		side: "new",
		line: l.new ?? 0
	};
	const weakOn = (l) => l && l.kind !== "context" ? weak().find((w) => w.matched === l.text.trim()) : void 0;
	const under = (l) => {
		if (!l) return [];
		const p = place(l);
		return comments().filter((c) => c.side === p.side && (c.anchor.is === "moved" ? c.anchor.line : c.line) === p.line && c.anchor.is !== "changed");
	};
	const isDrafting = (l) => !!l && !!drafting() && drafting().side === place(l).side && drafting().line === place(l).line;
	function pressed(e) {
		const cell = e.target.closest("td.no[data-line]");
		if (!cell || !$$props.oncomment) return;
		$$props.oncomment(cell.dataset.side === "old" ? "old" : "new", Number(cell.dataset.line));
	}
	const when = (c) => c.anchor.is === "changed" ? `changed — written against: ${c.line_text}` : c.anchor.is === "moved" ? `moved from line ${c.line}` : "";
	var fragment_2 = comment();
	var node_12 = first_child(fragment_2);
	each(node_12, 19, () => $$props.hunks, (h, i) => hunkKey(h, i), ($$anchor, h, i) => {
		const m = user_derived(() => seen()(get(h)));
		var section = root_14();
		let classes_1;
		var button_1 = child(section);
		var code = child(button_1);
		var text_7 = only_child(code, true);
		var node_13 = sibling(code, 2);
		var consequent_6 = ($$anchor) => {
			var span_5 = root_8$2();
			var node_14 = child(span_5);
			Icon(node_14, {
				name: "eye",
				size: 12
			});
			next();
			reset(span_5);
			append($$anchor, span_5);
		};
		if_block(node_13, ($$render) => {
			if (get(m)) $$render(consequent_6);
		});
		reset(button_1);
		var node_15 = sibling(button_1, 2);
		var consequent_7 = ($$anchor) => {
			var button_2 = root_9$1();
			var node_16 = child(button_2);
			Icon(node_16, {
				name: "right",
				size: 12
			});
			var text_8 = sibling(node_16);
			reset(button_2);
			template_effect(() => set_text(text_8, ` Formatting only — ${get(h).lines.length ?? ""} lines whose text is unchanged apart from whitespace. Show them.`));
			delegated("click", button_2, function(...$$args) {
				$$props.onexpand?.apply(this, $$args);
			});
			append($$anchor, button_2);
		};
		var consequent_8 = ($$anchor) => {
			var table = root_11$1();
			var tbody = child(table);
			each(tbody, 21, () => numbered(get(h)), index, ($$anchor, l) => {
				const p = user_derived(() => place(get(l)));
				var fragment_3 = root_10$1();
				var tr_3 = first_child(fragment_3);
				let classes_2;
				var td_3 = child(tr_3);
				var text_9 = only_child(td_3, true);
				var td_4 = sibling(td_3);
				var text_10 = only_child(td_4, true);
				var td_5 = sibling(td_4);
				var text_11 = only_child(td_5, true);
				var td_6 = sibling(td_5);
				var text_12 = only_child(td_6, true);
				reset(tr_3);
				var node_17 = sibling(tr_3, 2);
				notes(node_17, () => get(l), () => 4);
				template_effect(($0, $1) => {
					classes_2 = set_class(tr_3, 1, clsx(get(l).kind), "svelte-7nsom7", classes_2, { weakline: $0 });
					set_attribute(td_3, "data-side", get(p).side);
					set_attribute(td_3, "data-line", get(p).line);
					set_text(text_9, get(l).old ?? "");
					set_attribute(td_4, "data-side", get(p).side);
					set_attribute(td_4, "data-line", get(p).line);
					set_text(text_10, get(l).new ?? "");
					set_text(text_11, $1);
					set_text(text_12, get(l).text);
				}, [() => !!weakOn(get(l)), () => weakOn(get(l)) ? "⚠" : sign(get(l).kind)]);
				append($$anchor, fragment_3);
			});
			reset(tbody);
			reset(table);
			delegated("click", table, pressed);
			append($$anchor, table);
		};
		var alternate = ($$anchor) => {
			var table_1 = root_13$1();
			var tbody_1 = child(table_1);
			each(tbody_1, 21, () => paired(numbered(get(h))), index, ($$anchor, pr) => {
				var fragment_4 = root_12$1();
				var tr_4 = first_child(fragment_4);
				let classes_3;
				var td_7 = child(tr_4);
				var text_13 = only_child(td_7, true);
				var td_8 = sibling(td_7);
				var text_14 = only_child(td_8, true);
				var td_9 = sibling(td_8);
				var text_15 = only_child(td_9, true);
				var td_10 = sibling(td_9);
				var text_16 = only_child(td_10, true);
				var td_11 = sibling(td_10);
				var text_17 = only_child(td_11, true);
				var td_12 = sibling(td_11);
				var text_18 = only_child(td_12, true);
				reset(tr_4);
				var node_18 = sibling(tr_4, 2);
				notes(node_18, () => get(pr).right ?? get(pr).left, () => 6);
				var node_19 = sibling(node_18, 2);
				var consequent_9 = ($$anchor) => {
					notes($$anchor, () => get(pr).left, () => 6);
				};
				if_block(node_19, ($$render) => {
					if (get(pr).left && get(pr).right && get(pr).left !== get(pr).right) $$render(consequent_9);
				});
				template_effect(($0, $1, $2) => {
					classes_3 = set_class(tr_4, 1, "svelte-7nsom7", null, classes_3, { weakline: $0 });
					set_class(td_7, 1, `no ${get(pr).left?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_attribute(td_7, "data-line", get(pr).left?.old ?? "");
					set_text(text_13, get(pr).left?.old ?? "");
					set_class(td_8, 1, `sg ${get(pr).left?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_14, $1);
					set_class(td_9, 1, `tx ${get(pr).left?.kind === "removed" ? "removed" : get(pr).left ? "context" : "none"}`, "svelte-7nsom7");
					set_text(text_15, get(pr).left?.text ?? "");
					set_class(td_10, 1, `no ${get(pr).right?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_attribute(td_10, "data-line", get(pr).right?.new ?? "");
					set_text(text_16, get(pr).right?.new ?? "");
					set_class(td_11, 1, `sg ${get(pr).right?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_17, $2);
					set_class(td_12, 1, `tx ${get(pr).right?.kind === "added" ? "added" : get(pr).right ? "context" : "none"}`, "svelte-7nsom7");
					set_text(text_18, get(pr).right?.text ?? "");
				}, [
					() => !!weakOn(get(pr).left) || !!weakOn(get(pr).right),
					() => get(pr).left ? weakOn(get(pr).left) ? "⚠" : sign(get(pr).left.kind) : "",
					() => get(pr).right ? weakOn(get(pr).right) ? "⚠" : sign(get(pr).right.kind) : ""
				]);
				append($$anchor, fragment_4);
			});
			reset(tbody_1);
			reset(table_1);
			delegated("click", table_1, pressed);
			append($$anchor, table_1);
		};
		if_block(node_15, ($$render) => {
			if (get(h).formatter_only && !expanded()) $$render(consequent_7);
			else if (mode() === "unified") $$render(consequent_8, 1);
			else $$render(alternate, -1);
		});
		reset(section);
		template_effect(() => {
			classes_1 = set_class(section, 1, "hunk svelte-7nsom7", null, classes_1, { current: get(i) === current() });
			set_attribute(section, "id", `hunk-${at() + get(i)}`);
			set_attribute(section, "data-hunk", at() + get(i));
			set_text(text_7, get(h).header);
		});
		delegated("click", button_1, () => $$props.onpick?.(get(i)));
		append($$anchor, section);
	});
	append($$anchor, fragment_2);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/FileSection.svelte
var root$5 = from_html(`<span class="weakw svelte-oq8s6f"><!> </span>`);
var root_1$4 = from_html(`<span class="moved svelte-oq8s6f">changed since you marked it</span>`);
var root_2$4 = from_html(`<button class="read svelte-oq8s6f"><!> I have read this</button>`);
var root_3$3 = from_html(`<dt class="svelte-oq8s6f">Asked for</dt><dd><!></dd>`, 1);
var root_4$1 = from_html(`<dt class="svelte-oq8s6f">Decided while writing</dt><dd class="svelte-oq8s6f"> </dd>`, 1);
var root_5$1 = from_html(`; <!>`, 1);
var root_6$1 = from_html(`<dt class="wait svelte-oq8s6f"> </dt><dd class="svelte-oq8s6f"><!><!></dd>`, 1);
var root_7$1 = from_html(`<code class="svelte-oq8s6f"> </code>`);
var root_8$1 = from_html(`<dt class="svelte-oq8s6f">Check it yourself</dt><dd class="cmds svelte-oq8s6f"></dd>`, 1);
var root_9 = from_html(`<p class="finding svelte-oq8s6f"><!></p>`);
var root_10 = from_html(`<p class="adrift svelte-oq8s6f"><b> </b> <code class="svelte-oq8s6f"> </code> </p>`);
var root_11 = from_html(`<button class="huge svelte-oq8s6f"> <kbd>x</kbd></button>`);
var root_12 = from_html(`<div class="hunks"><!></div>`);
var root_13 = from_html(`<section class="file svelte-oq8s6f"><header><code class="path svelte-oq8s6f"> </code> <span class="add svelte-oq8s6f"> </span><span class="del svelte-oq8s6f"> </span> <!> <!> <span class="facts svelte-oq8s6f"> </span> <span class="unread svelte-oq8s6f"> </span> <!> <details class="check svelte-oq8s6f"><summary class="svelte-oq8s6f">facts</summary> <dl class="card svelte-oq8s6f"><dt class="svelte-oq8s6f">Status</dt><dd class="svelte-oq8s6f"><!></dd> <dt class="svelte-oq8s6f">Role</dt><dd class="svelte-oq8s6f"><!></dd> <dt class="svelte-oq8s6f">Covered by</dt><dd><!></dd> <!> <!> <!> <!></dl></details></header> <!> <!> <!></section>`);
function FileSection($$anchor, $$props) {
	push($$props, true);
	let weak = prop($$props, "weak", 19, () => []), comments = prop($$props, "comments", 19, () => []), drafting = prop($$props, "drafting", 3, null);
	const unread = user_derived(() => unreadIn($$props.change, $$props.file));
	const unreadWeak = user_derived(() => weak().filter((w) => !w.seen));
	const lines = user_derived(() => $$props.file.hunks.reduce((n, h) => n + h.lines.length + 2, 0));
	const adrift = user_derived(() => comments().filter((c) => c.anchor.is === "changed"));
	const covered = user_derived(() => $$props.file.coverage?.coverage === "covered");
	const huge = user_derived(() => $$props.file.added + $$props.file.removed > 1e3);
	const facts = user_derived(() => [
		$$props.file.status_says,
		$$props.file.role_says,
		$$props.file.coverage_says ?? "no mapping declared",
		$$props.file.task_says,
		$$props.file.decisions.length ? `${$$props.file.decisions.length} ${$$props.file.decisions.length === 1 ? "decision" : "decisions"} while writing` : ""
	].filter(Boolean).map((x) => x.replace(/`/g, "")).join(" · "));
	var section = root_13();
	var header = child(section);
	let classes;
	var code = child(header);
	var text = only_child(code, true);
	var span = sibling(code, 2);
	var text_1 = only_child(span);
	var span_1 = sibling(span);
	var text_2 = only_child(span_1);
	var node = sibling(span_1, 2);
	var consequent = ($$anchor) => {
		var span_2 = root$5();
		var node_1 = child(span_2);
		Icon(node_1, {
			name: "alert",
			size: 12
		});
		var text_3 = sibling(node_1);
		reset(span_2);
		template_effect(() => set_text(text_3, ` check weakened${get(unreadWeak).length ? "" : " · read"}`));
		append($$anchor, span_2);
	};
	if_block(node, ($$render) => {
		if (weak().length) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var span_3 = root_1$4();
		append($$anchor, span_3);
	};
	if_block(node_2, ($$render) => {
		if ($$props.file.changed_since_seen) $$render(consequent_1);
	});
	var span_4 = sibling(node_2, 2);
	var text_4 = only_child(span_4, true);
	var span_5 = sibling(span_4, 2);
	var text_5 = only_child(span_5, true);
	var node_3 = sibling(span_5, 2);
	var consequent_2 = ($$anchor) => {
		var button = root_2$4();
		var node_4 = child(button);
		Icon(node_4, {
			name: "eye",
			size: 12
		});
		next();
		reset(button);
		delegated("click", button, function(...$$args) {
			$$props.readRows?.apply(this, $$args);
		});
		append($$anchor, button);
	};
	var d = user_derived(() => get(unreadWeak).length || !$$props.file.hunks.length && $$props.file.unit_seen === false && !unitSeen($$props.change, $$props.file));
	if_block(node_3, ($$render) => {
		if (get(d)) $$render(consequent_2);
	});
	var details = sibling(node_3, 2);
	var dl = sibling(child(details), 2);
	var dd = sibling(child(dl));
	var node_5 = child(dd);
	Inline(node_5, { get text() {
		return $$props.file.status_says;
	} });
	reset(dd);
	var dd_1 = sibling(dd, 3);
	var node_6 = child(dd_1);
	Inline(node_6, { get text() {
		return $$props.file.role_says;
	} });
	reset(dd_1);
	var dd_2 = sibling(dd_1, 3);
	let classes_1;
	var node_7 = child(dd_2);
	{
		let $0 = user_derived(() => $$props.file.coverage_says ?? "no mapping declared");
		Inline(node_7, { get text() {
			return get($0);
		} });
	}
	reset(dd_2);
	var node_8 = sibling(dd_2, 2);
	var consequent_3 = ($$anchor) => {
		var fragment = root_3$3();
		var dd_3 = sibling(first_child(fragment));
		let classes_2;
		var node_9 = child(dd_3);
		Inline(node_9, { get text() {
			return $$props.file.task_says;
		} });
		reset(dd_3);
		template_effect(($0) => classes_2 = set_class(dd_3, 1, "svelte-oq8s6f", null, classes_2, { wait: $0 }), [() => $$props.file.task_says.startsWith("not asked")]);
		append($$anchor, fragment);
	};
	if_block(node_8, ($$render) => {
		if ($$props.file.task_says) $$render(consequent_3);
	});
	var node_10 = sibling(node_8, 2);
	var consequent_4 = ($$anchor) => {
		var fragment_1 = root_4$1();
		var dd_4 = sibling(first_child(fragment_1));
		var text_6 = only_child(dd_4, true);
		template_effect(($0) => set_text(text_6, $0), [() => $$props.file.decisions.map((x) => x.says).join(" · ")]);
		append($$anchor, fragment_1);
	};
	if_block(node_10, ($$render) => {
		if ($$props.file.decisions.length) $$render(consequent_4);
	});
	var node_11 = sibling(node_10, 2);
	each(node_11, 17, weak, (w) => w.why + w.matched, ($$anchor, w) => {
		var fragment_2 = root_6$1();
		var dt = first_child(fragment_2);
		var text_7 = only_child(dt, true);
		var dd_5 = sibling(dt);
		var node_12 = child(dd_5);
		Inline(node_12, { get text() {
			return get(w).why;
		} });
		var node_13 = sibling(node_12);
		var consequent_5 = ($$anchor) => {
			var fragment_3 = root_5$1();
			var node_14 = sibling(first_child(fragment_3));
			Inline(node_14, { get text() {
				return get(w).gate_says;
			} });
			append($$anchor, fragment_3);
		};
		if_block(node_13, ($$render) => {
			if (get(w).gate_says) $$render(consequent_5);
		});
		reset(dd_5);
		template_effect(($0) => set_text(text_7, $0), [() => weakKindLabel(get(w).kind)]);
		append($$anchor, fragment_2);
	});
	var node_15 = sibling(node_11, 2);
	var consequent_6 = ($$anchor) => {
		var fragment_4 = root_8$1();
		var dd_6 = sibling(first_child(fragment_4));
		each(dd_6, 20, () => $$props.file.marker_commands, (c) => c, ($$anchor, c) => {
			var code_1 = root_7$1();
			var text_8 = only_child(code_1, true);
			template_effect(() => set_text(text_8, c));
			append($$anchor, code_1);
		});
		reset(dd_6);
		append($$anchor, fragment_4);
	};
	if_block(node_15, ($$render) => {
		if ($$props.file.marker_commands.length) $$render(consequent_6);
	});
	reset(dl);
	reset(details);
	reset(header);
	var node_16 = sibling(header, 2);
	var consequent_7 = ($$anchor) => {
		var p = root_9();
		var node_17 = child(p);
		Inline(node_17, { get text() {
			return $$props.file.body_says;
		} });
		reset(p);
		append($$anchor, p);
	};
	if_block(node_16, ($$render) => {
		if ($$props.file.body_says) $$render(consequent_7);
	});
	var node_18 = sibling(node_16, 2);
	each(node_18, 17, () => get(adrift), (c) => c.id, ($$anchor, c) => {
		var p_1 = root_10();
		var b = child(p_1);
		var text_9 = only_child(b, true);
		var text_10 = sibling(b);
		var code_2 = sibling(text_10);
		var text_11 = only_child(code_2, true);
		var text_12 = sibling(code_2);
		reset(p_1);
		template_effect(() => {
			set_text(text_9, get(c).sent_at ? "sent" : "pending");
			set_text(text_10, ` on line ${get(c).line ?? ""} — the line changed; written against: `);
			set_text(text_11, get(c).line_text);
			set_text(text_12, ` — ${get(c).body ?? ""}`);
		});
		append($$anchor, p_1);
	});
	var node_19 = sibling(node_18, 2);
	var consequent_8 = ($$anchor) => {
		var button_1 = root_11();
		var text_13 = child(button_1);
		next();
		reset(button_1);
		template_effect(() => set_text(text_13, `${$$props.file.added + $$props.file.removed} changed lines in ${$$props.file.hunks.length ?? ""} ${$$props.file.hunks.length === 1 ? "hunk" : "hunks"} — collapsed. Show them `));
		delegated("click", button_1, function(...$$args) {
			$$props.expand?.apply(this, $$args);
		});
		append($$anchor, button_1);
	};
	var alternate = ($$anchor) => {
		var div = root_12();
		var node_20 = child(div);
		{
			let $0 = user_derived(() => $$props.current - $$props.at);
			Diff(node_20, {
				get hunks() {
					return $$props.file.hunks;
				},
				get mode() {
					return $$props.mode;
				},
				get at() {
					return $$props.at;
				},
				get current() {
					return get($0);
				},
				get expanded() {
					return $$props.expanded;
				},
				seen: (h) => isSeen($$props.change, $$props.file.path, h),
				get weak() {
					return weak();
				},
				get comments() {
					return comments();
				},
				get drafting() {
					return drafting();
				},
				get composer() {
					return $$props.composer;
				},
				get onexpand() {
					return $$props.expand;
				},
				onpick: (i) => $$props.pick($$props.at + i),
				get oncomment() {
					return $$props.comment;
				},
				get ondelete() {
					return $$props.remove;
				}
			});
		}
		reset(div);
		append($$anchor, div);
	};
	if_block(node_19, ($$render) => {
		if (get(huge) && !$$props.expanded) $$render(consequent_8);
		else $$render(alternate, -1);
	});
	reset(section);
	template_effect(() => {
		set_attribute(section, "id", `file-${$$props.at ?? ""}`);
		set_attribute(section, "data-path", $$props.file.path);
		set_style(section, `contain-intrinsic-size: auto ${get(huge) && !$$props.expanded ? 90 : get(lines) * 19 + 48}px`);
		classes = set_class(header, 1, "strip svelte-oq8s6f", null, classes, { weak: get(unreadWeak).length > 0 });
		set_attribute(code, "title", $$props.file.path);
		set_text(text, $$props.file.path);
		set_text(text_1, `+${$$props.file.added ?? ""}`);
		set_text(text_2, `−${$$props.file.removed ?? ""}`);
		set_attribute(span_4, "title", get(facts));
		set_text(text_4, get(facts));
		set_text(text_5, get(unread) ? `${get(unread)} unread` : $$props.file.hunks.length || $$props.file.unit_seen != null ? "all seen" : "");
		classes_1 = set_class(dd_2, 1, "svelte-oq8s6f", null, classes_1, { quiet: !get(covered) });
	});
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/Comments.svelte
var root$4 = from_html(`<li class="svelte-1obkae4"><button class="to svelte-1obkae4"><code class="svelte-1obkae4"> </code> </button> <button class="x svelte-1obkae4"><!></button></li>`);
var root_1$3 = from_html(`<div class="pending svelte-1obkae4" role="group" aria-label="the pending review"><details class="list svelte-1obkae4"><summary class="svelte-1obkae4"> </summary> <ol class="svelte-1obkae4"></ol></details> <button class="primary svelte-1obkae4" title="every pending comment, as one turn to the agent"> <kbd class="svelte-1obkae4"> </kbd></button></div>`);
var root_2$3 = from_html(`<p class="said svelte-1obkae4" role="status"><!></p>`);
var root_3$2 = from_html(`<!> <!>`, 1);
function Comments($$anchor, $$props) {
	push($$props, true);
	let busy = prop($$props, "busy", 3, ""), said = prop($$props, "said", 3, "");
	const pending = user_derived(() => $$props.comments.filter((c) => !c.sent_at));
	const n = user_derived(() => get(pending).length);
	var fragment = root_3$2();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var div = root_1$3();
		var details = child(div);
		var summary = child(details);
		var text = only_child(summary, true);
		var ol = sibling(summary, 2);
		each(ol, 21, () => get(pending), (c) => c.id, ($$anchor, c) => {
			var li = root$4();
			var button = child(li);
			var code = child(button);
			var text_1 = only_child(code);
			var text_2 = sibling(code);
			reset(button);
			var button_1 = sibling(button, 2);
			var node_1 = child(button_1);
			Icon(node_1, {
				name: "x",
				size: 11
			});
			reset(button_1);
			reset(li);
			template_effect(() => {
				set_text(text_1, `${get(c).path ?? ""}:${get(c).line ?? ""}`);
				set_text(text_2, ` ${get(c).body ?? ""}`);
				set_attribute(button_1, "aria-label", `delete the comment on ${get(c).path ?? ""} line ${get(c).line ?? ""}`);
			});
			delegated("click", button, () => $$props.jump(get(c)));
			delegated("click", button_1, () => $$props.remove(get(c).id));
			append($$anchor, li);
		});
		reset(ol);
		reset(details);
		var button_2 = sibling(details, 2);
		var text_3 = child(button_2);
		var kbd = sibling(text_3);
		var text_4 = only_child(kbd, true);
		reset(button_2);
		reset(div);
		template_effect(($0) => {
			set_text(text, get(n) === 1 ? "1 comment pending" : `${get(n)} comments pending`);
			button_2.disabled = !!busy();
			set_text(text_3, `${busy() ? `${busy()}…` : get(n) === 1 ? "Send 1 comment" : `Send ${get(n)} comments`} `);
			set_text(text_4, $0);
		}, [() => spell("Mod+Enter")]);
		delegated("click", button_2, function(...$$args) {
			$$props.send?.apply(this, $$args);
		});
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (get(n)) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	var consequent_1 = ($$anchor) => {
		var p = root_2$3();
		var node_3 = child(p);
		Inline(node_3, { get text() {
			return said();
		} });
		reset(p);
		append($$anchor, p);
	};
	if_block(node_2, ($$render) => {
		if (said()) $$render(consequent_1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/Composer.svelte
var root$3 = from_html(`<form class="compose svelte-1x1x9bo"><textarea rows="2" aria-label="a comment on this line, for the pending review" class="svelte-1x1x9bo"></textarea> <div class="svelte-1x1x9bo"><button type="submit" class="primary svelte-1x1x9bo">Add to the review <kbd class="svelte-1x1x9bo"> </kbd></button> <button type="button" class="svelte-1x1x9bo">Cancel <kbd class="svelte-1x1x9bo">Esc</kbd></button> <span class="hint svelte-1x1x9bo">nothing is sent until you send the review</span></div></form>`);
function Composer($$anchor, $$props) {
	push($$props, true);
	let busy = prop($$props, "busy", 3, false);
	let draft = state("");
	let field = state(null);
	user_effect(() => {
		get(field)?.focus();
	});
	function key(e) {
		if (e.key === "Escape") {
			e.preventDefault();
			e.stopPropagation();
			$$props.cancel();
		} else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			e.stopPropagation();
			if (get(draft).trim()) $$props.save(get(draft).trim());
		}
	}
	var form = root$3();
	var textarea = child(form);
	remove_textarea_child(textarea);
	bind_this(textarea, ($$value) => set(field, $$value), () => get(field));
	var div = sibling(textarea, 2);
	var button = child(div);
	var kbd = sibling(child(button));
	var text = only_child(kbd, true);
	reset(button);
	var button_1 = sibling(button, 2);
	next(2);
	reset(div);
	reset(form);
	template_effect(($0, $1) => {
		button.disabled = $0;
		set_text(text, $1);
	}, [() => !get(draft).trim() || busy(), () => spell("Mod+Enter")]);
	event("submit", form, (e) => {
		e.preventDefault();
		if (get(draft).trim()) $$props.save(get(draft).trim());
	});
	delegated("keydown", textarea, key);
	bind_value(textarea, () => get(draft), ($$value) => set(draft, $$value));
	delegated("click", button_1, function(...$$args) {
		$$props.cancel?.apply(this, $$args);
	});
	append($$anchor, form);
	pop();
}
delegate(["keydown", "click"]);
//#endregion
//#region src/surfaces/change/review/Pane.svelte
var root$2 = from_html(`<span class="progress svelte-15l7e6g"> </span>`);
var root_1$2 = from_html(`<p class="said svelte-15l7e6g" role="status"><!></p>`);
var root_2$2 = from_html(`<!><!>`, 1);
var root_3$1 = from_html(`<p class="finding svelte-15l7e6g"><!> <!></p>`);
var root_4 = from_html(`<div class="tree svelte-15l7e6g"><!></div>`);
var root_5 = from_html(`<h3><!> <span class="n svelte-15l7e6g"> </span></h3>`);
var root_6 = from_html(`<!> <!>`, 1);
var root_7 = from_html(`<div class="column svelte-15l7e6g"></div>`);
var root_8 = from_html(`<div class="review svelte-15l7e6g"><header class="bar svelte-15l7e6g"><span class="base">against <code class="svelte-15l7e6g"> </code></span> <span class="shape svelte-15l7e6g"> </span> <!> <span class="gap svelte-15l7e6g"></span> <!> <div class="seg svelte-15l7e6g" role="group" aria-label="order"><button>By risk</button> <button>By intent</button></div> <div class="seg svelte-15l7e6g" role="group" aria-label="diff layout"><button title="unified" aria-label="unified"><!></button> <button title="side by side" aria-label="side by side"><!></button></div></header> <!> <!> <!> <div class="body svelte-15l7e6g"><!></div></div>`);
function Pane($$anchor, $$props) {
	push($$props, true);
	const composer = ($$anchor) => {
		{
			let $0 = user_derived(() => !!get(busy));
			Composer($$anchor, {
				get busy() {
					return get($0);
				},
				save: (body) => void saveComment(body),
				cancel: () => set(drafting, null)
			});
		}
	};
	let planted = prop($$props, "review", 3, null), focus = prop($$props, "focus", 3, ""), source = prop($$props, "source", 3, null), counted = prop($$props, "counted", 3, false), onaddress = prop($$props, "onaddress", 3, null);
	const key = user_derived(() => $$props.id);
	const own = resource(() => get(key) && !source() ? `/api/changes/${encodeURIComponent(get(key))}/review` : null, { tell: () => `devplane change review ${get(key)}` });
	const read = user_derived(() => source() ?? own);
	const r = user_derived(() => get(read).data ?? planted());
	const change = user_derived(() => get(r)?.change ?? get(key));
	const unreadNow = user_derived(() => counted() ? "" : unreadSays(unread(get(r))));
	function pref(k, dflt) {
		try {
			return localStorage.getItem(k) || dflt;
		} catch {
			return dflt;
		}
	}
	let by = state(proxy(pref("vp-review-by", "risk")));
	let mode = state(proxy(pref("vp-review-mode", "unified")));
	user_effect(() => {
		try {
			localStorage.setItem("vp-review-by", get(by));
			localStorage.setItem("vp-review-mode", get(mode));
		} catch {}
	});
	const files = user_derived(() => (get(r)?.groups ?? []).flatMap((g) => g.files));
	const findings = user_derived(() => [
		get(r)?.unordered,
		get(r)?.coverage_absent,
		get(r)?.truncated_says,
		get(by) === "intent" ? get(r)?.intent.unavailable : null
	].filter((f) => !!f));
	const byPath = user_derived(() => new Map(get(files).map((f) => [f.path, f])));
	const sections = user_derived(() => {
		if (!get(r)) return [];
		if (get(by) === "risk" || get(r).intent.unavailable) return get(r).groups.map((g) => ({
			says: g.says,
			files: g.files,
			tone: g.weakened ? "fail" : void 0
		}));
		const out = get(r).intent.groups.map((g) => ({
			says: g.title,
			files: g.files.map((p) => get(byPath).get(p)).filter((f) => !!f)
		}));
		if (get(r).intent.not_asked_for.length) out.push({
			says: get(r).intent.not_asked_for_heading,
			files: get(r).intent.not_asked_for.map((n) => get(byPath).get(n.path)).filter((f) => !!f),
			tone: "wait"
		});
		return out;
	});
	const placed = user_derived(() => {
		const seen = /* @__PURE__ */ new Set();
		let n = 0;
		return get(sections).map((s) => ({
			...s,
			files: s.files.filter((f) => !seen.has(f.path) && (seen.add(f.path), true)).map((f) => {
				const at = n;
				n += f.hunks.length;
				return {
					file: f,
					at
				};
			})
		}));
	});
	const flat = user_derived(() => get(placed).flatMap((s) => s.files.flatMap(({ file, at }) => file.hunks.map((h, i) => ({
		file,
		h,
		n: at + i
	})))));
	const firstOf = user_derived(() => new Map(get(placed).flatMap((s) => s.files.map(({ file, at }) => [file.path, at]))));
	const order = user_derived(() => get(placed).flatMap((s) => s.files.map(({ file }) => file.path)));
	const weakRows = user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const w of (get(r)?.groups ?? []).flatMap((g) => g.weakened ?? [])) m.set(w.path, [...m.get(w.path) ?? [], w]);
		return m;
	});
	const commentsBy = user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const c of get(r)?.comments ?? []) m.set(c.path, [...m.get(c.path) ?? [], c]);
		return m;
	});
	let cur = state(0);
	const here = user_derived(() => get(flat)[Math.min(get(cur), Math.max(0, get(flat).length - 1))]);
	let inView = state("");
	const selected = user_derived(() => get(inView) || get(here)?.file.path || get(order)[0] || "");
	let expanded = state(proxy(/* @__PURE__ */ new Set()));
	let said = state("");
	let col = state(null);
	function reveal(n) {
		document.getElementById(`hunk-${n}`)?.scrollIntoView({ block: "nearest" });
	}
	let addressing;
	function address() {
		clearTimeout(addressing);
		if (!onaddress()) return;
		addressing = setTimeout(() => {
			const at = get(flat)[get(cur)];
			if (at) onaddress()?.(`${at.file.path}@${at.h.digest ?? ""}`);
		}, 400);
	}
	function to(n) {
		if (n < 0 || n >= get(flat).length) return false;
		performance.mark("review:move");
		set(cur, n, true);
		set(inView, "");
		queueMicrotask(() => {
			reveal(n);
			try {
				performance.measure("review: hunk to hunk", "review:move");
			} catch {}
		});
		address();
		return true;
	}
	function moveFile(step) {
		const at = get(order).indexOf(get(here)?.file.path ?? "") + step;
		for (let i = at; i >= 0 && i < get(order).length; i += step) {
			const first = get(firstOf).get(get(order)[i]);
			if (first !== void 0 && get(byPath).get(get(order)[i])?.hunks.length) return to(first);
		}
		return false;
	}
	let onKey = "";
	let onPath = "";
	let lastFlat = [];
	user_effect(() => {
		const now = get(flat);
		untrack(() => {
			const was = lastFlat;
			lastFlat = now;
			if (!was.length || was === now) return;
			const at = now.findIndex((x) => x.file.path === onPath && x.h.digest === onKey);
			if (onKey && at >= 0 && at !== get(cur)) set(cur, at, true);
			const before = new Set(was.map((x) => `${x.file.path}@${x.h.digest}`));
			const moved = new Set(now.filter((x) => !before.has(`${x.file.path}@${x.h.digest}`)).map((x) => x.file.path));
			if (moved.size && was[0]?.h.digest) set(said, `This change moved — ${moved.size === 1 ? "1 file" : `${moved.size} files`} updated; what changed is unread again.`);
		});
	});
	user_effect(() => {
		onKey = get(flat)[get(cur)]?.h.digest ?? "";
		onPath = get(flat)[get(cur)]?.file.path ?? "";
	});
	let opened = "";
	user_effect(() => {
		const want = focus();
		if (!get(flat).length || untrack(() => opened) === `${get(change)}|${want}`) return;
		untrack(() => {
			opened = `${get(change)}|${want}`;
			if (!want) return;
			const [path, digest] = want.includes("@") ? [want.slice(0, want.lastIndexOf("@")), want.slice(want.lastIndexOf("@") + 1)] : [want, ""];
			const exact = get(flat).find((x) => x.file.path === path && digest && x.h.digest === digest);
			const first = get(firstOf).get(path);
			if (exact) to(exact.n);
			else if (first !== void 0 && get(byPath).get(path)?.hunks.length) {
				to(first);
				if (digest) set(said, `The hunk the address names has changed since; showing \`${path}\` from its first hunk.`);
			} else if (path) {
				set(said, `\`${path}\` is not in this change now; showing the first hunk.`);
				to(0);
			}
		});
	});
	let rereading;
	function reread() {
		clearTimeout(rereading);
		rereading = setTimeout(() => void get(read).reload(), 900);
	}
	async function markHere() {
		const x = get(flat)[get(cur)];
		if (!x) return;
		if (!x.h.digest) {
			set(said, "This recording carries no hunk digests, so nothing can be marked.");
			return;
		}
		to(get(cur) + 1);
		try {
			await markSeen(get(change), x.file.path, x.h.digest);
			reread();
		} catch (e) {
			set(said, `That mark did not land: ${failure(e).says}`);
		}
	}
	async function readPath(path) {
		try {
			await markSeen(get(change), path, null);
			set(said, `Marked \`${path}\` seen, as yours.`);
			await get(read).reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	let drafting = state(null);
	let busy = state("");
	user_pre_effect(() => {
		get(key);
		untrack(() => {
			set(cur, 0);
			set(inView, "");
			set(drafting, null);
			set(said, "");
		});
	});
	function startComment(path, side, line) {
		set(drafting, {
			path,
			side,
			line
		}, true);
	}
	function commentHere() {
		const x = get(flat)[get(cur)];
		if (!x) return;
		const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(x.h.header);
		let o = m ? Number(m[1]) : 0;
		let n = m ? Number(m[2]) : 0;
		for (const [kind] of x.h.lines) {
			if (kind === "added") return startComment(x.file.path, "new", n);
			if (kind === "removed") return startComment(x.file.path, "old", o);
			o++;
			n++;
		}
	}
	async function saveComment(body) {
		const d = get(drafting);
		if (!d || !body.trim() || get(busy)) return;
		set(busy, "Keeping the comment");
		try {
			await api(`/api/changes/${encodeURIComponent(get(change))}/comments`, {
				method: "POST",
				body: JSON.stringify({
					path: d.path,
					line: d.line,
					side: d.side,
					body: body.trim()
				})
			});
			set(drafting, null);
			set(said, "");
			await get(read).reload();
		} catch (e) {
			set(said, `The comment was not kept: ${failure(e).says}`);
		} finally {
			set(busy, "");
		}
	}
	async function sendReview() {
		if (get(busy) || !(get(r)?.comments ?? []).some((c) => !c.sent_at)) return false;
		set(busy, "Sending");
		try {
			const res = await api(`/api/changes/${encodeURIComponent(get(change))}/comments/send`, { method: "POST" });
			set(said, res.says ?? "Sent.", true);
			await get(read).reload();
		} catch (e) {
			const body = e instanceof Refused ? e.body : null;
			set(said, `Nothing was sent: ${body?.says ?? failure(e).says}. The comments are kept.`);
		} finally {
			set(busy, "");
		}
		return true;
	}
	async function removeComment(cid) {
		try {
			await api(`/api/changes/${encodeURIComponent(get(change))}/comments/${encodeURIComponent(cid)}`, { method: "DELETE" });
			await get(read).reload();
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	function jumpTo(c) {
		const x = get(flat).find((y) => y.file.path === c.path && y.h.digest === c.hunk_digest) ?? get(flat).find((y) => y.file.path === c.path);
		if (x) to(x.n);
	}
	let framing = 0;
	function scrolled() {
		if (framing || !get(col)) return;
		framing = requestAnimationFrame(() => {
			framing = 0;
			if (!get(col)) return;
			const top = get(col).getBoundingClientRect().top + 8;
			for (const s of get(col).querySelectorAll("section.file")) if (s.getBoundingClientRect().bottom > top) {
				set(inView, s.dataset.path ?? "", true);
				return;
			}
		});
	}
	user_effect(() => {
		const offs = [
			onAction("hunk-next", () => to(get(cur) + 1) || true),
			onAction("hunk-prev", () => to(get(cur) - 1) || true),
			onAction("file-next", () => (moveFile(1), true)),
			onAction("file-prev", () => (moveFile(-1), true)),
			onAction("hunk-seen", () => (markHere(), true)),
			onAction("hunk-fix", () => (commentHere(), true)),
			onAction("review-send", () => get(drafting) ? false : (sendReview(), true)),
			onAction("hunk-expand", () => (get(here) && set(expanded, /* @__PURE__ */ new Set([...get(expanded), get(here).file.path]), true), true)),
			onAction("tab-risk", () => (set(by, "risk"), true)),
			onAction("tab-intent", () => (set(by, "intent"), true)),
			onAction("diff-mode", () => (set(mode, get(mode) === "unified" ? "split" : "unified", true), true))
		];
		return () => offs.forEach((off) => off());
	});
	var fragment_1 = comment();
	var node = first_child(fragment_1);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "the review",
			get failure() {
				return get(read).failure;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		Skeleton($$anchor, { rows: 5 });
	};
	var consequent_2 = ($$anchor) => {
		{
			let $0 = user_derived(() => get(r).empty_says ?? "The worktree has no changes against its base.");
			Empty($$anchor, {
				icon: "split",
				title: "Nothing to review yet",
				get body() {
					return get($0);
				}
			});
		}
	};
	var alternate = ($$anchor) => {
		var div = root_8();
		var header = child(div);
		var span = child(header);
		var code = sibling(child(span));
		var text$1 = only_child(code, true);
		reset(span);
		var span_1 = sibling(span, 2);
		var text_1 = only_child(span_1, true);
		var node_1 = sibling(span_1, 2);
		var consequent_3 = ($$anchor) => {
			var span_2 = root$2();
			var text_2 = only_child(span_2, true);
			template_effect(() => set_text(text_2, get(unreadNow)));
			append($$anchor, span_2);
		};
		if_block(node_1, ($$render) => {
			if (get(unreadNow)) $$render(consequent_3);
		});
		var node_2 = sibling(node_1, 4);
		{
			let $0 = user_derived(() => get(r).comments ?? []);
			Comments(node_2, {
				get comments() {
					return get($0);
				},
				get busy() {
					return get(busy);
				},
				send: () => void sendReview(),
				jump: jumpTo,
				remove: removeComment
			});
		}
		var div_1 = sibling(node_2, 2);
		var button = child(div_1);
		let classes;
		var button_1 = sibling(button, 2);
		let classes_1;
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		var button_2 = child(div_2);
		let classes_2;
		var node_3 = child(button_2);
		Icon(node_3, {
			name: "unified",
			size: 14
		});
		reset(button_2);
		var button_3 = sibling(button_2, 2);
		let classes_3;
		var node_4 = child(button_3);
		Icon(node_4, {
			name: "split",
			size: 14
		});
		reset(button_3);
		reset(div_2);
		reset(header);
		var node_5 = sibling(header, 2);
		var consequent_4 = ($$anchor) => {
			var p_1 = root_1$2();
			var node_6 = child(p_1);
			Inline(node_6, { get text() {
				return get(said);
			} });
			reset(p_1);
			append($$anchor, p_1);
		};
		if_block(node_5, ($$render) => {
			if (get(said)) $$render(consequent_4);
		});
		var node_7 = sibling(node_5, 2);
		var consequent_5 = ($$anchor) => {
			Failed($$anchor, {
				what: "the review",
				get failure() {
					return get(read).failure;
				},
				get at() {
					return get(read).at;
				},
				stale: true
			});
		};
		if_block(node_7, ($$render) => {
			if (get(read).phase === "stale" && get(read).failure) $$render(consequent_5);
		});
		var node_8 = sibling(node_7, 2);
		var consequent_7 = ($$anchor) => {
			var p_2 = root_3$1();
			var node_9 = child(p_2);
			Icon(node_9, {
				name: "alert",
				size: 12
			});
			var node_10 = sibling(node_9, 2);
			each(node_10, 17, () => get(findings), index, ($$anchor, f, i) => {
				var fragment_6 = root_2$2();
				var node_11 = first_child(fragment_6);
				var consequent_6 = ($$anchor) => {
					var text_3 = text();
					text_3.nodeValue = " · ";
					append($$anchor, text_3);
				};
				if_block(node_11, ($$render) => {
					if (i) $$render(consequent_6);
				});
				var node_12 = sibling(node_11);
				Inline(node_12, { get text() {
					return get(f);
				} });
				append($$anchor, fragment_6);
			});
			reset(p_2);
			append($$anchor, p_2);
		};
		if_block(node_8, ($$render) => {
			if (get(findings).length) $$render(consequent_7);
		});
		var div_3 = sibling(node_8, 2);
		var node_13 = child(div_3);
		{
			const pane = ($$anchor) => {
				var div_4 = root_4();
				Files(child(div_4), {
					get sections() {
						return get(sections);
					},
					get selected() {
						return get(selected);
					},
					unreadOf: (f) => unreadIn(get(change), f),
					pick: (p) => {
						const n = get(firstOf).get(p);
						if (n !== void 0 && get(byPath).get(p)?.hunks.length) to(n);
						else document.querySelector(`section.file[data-path="${CSS.escape(p)}"]`)?.scrollIntoView({ block: "start" });
					}
				});
				reset(div_4);
				append($$anchor, div_4);
			};
			Split(node_13, {
				id: "review-files",
				size: 260,
				min: 180,
				max: 520,
				pane,
				children: ($$anchor, $$slotProps) => {
					var div_5 = root_7();
					each(div_5, 21, () => get(placed), (s) => s.says, ($$anchor, s) => {
						var fragment_8 = root_6();
						var node_15 = first_child(fragment_8);
						var consequent_8 = ($$anchor) => {
							var h3 = root_5();
							var node_16 = child(h3);
							Inline(node_16, { get text() {
								return get(s).says;
							} });
							var span_3 = sibling(node_16, 2);
							var text_4 = only_child(span_3, true);
							reset(h3);
							template_effect(() => {
								set_class(h3, 1, `sec ${get(s).tone ?? "" ?? ""}`, "svelte-15l7e6g");
								set_text(text_4, get(s).files.length);
							});
							append($$anchor, h3);
						};
						if_block(node_15, ($$render) => {
							if (get(placed).length > 1 || get(s).tone) $$render(consequent_8);
						});
						var node_17 = sibling(node_15, 2);
						each(node_17, 17, () => get(s).files, ({ file, at }) => file.path, ($$anchor, $$item) => {
							let file = () => get($$item).file;
							let at = () => get($$item).at;
							{
								let $0 = user_derived(() => get(cur) >= at() && get(cur) < at() + file().hunks.length ? get(cur) : -1);
								let $1 = user_derived(() => get(expanded).has(file().path));
								let $2 = user_derived(() => get(weakRows).get(file().path) ?? []);
								let $3 = user_derived(() => get(commentsBy).get(file().path) ?? []);
								let $4 = user_derived(() => get(drafting)?.path === file().path ? get(drafting) : null);
								FileSection($$anchor, {
									get change() {
										return get(change);
									},
									get file() {
										return file();
									},
									get at() {
										return at();
									},
									get current() {
										return get($0);
									},
									get mode() {
										return get(mode);
									},
									get expanded() {
										return get($1);
									},
									get weak() {
										return get($2);
									},
									get comments() {
										return get($3);
									},
									get drafting() {
										return get($4);
									},
									get composer() {
										return composer;
									},
									pick: (n) => to(n),
									expand: () => set(expanded, /* @__PURE__ */ new Set([...get(expanded), file().path]), true),
									comment: (side, line) => startComment(file().path, side, line),
									remove: removeComment,
									readRows: () => void readPath(file().path)
								});
							}
						});
						append($$anchor, fragment_8);
					});
					reset(div_5);
					bind_this(div_5, ($$value) => set(col, $$value), () => get(col));
					event("scroll", div_5, scrolled);
					append($$anchor, div_5);
				},
				$$slots: {
					pane: true,
					default: true
				}
			});
		}
		reset(div_3);
		reset(div);
		template_effect(($0) => {
			set_text(text$1, get(r).base);
			set_text(text_1, $0);
			classes = set_class(button, 1, "svelte-15l7e6g", null, classes, { on: get(by) === "risk" });
			classes_1 = set_class(button_1, 1, "svelte-15l7e6g", null, classes_1, { on: get(by) === "intent" });
			classes_2 = set_class(button_2, 1, "svelte-15l7e6g", null, classes_2, { on: get(mode) === "unified" });
			classes_3 = set_class(button_3, 1, "svelte-15l7e6g", null, classes_3, { on: get(mode) === "split" });
		}, [() => shape(get(r).shape_says)]);
		delegated("click", button, () => set(by, "risk"));
		delegated("click", button_1, () => set(by, "intent"));
		delegated("click", button_2, () => set(mode, "unified"));
		delegated("click", button_3, () => set(mode, "split"));
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (get(read).failure && !get(r)) $$render(consequent);
		else if (!get(r)) $$render(consequent_1, 1);
		else if (get(files).length === 0) $$render(consequent_2, 2);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment_1);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Refusal.svelte
var root$1 = from_html(`<b class="kind"> </b>`);
var root_1$1 = from_html(`<li class="svelte-1wz92j1"><code class="svelte-1wz92j1"> </code> <!><span class="svelte-1wz92j1"><!></span> <button class="svelte-1wz92j1"><!> Read it</button></li>`);
var root_2$1 = from_html(`<div class="refusal svelte-1wz92j1" role="alert"><p class="svelte-1wz92j1"><b>Not offered</b> — <!>. Nothing was pushed.</p> <ul class="svelte-1wz92j1"></ul> <p class="how svelte-1wz92j1">Mark each row read in the review, or from a terminal of your own: <code class="svelte-1wz92j1"> </code></p></div>`);
function Refusal($$anchor, $$props) {
	push($$props, true);
	var div = root_2$1();
	var p = child(div);
	var node = sibling(child(p), 2);
	Inline(node, { get text() {
		return $$props.refusal.says;
	} });
	next();
	reset(p);
	var ul = sibling(p, 2);
	each(ul, 21, () => $$props.refusal.rows, (row) => row.path + row.matched, ($$anchor, row) => {
		var li = root_1$1();
		var code = child(li);
		var text = only_child(code, true);
		var node_1 = sibling(code, 2);
		var consequent = ($$anchor) => {
			var b = root$1();
			var text_1 = only_child(b, true);
			template_effect(() => set_text(text_1, weakKindSays[get(row).kind]));
			append($$anchor, b);
		};
		if_block(node_1, ($$render) => {
			if (get(row).kind) $$render(consequent);
		});
		var span = sibling(node_1);
		var node_2 = child(span);
		Inline(node_2, { get text() {
			return get(row).why;
		} });
		reset(span);
		var button = sibling(span, 2);
		var node_3 = child(button);
		Icon(node_3, {
			name: "split",
			size: 13
		});
		next();
		reset(button);
		reset(li);
		template_effect(() => set_text(text, get(row).path));
		delegated("click", button, () => $$props.read(get(row).path));
		append($$anchor, li);
	});
	reset(ul);
	var p_1 = sibling(ul, 2);
	var code_1 = sibling(child(p_1));
	var text_2 = only_child(code_1, true);
	reset(p_1);
	reset(div);
	template_effect(() => set_text(text_2, $$props.refusal.seen_with));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Doc.svelte
var root = from_html(`<div class="loading svelte-178dv07"><!></div>`);
var root_1 = from_html(`<div class="failed svelte-178dv07"><!></div>`);
var root_2 = from_html(`<div class="under svelte-178dv07"><!></div>`);
var root_3 = from_html(`<article><!> <p role="status" aria-live="polite"><!></p> <!> <!> <!> <div class="tabbar svelte-178dv07"><!></div> <div role="tabpanel"><!></div></article>`);
function Doc($$anchor, $$props) {
	push($$props, true);
	let id = prop($$props, "id", 3, ""), brief = prop($$props, "brief", 3, null), loaded = prop($$props, "loaded", 3, false), planted = prop($$props, "detail", 3, null), plantedReview = prop($$props, "review", 3, null), asked = prop($$props, "view", 3, ""), showView = prop($$props, "showView", 3, null);
	const key = user_derived(id);
	const read = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}` : null, { tell: () => `devplane change show ${get(key)}` });
	const d = user_derived(() => read.data ?? planted());
	const reviewRead = resource(() => get(key) ? `/api/changes/${encodeURIComponent(get(key))}/review` : null, { tell: () => `devplane change review ${get(key)}` });
	const review = user_derived(() => reviewRead.data ?? plantedReview());
	let said = state("");
	let commands = state(proxy([]));
	const pending = writer();
	const VIEWS = [
		"overview",
		"tasks",
		"review",
		"gates",
		"ledger",
		"agent"
	];
	let local = state("overview");
	const askedView = user_derived(() => asked().split("/")[0]);
	const askedHunk = user_derived(() => get(askedView) === "review" ? asked().slice(7) : "");
	const view = user_derived(() => VIEWS.includes(get(askedView)) ? get(askedView) : get(local));
	const lost = user_derived(() => get(askedView) && !VIEWS.includes(get(askedView)) ? `There is no view called "${get(askedView)}"; this is the overview.` : "");
	function setView(v) {
		set(local, v, true);
		showView()?.(v === "overview" ? "" : v);
	}
	let refusal = state(null);
	let reviewAt = state("");
	let lastMoved = null;
	user_pre_effect(() => {
		get(key);
		untrack(() => {
			set(local, "overview");
			set(said, "");
			set(commands, [], true);
			set(refusal, null);
			set(reviewAt, "");
			lastMoved = null;
		});
	});
	const moved = user_derived(() => brief()?.state ?? "");
	user_effect(() => {
		const m = get(moved);
		untrack(() => {
			if (lastMoved !== null && m !== lastMoved) {
				read.reload();
				reviewRead.reload();
			}
			lastMoved = m;
		});
	});
	const verified = user_derived(() => get(d)?.state === STATES.verified.word);
	const unseen = user_derived(() => get(d)?.qualifier?.unseen ?? 0);
	const leads = user_derived(() => weakLeads(get(d)?.qualifier));
	const unreadNow = user_derived(() => unreadSays({
		hunks: unread(get(review), get(unseen)).hunks,
		weakened: 0
	}));
	const uncovered = user_derived(() => !!get(d)?.tree_now && get(d).tree_now.clean === false && !!get(d).gate?.ran_at?.tree && get(d).gate.ran_at.tree !== get(d).tree_now.tree);
	const standing = user_derived(() => get(d)?.standing_says && !(get(verified) && (get(leads) || get(uncovered))) ? get(d).standing_says : "");
	function readRow(path) {
		set(reviewAt, path, true);
		setView("review");
	}
	const lastRun = user_derived(() => get(d)?.runs?.[get(d).runs.length - 1] ?? "");
	const ROUTE = {
		verify: (id) => `/api/changes/${id}/verify`,
		finish: (id) => `/api/changes/${id}/finish`,
		offer: (id) => `/api/changes/${id}/offer`,
		resume: (id) => `/api/changes/${id}/resume`,
		retry: (id) => `/api/changes/${id}/retry`,
		archive: (id) => `/api/changes/${id}/archive`
	};
	const PAST = {
		verify: "the gates ran",
		finish: "finished — recorded as yours",
		offer: STATES.offered.word,
		resume: "picked back up",
		retry: "trying again",
		archive: "archived — the worktree is removed and the record kept"
	};
	const DOING = {
		verify: "Running the gates",
		finish: "Finishing",
		offer: "Offering",
		resume: "Picking it back up",
		retry: "Trying again",
		archive: "Archiving"
	};
	function outcome(verb, r) {
		if (verb === "offer" && r?.offer !== "opened") return `${r?.not_opened_says ? `${r.not_opened_says}. ` : ""}Nothing was pushed — [github] pull_request is not set, so the pull request is yours to open. Run:`;
		if (verb === "verify" && typeof r?.passed === "boolean") return `${r.passed ? "The gates exited zero" : "The gates failed"}${r.summary ? `: ${r.summary}` : "."}`;
		return (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "") + (r?.not_opened_says ? `. ${r.not_opened_says}` : "");
	}
	async function act(verb) {
		const at = get(key);
		if (!at || pending.busy) return;
		await pending.run(verb, async () => {
			try {
				const r = await api(ROUTE[verb](encodeURIComponent(at)), { method: "POST" });
				if (at !== get(key)) return;
				set(said, outcome(verb, r), true);
				set(commands, verb === "offer" && r?.offer !== "opened" ? [r?.push, r?.create].filter((c) => !!c) : [], true);
				set(refusal, null);
				await Promise.all([read.reload(), reviewRead.reload()]);
			} catch (e) {
				if (at !== get(key)) return;
				set(commands, [], true);
				const body = e instanceof Refused ? e.body : null;
				if (body?.refused === "weakened_unseen") {
					set(said, "");
					set(refusal, {
						says: body.says ?? "",
						rows: body.rows ?? [],
						seen_with: body.seen_with ?? ""
					}, true);
				} else {
					const f = failure(e, `devplane change show ${at}`);
					set(said, `That did not land: ${f.says}. \`${f.tell}\` tells more.`);
				}
			}
		});
	}
	async function openIn(place) {
		try {
			const r = await api(`/api/changes/${encodeURIComponent(get(key))}/open?in=${place}`, { method: "POST" });
			const where = r.path ?? get(d)?.worktree ?? "";
			set(said, r.opened ? `Opened ${where || "the worktree"} in the ${place}.` : r.says ?? (where ? `Open it yourself: ${where}` : `It was not opened in the ${place}.`), true);
		} catch (e) {
			set(said, `That did not land: ${failure(e).says}`);
		}
	}
	async function copy(text, what) {
		set(said, await copyText(text) ? `Copied ${what}.` : `Nothing was copied — this page has no clipboard. ${what}: ${text}`, true);
	}
	const tabs = user_derived(() => [
		{
			id: "overview",
			label: "Overview",
			icon: "eye"
		},
		{
			id: "tasks",
			label: "Tasks",
			icon: "spec",
			count: get(d)?.counts?.tasks ?? null
		},
		{
			id: "review",
			label: "Review",
			icon: "split",
			count: get(unreadNow) || null,
			tone: get(unseen) > 0 ? "fail" : void 0
		},
		{
			id: "gates",
			label: "Gates",
			icon: "gate",
			count: get(d)?.gates?.length ?? null,
			tone: get(d)?.gate ? get(verified) && !get(leads) ? "done" : get(d).gate.passed ? void 0 : "fail" : void 0
		},
		{
			id: "ledger",
			label: "Ledger",
			icon: "ledger"
		},
		{
			id: "agent",
			label: "Agent",
			icon: "agent",
			count: get(d)?.runs?.length ?? null
		}
	]);
	user_effect(() => get(view) === "review" ? setMode("change", "review") : void 0);
	user_effect(() => {
		const to = (v) => () => (setView(v), true);
		const offs = [
			onAction("review", to("review")),
			onAction("view-overview", to("overview")),
			onAction("view-tasks", to("tasks")),
			onAction("view-review", to("review")),
			onAction("view-gates", to("gates")),
			onAction("view-ledger", to("ledger")),
			onAction("view-agent", to("agent"))
		];
		return () => offs.forEach((off) => off());
	});
	const canOffer = user_derived(() => get(verified) && get(unseen) === 0);
	const offerHeld = user_derived(() => get(verified) && get(unseen) > 0 ? `Read the weakened ${get(unseen) === 1 ? "check" : "checks"} first` : "");
	user_effect(() => {
		if (!get(d)) return;
		return offerCommands(get(d).title || get(d).id, [
			{
				label: `Review${get(unreadNow) ? ` — ${get(unreadNow)}` : ""}`,
				icon: "split",
				run: () => setView("review")
			},
			...get(d).worktree && !declaresNoCheck(get(d)) ? [{
				label: "Run gates",
				icon: "gate",
				run: () => void act("verify")
			}] : [],
			...get(canOffer) ? [{
				label: "Offer as pull request",
				icon: "forge",
				run: () => void act("offer")
			}] : []
		]);
	});
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		var consequent = ($$anchor) => {
			Empty($$anchor, {
				icon: "change",
				title: "Pick a change",
				body: "Choose one in the list, or start a new one — an isolated worktree, an agent in it, and the project's own gates."
			});
		};
		var alternate = ($$anchor) => {
			var div = root();
			var node_2 = child(div);
			Skeleton(node_2, { rows: 4 });
			reset(div);
			append($$anchor, div);
		};
		if_block(node_1, ($$render) => {
			if (loaded()) $$render(consequent);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_1);
	};
	var consequent_2 = ($$anchor) => {
		var div_1 = root_1();
		var node_3 = child(div_1);
		Failed(node_3, {
			what: "this change",
			get failure() {
				return read.failure;
			}
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	var consequent_3 = ($$anchor) => {
		var div_2 = root();
		var node_4 = child(div_2);
		Skeleton(node_4, { rows: 4 });
		reset(div_2);
		append($$anchor, div_2);
	};
	var alternate_2 = ($$anchor) => {
		var article = root_3();
		let classes;
		var node_5 = child(article);
		{
			let $0 = user_derived(() => pending.busy ?? "");
			let $1 = user_derived(() => get(review)?.not_opened ?? []);
			Head(node_5, {
				get d() {
					return get(d);
				},
				get view() {
					return get(view);
				},
				get unseen() {
					return get(unseen);
				},
				get verified() {
					return get(verified);
				},
				get leads() {
					return get(leads);
				},
				get standing() {
					return get(standing);
				},
				get offerHeld() {
					return get(offerHeld);
				},
				get busy() {
					return get($0);
				},
				get notOpened() {
					return get($1);
				},
				act,
				openIn,
				copy,
				review: () => setView("review"),
				reload: () => void read.reload()
			});
		}
		var p = sibling(node_5, 2);
		let classes_1;
		var node_6 = child(p);
		var consequent_4 = ($$anchor) => {
			var text_1 = text();
			template_effect(() => set_text(text_1, `${DOING[pending.busy] ?? ""} · ${pending.elapsed ?? ""} — nothing else can be started on this change until it is back.`));
			append($$anchor, text_1);
		};
		var alternate_1 = ($$anchor) => {
			{
				let $0 = user_derived(() => get(said) || get(lost));
				Inline($$anchor, { get text() {
					return get($0);
				} });
			}
		};
		if_block(node_6, ($$render) => {
			if (pending.busy) $$render(consequent_4);
			else $$render(alternate_1, -1);
		});
		reset(p);
		var node_7 = sibling(p, 2);
		var consequent_5 = ($$anchor) => {
			var div_3 = root_2();
			var node_8 = child(div_3);
			Commands(node_8, { get commands() {
				return get(commands);
			} });
			reset(div_3);
			append($$anchor, div_3);
		};
		if_block(node_7, ($$render) => {
			if (get(commands).length && !pending.busy) $$render(consequent_5);
		});
		var node_9 = sibling(node_7, 2);
		var consequent_6 = ($$anchor) => {
			var div_4 = root_2();
			var node_10 = child(div_4);
			Failed(node_10, {
				what: "this change",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
			reset(div_4);
			append($$anchor, div_4);
		};
		if_block(node_9, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_6);
		});
		var node_11 = sibling(node_9, 2);
		var consequent_7 = ($$anchor) => {
			var div_5 = root_2();
			Refusal(child(div_5), {
				get refusal() {
					return get(refusal);
				},
				read: readRow
			});
			reset(div_5);
			append($$anchor, div_5);
		};
		if_block(node_11, ($$render) => {
			if (get(refusal)) $$render(consequent_7);
		});
		var div_6 = sibling(node_11, 2);
		var node_13 = child(div_6);
		var bind_get = () => get(view);
		var bind_set = setView;
		Tabs(node_13, {
			get tabs() {
				return get(tabs);
			},
			get active() {
				return bind_get();
			},
			set active($$value) {
				bind_set($$value);
			},
			label: "views of this change"
		});
		reset(div_6);
		var div_7 = sibling(div_6, 2);
		let classes_2;
		var node_14 = child(div_7);
		var consequent_8 = ($$anchor) => {
			Overview($$anchor, {
				get d() {
					return get(d);
				},
				go: setView
			});
		};
		var consequent_9 = ($$anchor) => {
			Tasks($$anchor, {
				get d() {
					return get(d);
				},
				get lastRun() {
					return get(lastRun);
				},
				get reload() {
					return read.reload;
				}
			});
		};
		var consequent_10 = ($$anchor) => {
			{
				let $0 = user_derived(() => get(reviewAt) || get(askedHunk));
				Pane($$anchor, {
					get id() {
						return get(key);
					},
					get focus() {
						return get($0);
					},
					get source() {
						return reviewRead;
					},
					get review() {
						return plantedReview();
					},
					counted: true,
					onaddress: (at) => showView()?.(`review/${at}`)
				});
			}
		};
		var consequent_11 = ($$anchor) => {
			{
				let $0 = user_derived(() => (get(review)?.groups ?? []).flatMap((g) => g.weakened ?? []));
				Gates($$anchor, {
					get d() {
						return get(d);
					},
					get id() {
						return get(key);
					},
					reload: () => read.reload(),
					get weakened() {
						return get($0);
					},
					toLine: readRow
				});
			}
		};
		var consequent_12 = ($$anchor) => {
			Ledger($$anchor, { get id() {
				return get(key);
			} });
		};
		var consequent_13 = ($$anchor) => {
			Agent($$anchor, {
				get d() {
					return get(d);
				},
				get run() {
					return get(lastRun);
				}
			});
		};
		if_block(node_14, ($$render) => {
			if (get(view) === "overview") $$render(consequent_8);
			else if (get(view) === "tasks") $$render(consequent_9, 1);
			else if (get(view) === "review") $$render(consequent_10, 2);
			else if (get(view) === "gates") $$render(consequent_11, 3);
			else if (get(view) === "ledger") $$render(consequent_12, 4);
			else if (get(view) === "agent") $$render(consequent_13, 5);
		});
		reset(div_7);
		reset(article);
		template_effect(($0) => {
			classes = set_class(article, 1, "doc svelte-178dv07", null, classes, { reviewing: get(view) === "review" });
			classes_1 = set_class(p, 1, "said svelte-178dv07", null, classes_1, { empty: !get(said) && !pending.busy && !get(lost) });
			classes_2 = set_class(div_7, 1, "view svelte-178dv07", null, classes_2, { review: get(view) === "review" });
			set_attribute(div_7, "aria-label", $0);
		}, [() => get(tabs).find((t) => t.id === get(view))?.label]);
		append($$anchor, article);
	};
	if_block(node, ($$render) => {
		if (!id()) $$render(consequent_1);
		else if (read.failure && !get(d)) $$render(consequent_2, 1);
		else if (!get(d)) $$render(consequent_3, 2);
		else $$render(alternate_2, -1);
	});
	append($$anchor, fragment);
	pop();
}
//#endregion
export { Doc as default };

//# sourceMappingURL=Doc.js.map