import { $ as remove_textarea_child, A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, I as event, J as proxy, K as only_child, L as get, N as text, O as set_text, P as delegate, Q as autofocus, U as user_pre_effect, V as template_effect, W as child, X as state, Y as set, Z as user_derived, _ as set_style, at as noop, et as pop, f as bind_value, i as api, it as reset, j as from_html, k as append, m as set_attribute, o as prop, ot as to_array, q as sibling, rt as next, t as Refused, tt as push, v as set_class, w as index, y as clsx, z as untrack } from "./api.js";
import { D as onAction, S as STATES, a as Inline, d as shape, h as resource, i as Skeleton, k as setMode, m as failure, n as Icon, o as blocks, p as copyText, r as Split, t as Pill, x as LIFE } from "./app.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
import { c as Commands, i as markKey, n as unreadSays, o as prune, r as writeMark, s as readMark, t as unread } from "./unread.svelte.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
import { t as Props } from "./Props.js";
import { n as weakLeads, t as ChangeState } from "./ChangeState.js";
import { n as offerCommands } from "./offered.svelte.js";
import { t as Tabs } from "./Tabs.js";
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
function where(state, archived, offered, weakUnread = 0) {
	const at = archived ? STATES.archived.word : offered ? STATES.offered.word : state;
	const step = LIFE.includes(at) ? at : null;
	if (weakUnread > 0 && at !== STATES.archived.word) return {
		at,
		next: `read ${weakUnread} weakened ${weakUnread === 1 ? "check" : "checks"}`
	};
	return {
		at,
		next: step ? NEXT[step] : ""
	};
}
var root$13 = from_html(`<span class="arrow svelte-bllsbm" aria-hidden="true">→</span><span class="next"> </span>`, 1);
var root_1$13 = from_html(`<p class="stepper svelte-bllsbm" aria-label="where this change is" title="drafted → isolated → in flight → verified → offered → archived"><span class="at svelte-bllsbm" aria-current="step"> </span><!></p>`);
function Stepper($$anchor, $$props) {
	push($$props, true);
	let archived = prop($$props, "archived", 3, false), offered = prop($$props, "offered", 3, false), weakUnread = prop($$props, "weakUnread", 3, 0);
	const line = user_derived(() => where($$props.state, archived(), offered(), weakUnread()));
	var p = root_1$13();
	var span = child(p);
	var text = only_child(span, true);
	var node = sibling(span);
	var consequent = ($$anchor) => {
		var fragment = root$13();
		var span_1 = sibling(first_child(fragment));
		var text_1 = only_child(span_1);
		template_effect(() => set_text(text_1, `next: ${get(line).next ?? ""}`));
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (get(line).next) $$render(consequent);
	});
	reset(p);
	template_effect(() => set_text(text, get(line).at));
	append($$anchor, p);
	pop();
}
//#endregion
//#region src/lib/ui/Timeline.svelte
var root$12 = from_html(`<p class="none svelte-hwm3dn">Nothing has happened to it yet.</p>`);
var root_1$12 = from_html(`<button></button>`);
var root_2$11 = from_html(`<div class="lane svelte-hwm3dn"><span class="name svelte-hwm3dn"> </span> <div class="track svelte-hwm3dn"><!> <span class="now svelte-hwm3dn" aria-hidden="true"></span></div></div>`);
var root_3$11 = from_html(`<figure class="tl svelte-hwm3dn"><!> <div class="axis svelte-hwm3dn"><span></span><span class="ticks svelte-hwm3dn"><span> </span><span> </span></span></div> <figcaption aria-live="polite" class="svelte-hwm3dn"> </figcaption></figure>`);
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
		var p = root$12();
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var figure = root_3$11();
		var node_1 = child(figure);
		each(node_1, 16, () => $$props.lanes, (lane) => lane, ($$anchor, lane) => {
			var div = root_2$11();
			var span_1 = child(div);
			var text = only_child(span_1, true);
			var div_1 = sibling(span_1, 2);
			var node_2 = child(div_1);
			each(node_2, 17, () => $$props.marks.filter((m) => m.lane === lane), index, ($$anchor, m) => {
				var button = root_1$12();
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
var root$11 = from_html(`<span class="v quiet svelte-1og7e16">No gate has run against this change.</span>`);
var root_1$11 = from_html(`<span> </span>`);
var root_2$10 = from_html(`<code class="cmd svelte-1og7e16"> </code>`);
var root_3$10 = from_html(`<span class="v fail svelte-1og7e16"> </span> <!>`, 1);
var root_4$10 = from_html(`<span class="s wait svelte-1og7e16"> </span>`);
var root_5$9 = from_html(`<span class="v svelte-1og7e16"> </span> <!>`, 1);
var root_6$8 = from_html(`<span class="v quiet svelte-1og7e16"> </span>`);
var root_7$7 = from_html(`<span class="v quiet svelte-1og7e16">Not read.</span>`);
var root_8$6 = from_html(`<span class="v fail svelte-1og7e16">Not read: <!></span> <span class="s svelte-1og7e16"><code> </code> tells more</span>`, 1);
var root_9$6 = from_html(`<span class="v quiet svelte-1og7e16" aria-busy="true">…</span>`);
var root_10$5 = from_html(`<span class="v quiet svelte-1og7e16">Nothing has been decided about it.</span>`);
var root_11$5 = from_html(`<span class="v svelte-1og7e16"><b></b> read — there are more</span> <span class="s svelte-1og7e16"><code> </code> has every one</span>`, 1);
var root_12$5 = from_html(`<span class="v svelte-1og7e16"><b> </b> recorded</span> <span class="s svelte-1og7e16"> </span>`, 1);
var root_13$5 = from_html(`<span class="s fail svelte-1og7e16"> </span>`);
var root_14$5 = from_html(`<p class="svelte-1og7e16"><!></p>`);
var root_15$2 = from_html(`<section class="alert svelte-1og7e16"><!> <div><b>The specification moved under a run.</b> <!> <button class="svelte-1og7e16">Decide in Tasks</button></div></section>`);
var root_16$2 = from_html(`<div class="grid svelte-1og7e16"><section class="cards svelte-1og7e16"><button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Gates</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Tasks</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Decisions</span> <!></button> <button class="card svelte-1og7e16"><span class="k svelte-1og7e16"><!> Agent</span> <span class="v svelte-1og7e16"><b> </b> <!></span> <!></button></section> <!> <section class="facts svelte-1og7e16"><h2 class="svelte-1og7e16">Facts</h2> <!></section> <section class="history svelte-1og7e16"><h2 class="svelte-1og7e16">History</h2> <!> <!></section></div>`);
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
		...$$props.d.archived_at ? [{
			at: $$props.d.archived_at,
			lane: "change",
			label: STATES.archived.word,
			tone: "none"
		}] : []
	]);
	const byAuthority = user_derived(() => Object.entries(get(decisions).reduce((m, x) => (m[x.authority] = (m[x.authority] ?? 0) + 1, m), {})));
	var div = root_16$2();
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
		var span_1 = root$11();
		append($$anchor, span_1);
	};
	var consequent_1 = ($$anchor) => {
		var span_2 = root_1$11();
		let classes;
		var text = only_child(span_2);
		template_effect(($0) => {
			classes = set_class(span_2, 1, "v svelte-1og7e16", null, classes, { done: $0 });
			set_text(text, `${$$props.d.gate.name ?? ""} exited zero · attempt ${$$props.d.gate.attempt ?? ""}`);
		}, [() => $$props.d.state === STATES.verified.word && !weakLeads($$props.d.qualifier)]);
		append($$anchor, span_2);
	};
	var alternate = ($$anchor) => {
		var fragment = root_3$10();
		var span_3 = first_child(fragment);
		var text_1 = only_child(span_3);
		var node_2 = sibling(span_3, 2);
		each(node_2, 17, () => get(failing).slice(0, 2), (c) => c.command, ($$anchor, c) => {
			var code = root_2$10();
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
		var fragment_1 = root_5$9();
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
	var alternate_1 = ($$anchor) => {
		var span_7 = root_6$8();
		var text_5 = only_child(span_7, true);
		template_effect(() => set_text(text_5, $$props.d.spec ? "The specification has no tasks." : "No specification is attached."));
		append($$anchor, span_7);
	};
	if_block(node_4, ($$render) => {
		if ($$props.d.counts) $$render(consequent_3);
		else $$render(alternate_1, -1);
	});
	reset(button_1);
	var button_2 = sibling(button_1, 2);
	var span_8 = child(button_2);
	var node_6 = child(span_8);
	Icon(node_6, {
		name: "ledger",
		size: 14
	});
	next();
	reset(span_8);
	var node_7 = sibling(span_8, 2);
	var consequent_5 = ($$anchor) => {
		var fragment_2 = comment();
		var node_8 = first_child(fragment_2);
		var consequent_4 = ($$anchor) => {
			var span_9 = root_7$7();
			append($$anchor, span_9);
		};
		var alternate_2 = ($$anchor) => {
			var fragment_3 = root_8$6();
			var span_10 = first_child(fragment_3);
			var node_9 = sibling(child(span_10));
			Inline(node_9, { get text() {
				return read.failure.says;
			} });
			reset(span_10);
			var span_11 = sibling(span_10, 2);
			var code_1 = child(span_11);
			var text_6 = only_child(code_1, true);
			next();
			reset(span_11);
			template_effect(() => set_text(text_6, read.failure.tell));
			append($$anchor, fragment_3);
		};
		if_block(node_8, ($$render) => {
			if (read.failure.down) $$render(consequent_4);
			else $$render(alternate_2, -1);
		});
		append($$anchor, fragment_2);
	};
	var consequent_6 = ($$anchor) => {
		var span_12 = root_9$6();
		append($$anchor, span_12);
	};
	var consequent_7 = ($$anchor) => {
		var span_13 = root_10$5();
		append($$anchor, span_13);
	};
	var consequent_8 = ($$anchor) => {
		var fragment_4 = root_11$5();
		var span_14 = first_child(fragment_4);
		var b = child(span_14);
		b.textContent = "200";
		next();
		reset(span_14);
		var span_15 = sibling(span_14, 2);
		var code_2 = child(span_15);
		var text_7 = only_child(code_2);
		next();
		reset(span_15);
		template_effect(() => set_text(text_7, `devplane audit ${get(about) ?? ""}`));
		append($$anchor, fragment_4);
	};
	var alternate_3 = ($$anchor) => {
		var fragment_5 = root_12$5();
		var span_16 = first_child(fragment_5);
		var b_1 = child(span_16);
		var text_8 = only_child(b_1, true);
		next();
		reset(span_16);
		var span_17 = sibling(span_16, 2);
		var text_9 = only_child(span_17, true);
		template_effect(($0) => {
			set_text(text_8, get(decisions).length);
			set_text(text_9, $0);
		}, [() => get(byAuthority).map(([a, n]) => `${n} by ${a}`).join(" · ")]);
		append($$anchor, fragment_5);
	};
	if_block(node_7, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_5);
		else if (read.phase === "loading") $$render(consequent_6, 1);
		else if (get(decisions).length === 0) $$render(consequent_7, 2);
		else if (get(decisions).length >= LIMIT) $$render(consequent_8, 3);
		else $$render(alternate_3, -1);
	});
	reset(button_2);
	var button_3 = sibling(button_2, 2);
	var span_18 = child(button_3);
	var node_10 = child(span_18);
	Icon(node_10, {
		name: "agent",
		size: 14
	});
	next();
	reset(span_18);
	var span_19 = sibling(span_18, 2);
	var b_2 = child(span_19);
	var text_10 = only_child(b_2, true);
	var text_11 = sibling(b_2);
	var node_11 = sibling(text_11);
	var consequent_9 = ($$anchor) => {
		var text_12 = text();
		template_effect(() => set_text(text_12, `· ${$$props.d.feedback_rounds ?? ""} handed back`));
		append($$anchor, text_12);
	};
	if_block(node_11, ($$render) => {
		if ($$props.d.feedback_rounds) $$render(consequent_9);
	});
	reset(span_19);
	var node_12 = sibling(span_19, 2);
	var consequent_10 = ($$anchor) => {
		var span_20 = root_13$5();
		var text_13 = only_child(span_20, true);
		template_effect(() => set_text(text_13, $$props.d.stopped_summary));
		append($$anchor, span_20);
	};
	if_block(node_12, ($$render) => {
		if ($$props.d.stopped_summary) $$render(consequent_10);
	});
	reset(button_3);
	reset(section);
	var node_13 = sibling(section, 2);
	var consequent_11 = ($$anchor) => {
		var section_1 = root_15$2();
		var node_14 = child(section_1);
		Icon(node_14, {
			name: "alert",
			size: 16
		});
		var div_1 = sibling(node_14, 2);
		var node_15 = sibling(child(div_1), 2);
		each(node_15, 17, () => $$props.d.drifts ?? [], index, ($$anchor, x) => {
			var p = root_14$5();
			var node_16 = child(p);
			Inline(node_16, { get text() {
				return get(x).says;
			} });
			reset(p);
			append($$anchor, p);
		});
		var button_4 = sibling(node_15, 2);
		reset(div_1);
		reset(section_1);
		delegated("click", button_4, () => $$props.go("tasks"));
		append($$anchor, section_1);
	};
	if_block(node_13, ($$render) => {
		if (($$props.d.drifts ?? []).length > 0) $$render(consequent_11);
	});
	var section_2 = sibling(node_13, 2);
	var node_17 = sibling(child(section_2), 2);
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
		Props(node_17, { get rows() {
			return get($0);
		} });
	}
	reset(section_2);
	var section_3 = sibling(section_2, 2);
	var node_18 = sibling(child(section_3), 2);
	var consequent_12 = ($$anchor) => {
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
	if_block(node_18, ($$render) => {
		if (read.failure && !(read.failure.down && read.data === null)) $$render(consequent_12);
	});
	Timeline(sibling(node_18, 2), {
		get marks() {
			return get(marks);
		},
		lanes: [
			"change",
			"gates",
			"decisions"
		]
	});
	reset(section_3);
	reset(div);
	template_effect(() => {
		set_text(text_10, $$props.d.runs?.length ?? 0);
		set_text(text_11, ` ${$$props.d.runs?.length === 1 ? "run" : "runs"}`);
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
//#region src/surfaces/change/Gates.svelte
var root$10 = from_html(`<p class="quiet svelte-wqjyvl">No gate has run against this change yet. The gates run when the agent says it has finished, or when you press <b>Run gates</b>.</p>`);
var root_1$10 = from_html(`<code class="tree svelte-wqjyvl" title="the tree this ran against"> </code>`);
var root_2$9 = from_html(`<span class="tests svelte-wqjyvl"> </span>`);
var root_3$9 = from_html(`<li class="svelte-wqjyvl"> </li>`);
var root_4$9 = from_html(`<ul class="fails svelte-wqjyvl"></ul>`);
var root_5$8 = from_html(`<pre class="out svelte-wqjyvl"> </pre> <span class="bytes svelte-wqjyvl"> </span>`, 1);
var root_6$7 = from_html(`<li><div class="row svelte-wqjyvl"><!> <code class="cmd svelte-wqjyvl"> </code> <span class="exit svelte-wqjyvl"> </span> <!> <span class="dur svelte-wqjyvl"> </span></div> <!> <!></li>`);
var root_7$6 = from_html(`<ol class="cmds svelte-wqjyvl"></ol>`);
var root_8$5 = from_html(`<section><button class="head svelte-wqjyvl"><!> <!> <b> </b> <span> </span> <span class="verdict svelte-wqjyvl"> </span> <span class="meta svelte-wqjyvl"> </span> <!></button> <!></section>`);
var root_9$5 = from_html(`<button class="svelte-wqjyvl"><!> Run base checks</button>`);
var root_10$4 = from_html(`<p class="w-warn svelte-wqjyvl"> </p>`);
var root_11$4 = from_html(`<p class="said svelte-wqjyvl"> </p>`);
var root_12$4 = from_html(`<p class="quiet svelte-wqjyvl"> </p>`);
var root_13$4 = from_html(`<code class="svelte-wqjyvl"> </code> `, 1);
var root_14$4 = from_html(`<p class="files svelte-wqjyvl">Put back as the base has them: <!></p>`);
var root_15$1 = from_html(`<p class="files svelte-wqjyvl">Removed, as the base lacks them: <!></p>`);
var root_16$1 = from_html(`<p class="cmdline svelte-wqjyvl"><code class="svelte-wqjyvl"> </code> </p>`);
var root_17$1 = from_html(`<li> </li>`);
var root_18$1 = from_html(`<p class="quiet svelte-wqjyvl"> <code> </code> lists every one</p>`);
var root_19 = from_html(`<ul class="names svelte-wqjyvl"></ul> <!>`, 1);
var root_20 = from_html(`<p class="svelte-wqjyvl"> </p> <ul class="names svelte-wqjyvl"></ul> <!>`, 1);
var root_21 = from_html(`<h3 class="svelte-wqjyvl">The base's tests on this change</h3> <p> </p> <!> <!> <!> <!> <!> <!>`, 1);
var root_22 = from_html(`<span class="quiet svelte-wqjyvl"> </span>`);
var root_23 = from_html(`<code class="svelte-wqjyvl"> </code><!> `, 1);
var root_24 = from_html(`<p class="files svelte-wqjyvl">Placed: <!></p>`);
var root_25 = from_html(`<p class="files svelte-wqjyvl">Not separable: <!></p>`);
var root_26 = from_html(`<p class="svelte-wqjyvl">Passing without the change:</p><ul class="names svelte-wqjyvl"></ul>`, 1);
var root_27 = from_html(`<h3 class="svelte-wqjyvl">This change's new tests on the base</h3> <p> </p> <!> <!> <!> <!> <!>`, 1);
var root_28 = from_html(`<section class="base svelte-wqjyvl"><header class="svelte-wqjyvl"><h2 class="svelte-wqjyvl"><!> Base checks</h2> <!></header> <p class="counts svelte-wqjyvl"> </p> <!> <!> <!> <!> <!></section>`);
var root_29 = from_html(`<button class="svelte-wqjyvl"><!> Copy as markdown</button>`);
var root_30 = from_html(`<p class="said svelte-wqjyvl"><!></p>`);
var root_31 = from_html(`<pre class="svelte-wqjyvl"> </pre>`);
var root_32 = from_html(`<h3 class="svelte-wqjyvl"><!></h3>`);
var root_33 = from_html(`<p class="li svelte-wqjyvl">• <!></p>`);
var root_34 = from_html(`<p class="row svelte-wqjyvl"><!></p>`);
var root_35 = from_html(`<p class="svelte-wqjyvl"><!></p>`);
var root_36 = from_html(`<div class="md svelte-wqjyvl"></div>`);
var root_37 = from_html(`<div class="gates svelte-wqjyvl"><!> <!> <section class="cert svelte-wqjyvl"><header class="svelte-wqjyvl"><h2 class="svelte-wqjyvl"><!> Certificate</h2> <!></header> <!> <!></section></div>`);
function Gates($$anchor, $$props) {
	push($$props, true);
	let reload = prop($$props, "reload", 3, () => {}), certificate = prop($$props, "certificate", 3, null);
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
	var div = root_37();
	var node = child(div);
	var consequent = ($$anchor) => {
		var p = root$10();
		append($$anchor, p);
	};
	var alternate = ($$anchor) => {
		var fragment = comment();
		var node_1 = first_child(fragment);
		each(node_1, 19, () => get(attempts), (g) => g.at, ($$anchor, g, i) => {
			const ok = user_derived(() => passed(get(g)));
			const shown = user_derived(() => get(open)[get(i)] ?? get(i) === 0);
			var section = root_8$5();
			let classes;
			var button = child(section);
			var node_2 = child(button);
			{
				let $0 = user_derived(() => get(shown) ? "down" : "right");
				Icon(node_2, {
					get name() {
						return get($0);
					},
					size: 12
				});
			}
			var node_3 = sibling(node_2, 2);
			{
				let $0 = user_derived(() => get(ok) ? "check" : "x");
				Icon(node_3, {
					get name() {
						return get($0);
					},
					size: 15
				});
			}
			var b_1 = sibling(node_3, 2);
			var text = only_child(b_1, true);
			var span = sibling(b_1, 2);
			var text_1 = only_child(span);
			var span_1 = sibling(span, 2);
			var text_2 = only_child(span_1, true);
			var span_2 = sibling(span_1, 2);
			var text_3 = only_child(span_2);
			var node_4 = sibling(span_2, 2);
			var consequent_1 = ($$anchor) => {
				var code = root_1$10();
				var text_4 = only_child(code);
				template_effect(($0) => set_text(text_4, `tree ${$0 ?? ""}`), [() => get(g).commit.tree.slice(0, 10)]);
				append($$anchor, code);
			};
			if_block(node_4, ($$render) => {
				if (get(g).commit?.tree) $$render(consequent_1);
			});
			reset(button);
			var node_5 = sibling(button, 2);
			var consequent_5 = ($$anchor) => {
				var ol = root_7$6();
				each(ol, 21, () => get(g).commands, index, ($$anchor, c) => {
					const cok = user_derived(() => get(c).outcome.outcome === "exited" && get(c).outcome.code === 0);
					var li = root_6$7();
					let classes_1;
					var div_1 = child(li);
					var node_6 = child(div_1);
					{
						let $0 = user_derived(() => get(cok) ? "check" : "x");
						Icon(node_6, {
							get name() {
								return get($0);
							},
							size: 13
						});
					}
					var code_1 = sibling(node_6, 2);
					var text_5 = only_child(code_1, true);
					var span_3 = sibling(code_1, 2);
					var text_6 = only_child(span_3, true);
					var node_7 = sibling(span_3, 2);
					var consequent_2 = ($$anchor) => {
						var span_4 = root_2$9();
						var text_7 = only_child(span_4, true);
						template_effect(($0) => set_text(text_7, $0), [() => countSays(get(c).tests)]);
						append($$anchor, span_4);
					};
					if_block(node_7, ($$render) => {
						if (get(c).tests) $$render(consequent_2);
					});
					var span_5 = sibling(node_7, 2);
					var text_8 = only_child(span_5, true);
					reset(div_1);
					var node_8 = sibling(div_1, 2);
					var consequent_3 = ($$anchor) => {
						var ul = root_4$9();
						each(ul, 20, () => get(c).failures, (f) => f, ($$anchor, f) => {
							var li_1 = root_3$9();
							var text_9 = only_child(li_1, true);
							template_effect(() => set_text(text_9, f));
							append($$anchor, li_1);
						});
						reset(ul);
						append($$anchor, ul);
					};
					if_block(node_8, ($$render) => {
						if (get(c).failures?.length) $$render(consequent_3);
					});
					var node_9 = sibling(node_8, 2);
					var consequent_4 = ($$anchor) => {
						var fragment_1 = root_5$8();
						var pre = first_child(fragment_1);
						var text_10 = only_child(pre, true);
						var span_6 = sibling(pre, 2);
						var text_11 = only_child(span_6);
						template_effect(() => {
							set_text(text_10, get(c).output_tail);
							set_text(text_11, `${get(c).output_bytes ?? "?" ?? ""} bytes · digest ${get(c).output_digest ?? "—" ?? ""}`);
						});
						append($$anchor, fragment_1);
					};
					if_block(node_9, ($$render) => {
						if (get(c).output_tail) $$render(consequent_4);
					});
					reset(li);
					template_effect(($0, $1) => {
						classes_1 = set_class(li, 1, "svelte-wqjyvl", null, classes_1, { cbad: !get(cok) });
						set_text(text_5, get(c).command);
						set_text(text_6, $0);
						set_text(text_8, $1);
					}, [() => exit(get(c).outcome), () => took(get(c).duration_ms)]);
					append($$anchor, li);
				});
				reset(ol);
				append($$anchor, ol);
			};
			if_block(node_5, ($$render) => {
				if (get(shown)) $$render(consequent_5);
			});
			reset(section);
			template_effect(($0, $1) => {
				classes = set_class(section, 1, "attempt svelte-wqjyvl", null, classes, {
					ok: get(ok),
					bad: !get(ok)
				});
				set_attribute(button, "aria-expanded", get(shown));
				set_text(text, get(g).gate);
				set_text(text_1, `attempt ${get(g).attempt ?? ""}`);
				set_text(text_2, get(ok) ? "passed" : "failed");
				set_text(text_3, `${get(g).commands.length ?? ""} commands · ${$0 ?? ""} · ${$1 ?? ""}`);
			}, [() => took(get(g).duration_ms), () => ago(get(g).at)]);
			delegated("click", button, () => set(open, {
				...get(open),
				[get(i)]: !get(shown)
			}, true));
			append($$anchor, section);
		});
		append($$anchor, fragment);
	};
	if_block(node, ($$render) => {
		if (get(attempts).length === 0) $$render(consequent);
		else $$render(alternate, -1);
	});
	var node_10 = sibling(node, 2);
	var consequent_22 = ($$anchor) => {
		var section_1 = root_28();
		var header = child(section_1);
		var h2 = child(header);
		var node_11 = child(h2);
		Icon(node_11, {
			name: "gate",
			size: 14
		});
		next();
		reset(h2);
		var node_12 = sibling(h2, 2);
		var consequent_6 = ($$anchor) => {
			var button_1 = root_9$5();
			var node_13 = child(button_1);
			Icon(node_13, {
				name: "play",
				size: 13
			});
			next();
			reset(button_1);
			template_effect(() => button_1.disabled = get(asking));
			delegated("click", button_1, runBaseChecks);
			append($$anchor, button_1);
		};
		if_block(node_12, ($$render) => {
			if (get(bc).can_run) $$render(consequent_6);
		});
		reset(header);
		var p_1 = sibling(header, 2);
		var text_12 = only_child(p_1, true);
		var node_14 = sibling(p_1, 2);
		each(node_14, 17, () => get(bc).junit_notes, index, ($$anchor, n) => {
			var p_2 = root_10$4();
			var text_13 = only_child(p_2, true);
			template_effect(() => set_text(text_13, get(n)));
			append($$anchor, p_2);
		});
		var node_15 = sibling(node_14, 2);
		var consequent_7 = ($$anchor) => {
			var p_3 = root_11$4();
			var text_14 = only_child(p_3, true);
			template_effect(() => set_text(text_14, get(asked)));
			append($$anchor, p_3);
		};
		if_block(node_15, ($$render) => {
			if (get(asked)) $$render(consequent_7);
		});
		var node_16 = sibling(node_15, 2);
		var consequent_8 = ($$anchor) => {
			var p_4 = root_12$4();
			var text_15 = only_child(p_4);
			template_effect(($0) => set_text(text_15, `These runs describe tree ${$0 ?? ""}, not this one.`), [() => get(bc).tree?.slice(0, 10)]);
			append($$anchor, p_4);
		};
		if_block(node_16, ($$render) => {
			if (get(bc).stale && get(bc).on_change) $$render(consequent_8);
		});
		var node_17 = sibling(node_16, 2);
		var consequent_15 = ($$anchor) => {
			var fragment_2 = root_21();
			var p_5 = sibling(first_child(fragment_2), 2);
			var text_16 = only_child(p_5, true);
			var node_18 = sibling(p_5, 2);
			var consequent_9 = ($$anchor) => {
				var p_6 = root_14$4();
				var node_19 = sibling(child(p_6));
				each(node_19, 17, () => get(bc).on_change.restored, index, ($$anchor, f) => {
					var fragment_3 = root_13$4();
					var code_2 = first_child(fragment_3);
					var text_17 = only_child(code_2, true);
					var text_18 = sibling(code_2, 1, true);
					text_18.nodeValue = " ";
					template_effect(() => set_text(text_17, get(f)));
					append($$anchor, fragment_3);
				});
				reset(p_6);
				append($$anchor, p_6);
			};
			if_block(node_18, ($$render) => {
				if (get(bc).on_change.restored.length) $$render(consequent_9);
			});
			var node_20 = sibling(node_18, 2);
			var consequent_10 = ($$anchor) => {
				var p_7 = root_15$1();
				var node_21 = sibling(child(p_7));
				each(node_21, 17, () => get(bc).on_change.removed, index, ($$anchor, f) => {
					var fragment_4 = root_13$4();
					var code_3 = first_child(fragment_4);
					var text_19 = only_child(code_3, true);
					var text_20 = sibling(code_3, 1, true);
					text_20.nodeValue = " ";
					template_effect(() => set_text(text_19, get(f)));
					append($$anchor, fragment_4);
				});
				reset(p_7);
				append($$anchor, p_7);
			};
			if_block(node_20, ($$render) => {
				if (get(bc).on_change.removed.length) $$render(consequent_10);
			});
			var node_22 = sibling(node_20, 2);
			each(node_22, 17, () => get(bc).on_change.junit_notes, index, ($$anchor, n) => {
				var p_8 = root_12$4();
				var text_21 = only_child(p_8, true);
				template_effect(() => set_text(text_21, get(n)));
				append($$anchor, p_8);
			});
			var node_23 = sibling(node_22, 2);
			each(node_23, 17, () => get(bc).on_change.commands, index, ($$anchor, c) => {
				var p_9 = root_16$1();
				var code_4 = child(p_9);
				var text_22 = only_child(code_4, true);
				var text_23 = sibling(code_4);
				reset(p_9);
				template_effect(($0, $1) => {
					set_text(text_22, get(c).command);
					set_text(text_23, ` ${$0 ?? ""} · ${$1 ?? ""}`);
				}, [() => exit(get(c).outcome), () => countSays(get(c).tests)]);
				append($$anchor, p_9);
			});
			var node_24 = sibling(node_23, 2);
			var consequent_12 = ($$anchor) => {
				var fragment_5 = root_19();
				var ul_1 = first_child(fragment_5);
				each(ul_1, 21, () => get(bc).on_change.failing, index, ($$anchor, n) => {
					var li_2 = root_17$1();
					var text_24 = only_child(li_2, true);
					template_effect(() => set_text(text_24, get(n)));
					append($$anchor, li_2);
				});
				reset(ul_1);
				var node_25 = sibling(ul_1, 2);
				var consequent_11 = ($$anchor) => {
					var p_10 = root_18$1();
					var text_25 = child(p_10);
					var code_5 = sibling(text_25);
					var text_26 = only_child(code_5);
					next();
					reset(p_10);
					template_effect(() => {
						set_text(text_25, `and ${get(bc).on_change.failing_more ?? ""} more — `);
						set_text(text_26, `devplane change show ${$$props.id ?? ""} --json`);
					});
					append($$anchor, p_10);
				};
				if_block(node_25, ($$render) => {
					if (get(bc).on_change.failing_more) $$render(consequent_11);
				});
				append($$anchor, fragment_5);
			};
			if_block(node_24, ($$render) => {
				if (get(bc).on_change.failing.length) $$render(consequent_12);
			});
			var node_26 = sibling(node_24, 2);
			var consequent_14 = ($$anchor) => {
				var fragment_6 = root_20();
				var p_11 = first_child(fragment_6);
				var text_27 = only_child(p_11, true);
				var ul_2 = sibling(p_11, 2);
				each(ul_2, 21, () => get(bc).on_change.named.not_run_on_change, index, ($$anchor, n) => {
					var li_3 = root_17$1();
					var text_28 = only_child(li_3, true);
					template_effect(() => set_text(text_28, get(n)));
					append($$anchor, li_3);
				});
				reset(ul_2);
				var node_27 = sibling(ul_2, 2);
				var consequent_13 = ($$anchor) => {
					var p_12 = root_18$1();
					var text_29 = child(p_12);
					var code_6 = sibling(text_29);
					var text_30 = only_child(code_6);
					next();
					reset(p_12);
					template_effect(() => {
						set_text(text_29, `and ${get(bc).on_change.named.not_run_on_change_more ?? ""} more — `);
						set_text(text_30, `devplane change show ${$$props.id ?? ""} --json`);
					});
					append($$anchor, p_12);
				};
				if_block(node_27, ($$render) => {
					if (get(bc).on_change.named.not_run_on_change_more) $$render(consequent_13);
				});
				template_effect(() => set_text(text_27, get(bc).on_change.named.says));
				append($$anchor, fragment_6);
			};
			if_block(node_26, ($$render) => {
				if (get(bc).on_change.named?.not_run_on_change.length) $$render(consequent_14);
			});
			template_effect(() => {
				set_class(p_5, 1, `w-${get(bc).on_change.weight ?? ""}`, "svelte-wqjyvl");
				set_text(text_16, get(bc).on_change.says);
			});
			append($$anchor, fragment_2);
		};
		if_block(node_17, ($$render) => {
			if (get(bc).on_change) $$render(consequent_15);
		});
		var node_28 = sibling(node_17, 2);
		var consequent_21 = ($$anchor) => {
			var fragment_7 = root_27();
			var p_13 = sibling(first_child(fragment_7), 2);
			var text_31 = only_child(p_13, true);
			var node_29 = sibling(p_13, 2);
			var consequent_17 = ($$anchor) => {
				var p_14 = root_24();
				var node_30 = sibling(child(p_14));
				each(node_30, 17, () => get(bc).on_base.placed, index, ($$anchor, f) => {
					var fragment_8 = root_23();
					var code_7 = first_child(fragment_8);
					var text_32 = only_child(code_7, true);
					var node_31 = sibling(code_7);
					var consequent_16 = ($$anchor) => {
						var span_7 = root_22();
						var text_33 = only_child(span_7);
						template_effect(($0) => set_text(text_33, `blob ${$0 ?? ""}`), [() => get(f).blob.slice(0, 10)]);
						append($$anchor, span_7);
					};
					if_block(node_31, ($$render) => {
						if (get(f).blob) $$render(consequent_16);
					});
					var text_34 = sibling(node_31, 1, true);
					text_34.nodeValue = " ";
					template_effect(() => set_text(text_32, get(f).path));
					append($$anchor, fragment_8);
				});
				reset(p_14);
				append($$anchor, p_14);
			};
			if_block(node_29, ($$render) => {
				if (get(bc).on_base.placed.length) $$render(consequent_17);
			});
			var node_32 = sibling(node_29, 2);
			var consequent_18 = ($$anchor) => {
				var p_15 = root_25();
				var node_33 = sibling(child(p_15));
				each(node_33, 17, () => get(bc).on_base.not_separable, index, ($$anchor, f) => {
					var fragment_9 = root_13$4();
					var code_8 = first_child(fragment_9);
					var text_35 = only_child(code_8, true);
					var text_36 = sibling(code_8, 1, true);
					text_36.nodeValue = " ";
					template_effect(() => set_text(text_35, get(f)));
					append($$anchor, fragment_9);
				});
				reset(p_15);
				append($$anchor, p_15);
			};
			if_block(node_32, ($$render) => {
				if (get(bc).on_base.not_separable.length) $$render(consequent_18);
			});
			var node_34 = sibling(node_32, 2);
			each(node_34, 17, () => get(bc).on_base.commands, index, ($$anchor, c) => {
				var p_16 = root_16$1();
				var code_9 = child(p_16);
				var text_37 = only_child(code_9, true);
				var text_38 = sibling(code_9);
				reset(p_16);
				template_effect(($0, $1) => {
					set_text(text_37, get(c).command);
					set_text(text_38, ` ${$0 ?? ""} · ${$1 ?? ""}`);
				}, [() => exit(get(c).outcome), () => countSays(get(c).tests)]);
				append($$anchor, p_16);
			});
			var node_35 = sibling(node_34, 2);
			var consequent_19 = ($$anchor) => {
				var fragment_10 = root_26();
				var ul_3 = sibling(first_child(fragment_10));
				each(ul_3, 21, () => get(bc).on_base.passing, index, ($$anchor, n) => {
					var li_4 = root_17$1();
					var text_39 = only_child(li_4, true);
					template_effect(() => set_text(text_39, get(n)));
					append($$anchor, li_4);
				});
				reset(ul_3);
				append($$anchor, fragment_10);
			};
			if_block(node_35, ($$render) => {
				if (get(bc).on_base.passing.length) $$render(consequent_19);
			});
			var node_36 = sibling(node_35, 2);
			var consequent_20 = ($$anchor) => {
				var p_17 = root_18$1();
				var text_40 = child(p_17);
				var code_10 = sibling(text_40);
				var text_41 = only_child(code_10);
				next();
				reset(p_17);
				template_effect(() => {
					set_text(text_40, `and ${get(bc).on_base.passing_more ?? ""} more — `);
					set_text(text_41, `devplane change show ${$$props.id ?? ""} --json`);
				});
				append($$anchor, p_17);
			};
			if_block(node_36, ($$render) => {
				if (get(bc).on_base.passing_more) $$render(consequent_20);
			});
			template_effect(() => {
				set_class(p_13, 1, `w-${get(bc).on_base.weight ?? ""}`, "svelte-wqjyvl");
				set_text(text_31, get(bc).on_base.says);
			});
			append($$anchor, fragment_7);
		};
		if_block(node_28, ($$render) => {
			if (get(bc).on_base) $$render(consequent_21);
		});
		reset(section_1);
		template_effect(() => set_text(text_12, get(bc).counts_says));
		append($$anchor, section_1);
	};
	if_block(node_10, ($$render) => {
		if (get(bc)) $$render(consequent_22);
	});
	var section_2 = sibling(node_10, 2);
	var header_1 = child(section_2);
	var h2_1 = child(header_1);
	var node_37 = child(h2_1);
	Icon(node_37, {
		name: "shield",
		size: 14
	});
	next();
	reset(h2_1);
	var node_38 = sibling(h2_1, 2);
	var consequent_23 = ($$anchor) => {
		var button_2 = root_29();
		var node_39 = child(button_2);
		Icon(node_39, {
			name: "file",
			size: 13
		});
		next();
		reset(button_2);
		delegated("click", button_2, copy);
		append($$anchor, button_2);
	};
	if_block(node_38, ($$render) => {
		if (get(cert)?.markdown) $$render(consequent_23);
	});
	reset(header_1);
	var node_40 = sibling(header_1, 2);
	var consequent_24 = ($$anchor) => {
		var p_18 = root_30();
		var node_41 = child(p_18);
		Inline(node_41, { get text() {
			return get(said);
		} });
		reset(p_18);
		append($$anchor, p_18);
	};
	if_block(node_40, ($$render) => {
		if (get(said)) $$render(consequent_24);
	});
	var node_42 = sibling(node_40, 2);
	var consequent_25 = ($$anchor) => {
		Failed($$anchor, {
			what: "the certificate",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_26 = ($$anchor) => {
		Skeleton($$anchor, { rows: 3 });
	};
	var alternate_2 = ($$anchor) => {
		var div_2 = root_36();
		each(div_2, 21, () => blocks(get(cert).markdown), index, ($$anchor, b) => {
			var fragment_13 = comment();
			var node_43 = first_child(fragment_13);
			var consequent_27 = ($$anchor) => {
				var pre_1 = root_31();
				var text_42 = only_child(pre_1, true);
				template_effect(() => set_text(text_42, get(b).text));
				append($$anchor, pre_1);
			};
			var consequent_28 = ($$anchor) => {
				var h3 = root_32();
				var node_44 = child(h3);
				Inline(node_44, { get segs() {
					return get(b).segs;
				} });
				reset(h3);
				append($$anchor, h3);
			};
			var consequent_29 = ($$anchor) => {
				var p_19 = root_33();
				var node_45 = sibling(child(p_19));
				Inline(node_45, { get segs() {
					return get(b).segs;
				} });
				reset(p_19);
				append($$anchor, p_19);
			};
			var consequent_30 = ($$anchor) => {
				var p_20 = root_34();
				var node_46 = child(p_20);
				Inline(node_46, { get segs() {
					return get(b).segs;
				} });
				reset(p_20);
				append($$anchor, p_20);
			};
			var alternate_1 = ($$anchor) => {
				var p_21 = root_35();
				var node_47 = child(p_21);
				Inline(node_47, { get segs() {
					return get(b).segs;
				} });
				reset(p_21);
				append($$anchor, p_21);
			};
			if_block(node_43, ($$render) => {
				if (get(b).kind === "pre") $$render(consequent_27);
				else if (get(b).kind === "h") $$render(consequent_28, 1);
				else if (get(b).kind === "li") $$render(consequent_29, 2);
				else if (get(b).kind === "row") $$render(consequent_30, 3);
				else $$render(alternate_1, -1);
			});
			append($$anchor, fragment_13);
		});
		reset(div_2);
		append($$anchor, div_2);
	};
	if_block(node_42, ($$render) => {
		if (!get(cert) && read.failure) $$render(consequent_25);
		else if (!get(cert)) $$render(consequent_26, 1);
		else $$render(alternate_2, -1);
	});
	reset(section_2);
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Ledger.svelte
var root$9 = from_html(`<button> <span class="svelte-6p9ro4"> </span></button>`);
var root_1$9 = from_html(`<button class="svelte-6p9ro4"> </button>`);
var root_2$8 = from_html(`<p class="quiet svelte-6p9ro4"> <code> </code> has every one.</p>`);
var root_3$8 = from_html(`<span> </span>`);
var root_4$8 = from_html(`<span class="why svelte-6p9ro4"> </span>`);
var root_5$7 = from_html(`<p class="quiet svelte-6p9ro4"> </p>`);
var root_6$6 = from_html(`<p><!></p>`);
var root_7$5 = from_html(`<aside class="detail svelte-6p9ro4"><b> </b> <pre class="svelte-6p9ro4"> </pre> <!></aside>`);
var root_8$4 = from_html(`<div class="frame svelte-6p9ro4"><!></div> <!>`, 1);
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
	var text$1 = only_child(span, true);
	reset(button);
	var node = sibling(button, 2);
	each(node, 17, () => get(chips), ([a, n]) => a, ($$anchor, $$item) => {
		var $$array = user_derived(() => to_array(get($$item), 2));
		let a = () => get($$array)[0];
		let n = () => get($$array)[1];
		var button_1 = root$9();
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
		var button_2 = root_1$9();
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
		var p = root_2$8();
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
		var fragment_1 = root_8$4();
		var div_2 = first_child(fragment_1);
		var node_4 = child(div_2);
		{
			const cell = ($$anchor, r = noop, c = noop) => {
				var fragment_2 = comment();
				var node_5 = first_child(fragment_2);
				var consequent_3 = ($$anchor) => {
					var span_2 = root_3$8();
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
					var p_1 = root_5$7();
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
			var aside = root_7$5();
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
		set_text(text$1, get(rows) ? get(rows).length : "");
	});
	delegated("click", button, () => set(only, null));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/ui/Md.svelte
var root$8 = from_html(`<pre class="svelte-ozut7t"> </pre>`);
var root_1$8 = from_html(`<p class="h svelte-ozut7t"><!></p>`);
var root_2$7 = from_html(`<p class="li svelte-ozut7t"><span aria-hidden="true">•</span> <!></p>`);
var root_3$7 = from_html(`<p class="row svelte-ozut7t"><!></p>`);
var root_4$7 = from_html(`<p class="svelte-ozut7t"><!></p>`);
var root_5$6 = from_html(`<div class="md svelte-ozut7t"></div>`);
function Md($$anchor, $$props) {
	push($$props, true);
	let text = prop($$props, "text", 3, "");
	const parts = user_derived(() => blocks(text()));
	var div = root_5$6();
	each(div, 21, () => get(parts), index, ($$anchor, b) => {
		var fragment = comment();
		var node = first_child(fragment);
		var consequent = ($$anchor) => {
			var pre = root$8();
			var text_1 = only_child(pre, true);
			template_effect(() => set_text(text_1, get(b).text));
			append($$anchor, pre);
		};
		var consequent_1 = ($$anchor) => {
			var p = root_1$8();
			var node_1 = child(p);
			Inline(node_1, { get segs() {
				return get(b).segs;
			} });
			reset(p);
			append($$anchor, p);
		};
		var consequent_2 = ($$anchor) => {
			var p_1 = root_2$7();
			var node_2 = sibling(child(p_1), 2);
			Inline(node_2, { get segs() {
				return get(b).segs;
			} });
			reset(p_1);
			append($$anchor, p_1);
		};
		var consequent_3 = ($$anchor) => {
			var p_2 = root_3$7();
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
var root$7 = from_html(`<p class="quiet svelte-1w0chr6">No agent has run on this change yet.</p>`);
var root_1$7 = from_html(`<span class="quiet svelte-1w0chr6"> </span>`);
var root_2$6 = from_html(`<div class="fail svelte-1w0chr6"><!></div>`);
var root_3$6 = from_html(`<li class="quiet svelte-1w0chr6" aria-busy="true">…</li>`);
var root_4$6 = from_html(`<li class="quiet svelte-1w0chr6">Nothing has been said yet.</li>`);
var root_5$5 = from_html(`<li class="quiet svelte-1w0chr6"> <code class="svelte-1w0chr6"> </code> has every one.</li>`);
var root_6$5 = from_html(`<li><span class="who svelte-1w0chr6"> </span> <div class="text svelte-1w0chr6"><!></div></li>`);
var root_7$4 = from_html(`<span class="warn svelte-1w0chr6"><!></span> <button type="button" class="danger svelte-1w0chr6">Stop the run</button> <button type="button" class="svelte-1w0chr6">Keep it running</button>`, 1);
var root_8$3 = from_html(`<button type="button" class="ghost svelte-1w0chr6"><!> Stop…</button>`);
var root_9$3 = from_html(`<p class="said svelte-1w0chr6" role="status"><!></p>`);
var root_10$3 = from_html(`<li class="svelte-1w0chr6"><!><code class="svelte-1w0chr6"> </code></li>`);
var root_11$3 = from_html(`<li class="quiet svelte-1w0chr6"> </li>`);
var root_12$3 = from_html(`<code class="svelte-1w0chr6"> </code>`);
var root_13$3 = from_html(`<li class="svelte-1w0chr6"><!><span> </span><!></li>`);
var root_14$3 = from_html(`<div class="agent svelte-1w0chr6"><section class="convo svelte-1w0chr6"><header class="bar svelte-1w0chr6"><!> <b> </b> <!> <!> <code class="quiet svelte-1w0chr6"> </code> <!></header> <!> <ol class="turns svelte-1w0chr6"><!> <!></ol> <form class="composer svelte-1w0chr6"><textarea rows="3" placeholder="Another turn for the agent — it is queued if one is under way" aria-label="another turn for the agent" class="svelte-1w0chr6"></textarea> <div class="send svelte-1w0chr6"><span class="quiet svelte-1w0chr6">⌘↵ to send</span> <!> <button type="submit" class="primary svelte-1w0chr6"><!> Send</button></div></form> <!></section> <aside class="record svelte-1w0chr6"><h2 class="svelte-1w0chr6">Files written <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul> <h2 class="svelte-1w0chr6">Running now <span class="svelte-1w0chr6"> </span></h2> <ul class="svelte-1w0chr6"></ul></aside></div>`);
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
		var p = root$7();
		append($$anchor, p);
	};
	var alternate_1 = ($$anchor) => {
		var div = root_14$3();
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
			var span = root_1$7();
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
			var span_1 = root_1$7();
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
			var div_1 = root_2$6();
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
			var div_2 = root_2$6();
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
			var li = root_3$6();
			append($$anchor, li);
		};
		var consequent_7 = ($$anchor) => {
			var li_1 = root_4$6();
			append($$anchor, li_1);
		};
		var consequent_8 = ($$anchor) => {
			var li_2 = root_5$5();
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
			var fragment_2 = root_7$4();
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
			var button_2 = root_8$3();
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
var root$6 = from_html(`<p class="dim svelte-rag4cy">No tasks were sent and no plan was reported.</p>`);
var root_1$6 = from_html(`<span class="where svelte-rag4cy"> </span>`);
var root_2$5 = from_html(`<span class="text svelte-rag4cy"> </span><!>`, 1);
var root_3$5 = from_html(`<span class="status svelte-rag4cy"> </span><span class="text svelte-rag4cy"> </span>`, 1);
var root_4$5 = from_html(`<li><span class="task svelte-rag4cy"><!></span> <span class="step svelte-rag4cy"><!></span></li>`);
var root_5$4 = from_html(`<p class="dim tally svelte-rag4cy"> </p>`);
var root_6$4 = from_html(`<ol class="rows svelte-rag4cy"></ol> <!>`, 1);
var root_7$3 = from_html(`<section class="plan" aria-label="tasks and the agent's plan"><div class="heads svelte-rag4cy"><h3 class="svelte-rag4cy">Tasks sent <span class="n svelte-rag4cy"> </span></h3> <h3 class="svelte-rag4cy">The agent's plan <span class="n svelte-rag4cy"> </span></h3></div> <!></section>`);
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
	var section = root_7$3();
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
		var p = root$6();
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
				var fragment_1 = root_2$5();
				var span_3 = first_child(fragment_1);
				var text_2 = only_child(span_3, true);
				var node_2 = sibling(span_3);
				var consequent_1 = ($$anchor) => {
					var span_4 = root_1$6();
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
				var fragment_2 = root_3$5();
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
			var p_1 = root_5$4();
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
var root$5 = from_html(`<li> </li>`);
var root_1$5 = from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> Ticked, but no run was sent them <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_2$4 = from_html(`<section class="note svelte-6i9c4j"><h3 class="svelte-6i9c4j">Sent, not ticked <span class="n svelte-6i9c4j"> </span></h3> <ul class="svelte-6i9c4j"></ul></section>`);
var root_3$4 = from_html(`<p class="counts svelte-6i9c4j"> </p> <!> <!>`, 1);
var root_4$4 = from_html(`<p class="quiet svelte-6i9c4j"><!></p>`);
var root_5$3 = from_html(`<div class="drift svelte-6i9c4j"><p class="svelte-6i9c4j"><!></p> <button>Tell the run</button> <button>Accept what it saw</button></div>`);
var root_6$3 = from_html(`<p class="said svelte-6i9c4j" role="status"> </p>`);
var root_7$2 = from_html(`<p class="said svelte-6i9c4j" role="status"><!></p>`);
var root_8$2 = from_html(`<section class="note wait svelte-6i9c4j"><h3 class="svelte-6i9c4j"><!> The specification changed under a run</h3> <!> <!></section>`);
var root_9$2 = from_html(`<tr><td class="mono svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td class="svelte-6i9c4j"> </td><td> </td></tr>`);
var root_10$2 = from_html(`<section><h2 class="svelte-6i9c4j">Requirements and the tasks that cite them</h2> <table class="svelte-6i9c4j"><thead><tr><th class="svelte-6i9c4j">requirement</th><th class="svelte-6i9c4j">tasks</th><th class="svelte-6i9c4j">ticked</th><th class="svelte-6i9c4j">seen by a passing check</th></tr></thead><tbody></tbody></table></section>`);
var root_11$2 = from_html(`<li><code class="svelte-6i9c4j"> </code> <!></li>`);
var root_12$2 = from_html(`<section><h2 class="svelte-6i9c4j">Every run</h2> <ul class="runs svelte-6i9c4j"></ul></section>`);
var root_13$2 = from_html(`<section><h2 class="svelte-6i9c4j"> </h2> <ol class="outline svelte-6i9c4j"></ol></section>`);
var root_14$2 = from_html(`<div class="tasks svelte-6i9c4j"><!> <!> <!> <section><h2 class="svelte-6i9c4j">Sent to the latest run</h2> <!> <!></section> <!> <!></div>`);
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
	var div = root_14$2();
	var node = child(div);
	var consequent_2 = ($$anchor) => {
		var fragment = root_3$4();
		var p = first_child(fragment);
		var text = only_child(p, true);
		var node_1 = sibling(p, 2);
		var consequent = ($$anchor) => {
			var section = root_1$5();
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
				var li = root$5();
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
			var section_1 = root_2$4();
			var h3_1 = child(section_1);
			var span_1 = sibling(child(h3_1));
			var text_3 = only_child(span_1, true);
			reset(h3_1);
			var ul_1 = sibling(h3_1, 2);
			each(ul_1, 20, () => $$props.d.counts.sent_unticked, (t) => t, ($$anchor, t) => {
				var li_1 = root$5();
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
		var section_2 = root_8$2();
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
			var div_1 = root_5$3();
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
			var p_4 = root_7$2();
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
			var li_3 = root$5();
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
var root$4 = from_html(`<span class="cov svelte-etvvkh"><!></span>`);
var root_1$4 = from_html(`<span> </span>`);
var root_2$3 = from_html(`<button class="file svelte-etvvkh"><!> <span class="name svelte-etvvkh"><span class="dir svelte-etvvkh"> </span><b class="svelte-etvvkh"> </b></span> <span class="delta svelte-etvvkh"><span class="add svelte-etvvkh"> </span> <span class="del svelte-etvvkh"> </span></span> <span class="meta svelte-etvvkh"><!> <!></span></button>`);
var root_3$3 = from_html(`<button><!> <span><!></span> <span class="n svelte-etvvkh"> </span></button> <!>`, 1);
var root_4$3 = from_html(`<nav class="files svelte-etvvkh" aria-label="files in the change"></nav>`);
function Files($$anchor, $$props) {
	push($$props, true);
	let folded = state(proxy({}));
	const split = (p) => {
		const i = p.lastIndexOf("/");
		return i === -1 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
	};
	const statusIcon = (s) => s.startsWith("added") ? "plus" : s.startsWith("deleted") ? "x" : "file";
	var nav = root_4$3();
	each(nav, 21, () => $$props.sections, (s) => s.says, ($$anchor, s) => {
		var fragment = root_3$3();
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
		var consequent_2 = ($$anchor) => {
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
				const done = user_derived(() => $$props.marked(get(f)));
				var button_1 = root_2$3();
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
					var span_8 = root$4();
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
					var span_9 = root_1$4();
					let classes;
					var text_5 = only_child(span_9);
					template_effect(() => {
						classes = set_class(span_9, 1, "seen svelte-etvvkh", null, classes, { all: get(done) === get(f).hunks.length });
						set_attribute(span_9, "title", `${get(done) ?? ""} of ${get(f).hunks.length ?? ""} hunks marked in this browser`);
						set_text(text_5, `${get(done) ?? ""}/${get(f).hunks.length ?? ""}`);
					});
					append($$anchor, span_9);
				};
				if_block(node_7, ($$render) => {
					if (get(f).hunks.length) $$render(consequent_1);
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
			if (!get(folded)[get(s).says]) $$render(consequent_2);
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
//#region src/surfaces/change/review/Diff.svelte
var root$3 = from_html(`<span title="marked in this browser"><!> marked</span>`);
var root_1$3 = from_html(`<button class="folded svelte-7nsom7"><!> </button>`);
var root_2$2 = from_html(`<tr><td class="no svelte-7nsom7"> </td><td class="no svelte-7nsom7"> </td><td class="sg svelte-7nsom7"> </td><td class="tx svelte-7nsom7"> </td></tr>`);
var root_3$2 = from_html(`<table class="code unified svelte-7nsom7"><tbody></tbody></table>`);
var root_4$2 = from_html(`<tr><td> </td><td> </td><td> </td><td> </td><td> </td><td> </td></tr>`);
var root_5$2 = from_html(`<table class="code split svelte-7nsom7"><tbody></tbody></table>`);
var root_6$2 = from_html(`<section><button class="hh svelte-7nsom7"><code class="svelte-7nsom7"> </code> <!></button> <!></section>`);
function Diff($$anchor, $$props) {
	push($$props, true);
	let mode = prop($$props, "mode", 3, "unified"), current = prop($$props, "current", 19, () => -1), expanded = prop($$props, "expanded", 3, false);
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
	var fragment = comment();
	var node = first_child(fragment);
	each(node, 19, () => $$props.hunks, (h, i) => h.header + i, ($$anchor, h, i) => {
		const m = user_derived(() => $$props.mark(get(h)));
		var section = root_6$2();
		let classes;
		var button = child(section);
		var code = child(button);
		var text_1 = only_child(code, true);
		var node_1 = sibling(code, 2);
		var consequent = ($$anchor) => {
			var span = root$3();
			var node_2 = child(span);
			Icon(node_2, {
				name: "eye",
				size: 12
			});
			next();
			reset(span);
			template_effect(() => set_class(span, 1, `mark ${get(m) ?? ""}`, "svelte-7nsom7"));
			append($$anchor, span);
		};
		if_block(node_1, ($$render) => {
			if (get(m)) $$render(consequent);
		});
		reset(button);
		var node_3 = sibling(button, 2);
		var consequent_1 = ($$anchor) => {
			var button_1 = root_1$3();
			var node_4 = child(button_1);
			Icon(node_4, {
				name: "right",
				size: 12
			});
			var text_2 = sibling(node_4);
			reset(button_1);
			template_effect(() => set_text(text_2, ` Formatting only — ${get(h).lines.length ?? ""} lines whose text is unchanged apart from whitespace. Show them.`));
			delegated("click", button_1, function(...$$args) {
				$$props.onexpand?.apply(this, $$args);
			});
			append($$anchor, button_1);
		};
		var consequent_2 = ($$anchor) => {
			var table = root_3$2();
			var tbody = child(table);
			each(tbody, 21, () => numbered(get(h)), index, ($$anchor, l) => {
				var tr = root_2$2();
				var td = child(tr);
				var text_3 = only_child(td, true);
				var td_1 = sibling(td);
				var text_4 = only_child(td_1, true);
				var td_2 = sibling(td_1);
				var text_5 = only_child(td_2, true);
				var td_3 = sibling(td_2);
				var text_6 = only_child(td_3, true);
				reset(tr);
				template_effect(($0) => {
					set_class(tr, 1, clsx(get(l).kind), "svelte-7nsom7");
					set_text(text_3, get(l).old ?? "");
					set_text(text_4, get(l).new ?? "");
					set_text(text_5, $0);
					set_text(text_6, get(l).text);
				}, [() => sign(get(l).kind)]);
				append($$anchor, tr);
			});
			reset(tbody);
			reset(table);
			append($$anchor, table);
		};
		var alternate = ($$anchor) => {
			var table_1 = root_5$2();
			var tbody_1 = child(table_1);
			each(tbody_1, 21, () => paired(numbered(get(h))), index, ($$anchor, p) => {
				var tr_1 = root_4$2();
				var td_4 = child(tr_1);
				var text_7 = only_child(td_4, true);
				var td_5 = sibling(td_4);
				var text_8 = only_child(td_5, true);
				var td_6 = sibling(td_5);
				var text_9 = only_child(td_6, true);
				var td_7 = sibling(td_6);
				var text_10 = only_child(td_7, true);
				var td_8 = sibling(td_7);
				var text_11 = only_child(td_8, true);
				var td_9 = sibling(td_8);
				var text_12 = only_child(td_9, true);
				reset(tr_1);
				template_effect(($0, $1) => {
					set_class(td_4, 1, `no ${get(p).left?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_7, get(p).left?.old ?? "");
					set_class(td_5, 1, `sg ${get(p).left?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_8, $0);
					set_class(td_6, 1, `tx ${get(p).left?.kind === "removed" ? "removed" : get(p).left ? "context" : "none"}`, "svelte-7nsom7");
					set_text(text_9, get(p).left?.text ?? "");
					set_class(td_7, 1, `no ${get(p).right?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_10, get(p).right?.new ?? "");
					set_class(td_8, 1, `sg ${get(p).right?.kind ?? "none" ?? ""}`, "svelte-7nsom7");
					set_text(text_11, $1);
					set_class(td_9, 1, `tx ${get(p).right?.kind === "added" ? "added" : get(p).right ? "context" : "none"}`, "svelte-7nsom7");
					set_text(text_12, get(p).right?.text ?? "");
				}, [() => get(p).left ? sign(get(p).left.kind) : "", () => get(p).right ? sign(get(p).right.kind) : ""]);
				append($$anchor, tr_1);
			});
			reset(tbody_1);
			reset(table_1);
			append($$anchor, table_1);
		};
		if_block(node_3, ($$render) => {
			if (get(h).formatter_only && !expanded()) $$render(consequent_1);
			else if (mode() === "unified") $$render(consequent_2, 1);
			else $$render(alternate, -1);
		});
		reset(section);
		template_effect(() => {
			classes = set_class(section, 1, "hunk svelte-7nsom7", null, classes, { current: get(i) === current() });
			set_attribute(section, "id", `hunk-${get(i) ?? ""}`);
			set_text(text_1, get(h).header);
		});
		delegated("click", button, () => $$props.onpick?.(get(i)));
		append($$anchor, section);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/review/Pane.svelte
var root$2 = from_html(`<div class="stale svelte-15l7e6g"><!></div>`);
var root_1$2 = from_html(`<p class="finding svelte-15l7e6g"><!> <span><!></span></p>`);
var root_2$1 = from_html(`<div class="tree svelte-15l7e6g"><!></div>`);
var root_3$1 = from_html(`<span class="wrow svelte-15l7e6g"><!> <code class="matched svelte-15l7e6g"> </code> <span class="seen svelte-15l7e6g"> </span></span>`);
var root_4$1 = from_html(`<button class="read svelte-15l7e6g"><!> I have read this</button>`);
var root_5$1 = from_html(`<dt class="weak svelte-15l7e6g">Check weakened</dt> <dd class="weak svelte-15l7e6g"><!> <!></dd>`, 1);
var root_6$1 = from_html(`<dt class="svelte-15l7e6g">Decided while writing</dt><dd class="svelte-15l7e6g"> </dd>`, 1);
var root_7$1 = from_html(`<code class="cmd svelte-15l7e6g"> </code>`);
var root_8$1 = from_html(`<dt class="svelte-15l7e6g">Check it yourself</dt><dd class="svelte-15l7e6g"></dd>`, 1);
var root_9$1 = from_html(`<form class="fix svelte-15l7e6g"><textarea rows="3" aria-label="a message to the latest run" class="svelte-15l7e6g"></textarea> <div class="svelte-15l7e6g"><button type="submit" class="primary svelte-15l7e6g"> </button> <button type="button">Cancel <kbd class="svelte-15l7e6g">Esc</kbd></button></div></form>`);
var root_10$1 = from_html(`<p class="said svelte-15l7e6g" role="status"><!></p>`);
var root_11$1 = from_html(`<p class="finding svelte-15l7e6g"><!></p>`);
var root_12$1 = from_html(`<header class="fh svelte-15l7e6g"><div class="path svelte-15l7e6g"><code class="svelte-15l7e6g"> </code> <span class="st svelte-15l7e6g"><!></span> <span class="add svelte-15l7e6g"> </span> <span class="del svelte-15l7e6g"> </span></div> <dl class="why svelte-15l7e6g"><!> <dt class="svelte-15l7e6g">Role</dt><dd class="svelte-15l7e6g"><!></dd> <dt class="svelte-15l7e6g">Covered by</dt><dd><!></dd> <dt class="svelte-15l7e6g">Asked for</dt><dd><!></dd> <!> <!></dl> <div class="acts svelte-15l7e6g"><button title="marks this hunk in this browser; a weakened line inside it is recorded as read" class="svelte-15l7e6g"><!> Mark <kbd class="svelte-15l7e6g">s</kbd></button> <button class="svelte-15l7e6g"><!> Request a fix <kbd class="svelte-15l7e6g">f</kbd></button> <span class="hint svelte-15l7e6g"><kbd class="svelte-15l7e6g">j</kbd>/<kbd class="svelte-15l7e6g">k</kbd> hunks · <kbd class="svelte-15l7e6g">n</kbd>/<kbd class="svelte-15l7e6g">p</kbd> files</span></div> <!> <!> <!></header> <!> <!>`, 1);
var root_13$1 = from_html(`<div class="diff svelte-15l7e6g"><!></div>`);
var root_14$1 = from_html(`<div class="review svelte-15l7e6g"><header class="bar svelte-15l7e6g"><span class="base">against <code class="svelte-15l7e6g"> </code></span> <span class="shape"> </span> <span class="gap svelte-15l7e6g"></span> <span class="progress svelte-15l7e6g" title="hunks not marked in this browser, and weakened rows not recorded as read"> </span> <div class="seg svelte-15l7e6g" role="group" aria-label="order"><button>By risk</button> <button>By intent</button></div> <div class="seg svelte-15l7e6g" role="group" aria-label="diff layout"><button title="unified"><!></button> <button title="side by side"><!></button></div></header> <!> <!> <div class="body svelte-15l7e6g"><!></div></div>`);
function Pane($$anchor, $$props) {
	push($$props, true);
	let planted = prop($$props, "review", 3, null), focus = prop($$props, "focus", 3, ""), source = prop($$props, "source", 3, null);
	const key = user_derived(() => $$props.id);
	const own = resource(() => get(key) && !source() ? `/api/changes/${encodeURIComponent(get(key))}/review` : null, { tell: () => `devplane change review ${get(key)}` });
	const read = user_derived(() => source() ?? own);
	const r = user_derived(() => get(read).data ?? planted());
	const unreadNow = user_derived(() => unreadSays(unread(get(r))));
	function pref(key, dflt) {
		try {
			return localStorage.getItem(key) || dflt;
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
	const byPath = user_derived(() => new Map(get(files).map((f) => [f.path, f])));
	const sections = user_derived(() => {
		if (!get(r)) return [];
		if (get(by) === "risk") return get(r).groups.map((g) => ({
			says: g.says,
			files: g.files,
			tone: g.weakened ? "fail" : void 0
		}));
		const out = get(r).intent.groups.map((g) => ({
			says: `${g.title}`,
			files: g.files.map((p) => get(byPath).get(p)).filter((f) => !!f)
		}));
		if (get(r).intent.not_asked_for.length) out.push({
			says: get(r).intent.not_asked_for_heading,
			files: get(r).intent.not_asked_for.map((n) => get(byPath).get(n.path)).filter((f) => !!f),
			tone: "wait"
		});
		return out;
	});
	const order = user_derived(() => get(sections).flatMap((s) => s.files.map((f) => f.path)));
	let picked = state("");
	const selected = user_derived(() => get(order).includes(get(picked)) ? get(picked) : get(order)[0] ?? "");
	const weakRows = user_derived(() => {
		const m = /* @__PURE__ */ new Map();
		for (const w of (get(r)?.groups ?? []).flatMap((g) => g.weakened ?? [])) m.set(w.path, [...m.get(w.path) ?? [], w]);
		return m;
	});
	user_effect(() => {
		if (focus()) set(picked, focus());
	});
	let seenSaid = state("");
	async function markRead(rows) {
		const todo = rows.filter((w) => !w.seen);
		if (!todo.length || !$$props.id) return;
		try {
			for (const w of todo) await api(`/api/changes/${encodeURIComponent($$props.id)}/review/seen`, {
				method: "POST",
				body: JSON.stringify({
					path: w.path,
					matched: w.matched
				})
			});
			set(seenSaid, `Marked read, as yours: ${todo.length === 1 ? todo[0].path : `${todo.length} rows`}.`);
			await get(read).reload();
		} catch (e) {
			set(seenSaid, `That did not land: ${failure(e).says}`);
		}
	}
	const file = user_derived(() => get(byPath).get(get(selected)) ?? null);
	let current = state(0);
	user_effect(() => {
		get(selected);
		set(current, 0);
	});
	let expanded = state(proxy(/* @__PURE__ */ new Set()));
	const keyOf = (f, h) => markKey(get(r)?.change ?? "", f.path, h.header, h.lines);
	let marks = state(proxy({}));
	user_effect(() => {
		const next = {};
		const live = /* @__PURE__ */ new Set();
		for (const f of get(files)) for (const h of f.hunks) {
			const k = keyOf(f, h);
			live.add(k);
			const m = readMark(k);
			if (m) next[k] = m;
		}
		if (get(r)?.change && get(files).length && !get(r).truncated_says) prune(get(r).change, live);
		set(marks, next, true);
	});
	const markOf = (f) => (h) => get(marks)[keyOf(f, h)] ?? null;
	const inHunk = (h, w) => w.kind === "skip" && h.lines.some(([k, t]) => k !== "context" && t.trim() === w.matched);
	const marked = (f) => f.hunks.filter((h) => markOf(f)(h)).length;
	function setMark(m) {
		const h = get(file)?.hunks[get(current)];
		if (!get(file) || !h) return;
		const k = keyOf(get(file), h);
		set(marks, {
			...get(marks),
			[k]: m
		}, true);
		writeMark(k, m);
		markRead((get(weakRows).get(get(file).path) ?? []).filter((w) => inHunk(h, w)));
		move(1);
	}
	function reveal() {
		queueMicrotask(() => document.getElementById(`hunk-${get(current)}`)?.scrollIntoView({
			block: "nearest",
			behavior: "smooth"
		}));
	}
	function move(step) {
		if (!get(file)) return;
		const next = get(current) + step;
		if (next >= 0 && next < get(file).hunks.length) {
			set(current, next);
			reveal();
			return;
		}
		const at = get(order).indexOf(get(selected)) + step;
		if (at >= 0 && at < get(order).length) {
			set(picked, get(order)[at], true);
			queueMicrotask(() => {
				set(current, step === 1 ? 0 : Math.max(0, (get(byPath).get(get(order)[at])?.hunks.length ?? 1) - 1), true);
				reveal();
			});
		}
	}
	function moveFile(step) {
		const at = get(order).indexOf(get(selected)) + step;
		if (at >= 0 && at < get(order).length) set(picked, get(order)[at], true);
	}
	let fixing = state(false);
	let draft = state("");
	let said = state("");
	const sending = writer();
	function requestFix() {
		const h = get(file)?.hunks[get(current)];
		if (!get(file) || !h) return;
		set(draft, `In \`${get(file).path}\` ${h.header}:\n`);
		set(fixing, true);
	}
	async function sendFix() {
		const run = get(r)?.latest_run;
		if (!run || !get(draft).trim()) return;
		await sending.run("Sending to the agent", async () => {
			try {
				const res = await api(`/api/runs/${encodeURIComponent(run)}/prompt`, {
					method: "POST",
					body: JSON.stringify({ text: get(draft).trim() })
				});
				set(said, res.says ?? "Sent to the agent.", true);
				set(fixing, false);
				await get(read).reload();
			} catch (e) {
				set(said, `That did not land: ${failure(e).says}`);
			}
		});
	}
	function fixKey(e) {
		if (e.key === "Escape") {
			e.preventDefault();
			e.stopPropagation();
			set(fixing, false);
		} else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			sendFix();
		}
	}
	user_effect(() => {
		const offs = [
			onAction("hunk-next", () => (move(1), true)),
			onAction("hunk-prev", () => (move(-1), true)),
			onAction("file-next", () => (moveFile(1), true)),
			onAction("file-prev", () => (moveFile(-1), true)),
			onAction("hunk-seen", () => (setMark("seen"), true)),
			onAction("hunk-fix", () => (requestFix(), true)),
			onAction("hunk-expand", () => (get(file) && set(expanded, /* @__PURE__ */ new Set([...get(expanded), get(file).path]), true), true)),
			onAction("tab-risk", () => (set(by, "risk"), true)),
			onAction("tab-intent", () => (set(by, "intent"), true)),
			onAction("diff-mode", () => (set(mode, get(mode) === "unified" ? "split" : "unified", true), true))
		];
		return () => offs.forEach((off) => off());
	});
	var fragment = comment();
	var node = first_child(fragment);
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
		var div = root_14$1();
		var header = child(div);
		var span = child(header);
		var code = sibling(child(span));
		var text = only_child(code, true);
		reset(span);
		var span_1 = sibling(span, 2);
		var text_1 = only_child(span_1, true);
		var span_2 = sibling(span_1, 4);
		var text_2 = only_child(span_2, true);
		var div_1 = sibling(span_2, 2);
		var button = child(div_1);
		let classes;
		var button_1 = sibling(button, 2);
		let classes_1;
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		var button_2 = child(div_2);
		let classes_2;
		var node_1 = child(button_2);
		Icon(node_1, {
			name: "unified",
			size: 14
		});
		reset(button_2);
		var button_3 = sibling(button_2, 2);
		let classes_3;
		var node_2 = child(button_3);
		Icon(node_2, {
			name: "split",
			size: 14
		});
		reset(button_3);
		reset(div_2);
		reset(header);
		var node_3 = sibling(header, 2);
		var consequent_3 = ($$anchor) => {
			var div_3 = root$2();
			var node_4 = child(div_3);
			Failed(node_4, {
				what: "the review",
				get failure() {
					return get(read).failure;
				},
				get at() {
					return get(read).at;
				},
				stale: true
			});
			reset(div_3);
			append($$anchor, div_3);
		};
		if_block(node_3, ($$render) => {
			if (get(read).phase === "stale" && get(read).failure) $$render(consequent_3);
		});
		var node_5 = sibling(node_3, 2);
		each(node_5, 17, () => [
			get(r).unordered,
			get(r).coverage_absent,
			get(r).truncated_says,
			get(by) === "intent" ? get(r).intent.unavailable : null
		], index, ($$anchor, s) => {
			var fragment_4 = comment();
			var node_6 = first_child(fragment_4);
			var consequent_4 = ($$anchor) => {
				var p_1 = root_1$2();
				var node_7 = child(p_1);
				Icon(node_7, {
					name: "alert",
					size: 13
				});
				var span_3 = sibling(node_7, 2);
				var node_8 = child(span_3);
				Inline(node_8, { get text() {
					return get(s);
				} });
				reset(span_3);
				reset(p_1);
				append($$anchor, p_1);
			};
			if_block(node_6, ($$render) => {
				if (get(s)) $$render(consequent_4);
			});
			append($$anchor, fragment_4);
		});
		var div_4 = sibling(node_5, 2);
		var node_9 = child(div_4);
		{
			const pane = ($$anchor) => {
				var div_5 = root_2$1();
				Files(child(div_5), {
					get sections() {
						return get(sections);
					},
					get selected() {
						return get(selected);
					},
					marked,
					pick: (p) => set(picked, p, true)
				});
				reset(div_5);
				append($$anchor, div_5);
			};
			Split(node_9, {
				id: "review-files",
				size: 300,
				min: 200,
				max: 520,
				pane,
				children: ($$anchor, $$slotProps) => {
					var div_6 = root_13$1();
					var node_11 = child(div_6);
					var consequent_13 = ($$anchor) => {
						var fragment_5 = root_12$1();
						var header_1 = first_child(fragment_5);
						var div_7 = child(header_1);
						var code_1 = child(div_7);
						var text_3 = only_child(code_1, true);
						var span_4 = sibling(code_1, 2);
						var node_12 = child(span_4);
						Inline(node_12, { get text() {
							return get(file).status_says;
						} });
						reset(span_4);
						var span_5 = sibling(span_4, 2);
						var text_4 = only_child(span_5);
						var span_6 = sibling(span_5, 2);
						var text_5 = only_child(span_6);
						reset(div_7);
						var dl = sibling(div_7, 2);
						var node_13 = child(dl);
						var consequent_6 = ($$anchor) => {
							var fragment_6 = root_5$1();
							var dd = sibling(first_child(fragment_6), 2);
							var node_14 = child(dd);
							each(node_14, 17, () => get(weakRows).get(get(file).path) ?? [], (w) => w.why + w.matched, ($$anchor, w) => {
								var span_7 = root_3$1();
								var node_15 = child(span_7);
								Inline(node_15, { get text() {
									return get(w).why;
								} });
								var code_2 = sibling(node_15, 2);
								var text_6 = only_child(code_2, true);
								var span_8 = sibling(code_2, 2);
								var text_7 = only_child(span_8, true);
								reset(span_7);
								template_effect(() => {
									set_text(text_6, get(w).matched);
									set_text(text_7, get(w).seen ? "marked read" : "not yet read");
								});
								append($$anchor, span_7);
							});
							var node_16 = sibling(node_14, 2);
							var consequent_5 = ($$anchor) => {
								var button_4 = root_4$1();
								var node_17 = child(button_4);
								Icon(node_17, {
									name: "eye",
									size: 13
								});
								next();
								reset(button_4);
								delegated("click", button_4, () => markRead(get(weakRows).get(get(file).path) ?? []));
								append($$anchor, button_4);
							};
							var d = user_derived(() => (get(weakRows).get(get(file).path) ?? []).some((w) => !w.seen));
							if_block(node_16, ($$render) => {
								if (get(d)) $$render(consequent_5);
							});
							reset(dd);
							append($$anchor, fragment_6);
						};
						var d_1 = user_derived(() => get(weakRows).get(get(file).path));
						if_block(node_13, ($$render) => {
							if (get(d_1)) $$render(consequent_6);
						});
						var dd_1 = sibling(node_13, 3);
						var node_18 = child(dd_1);
						Inline(node_18, { get text() {
							return get(file).role_says;
						} });
						reset(dd_1);
						var dd_2 = sibling(dd_1, 3);
						let classes_4;
						var node_19 = child(dd_2);
						{
							let $0 = user_derived(() => get(file).coverage_says ?? "no mapping declared");
							Inline(node_19, { get text() {
								return get($0);
							} });
						}
						reset(dd_2);
						var dd_3 = sibling(dd_2, 3);
						let classes_5;
						var node_20 = child(dd_3);
						Inline(node_20, { get text() {
							return get(file).task_says;
						} });
						reset(dd_3);
						var node_21 = sibling(dd_3, 2);
						var consequent_7 = ($$anchor) => {
							var fragment_7 = root_6$1();
							var dd_4 = sibling(first_child(fragment_7));
							var text_8 = only_child(dd_4, true);
							template_effect(($0) => set_text(text_8, $0), [() => get(file).decisions.map((x) => x.says).join(" · ")]);
							append($$anchor, fragment_7);
						};
						if_block(node_21, ($$render) => {
							if (get(file).decisions.length) $$render(consequent_7);
						});
						var node_22 = sibling(node_21, 2);
						var consequent_8 = ($$anchor) => {
							var fragment_8 = root_8$1();
							var dd_5 = sibling(first_child(fragment_8));
							each(dd_5, 20, () => get(file).marker_commands, (c) => c, ($$anchor, c) => {
								var code_3 = root_7$1();
								var text_9 = only_child(code_3, true);
								template_effect(() => set_text(text_9, c));
								append($$anchor, code_3);
							});
							reset(dd_5);
							append($$anchor, fragment_8);
						};
						if_block(node_22, ($$render) => {
							if (get(file).marker_commands.length) $$render(consequent_8);
						});
						reset(dl);
						var div_8 = sibling(dl, 2);
						var button_5 = child(div_8);
						var node_23 = child(button_5);
						Icon(node_23, {
							name: "eye",
							size: 13
						});
						next(2);
						reset(button_5);
						var button_6 = sibling(button_5, 2);
						var node_24 = child(button_6);
						Icon(node_24, {
							name: "agent",
							size: 13
						});
						next(2);
						reset(button_6);
						next(2);
						reset(div_8);
						var node_25 = sibling(div_8, 2);
						var consequent_9 = ($$anchor) => {
							var form = root_9$1();
							var textarea = child(form);
							remove_textarea_child(textarea);
							autofocus(textarea, true);
							var div_9 = sibling(textarea, 2);
							var button_7 = child(div_9);
							var text_10 = only_child(button_7, true);
							var button_8 = sibling(button_7, 2);
							reset(div_9);
							reset(form);
							template_effect(() => {
								button_7.disabled = !!sending.busy;
								set_text(text_10, sending.busy ? `${sending.busy}… ${sending.elapsed}` : "Send to the agent");
							});
							event("submit", form, (e) => {
								e.preventDefault();
								sendFix();
							});
							delegated("keydown", textarea, fixKey);
							bind_value(textarea, () => get(draft), ($$value) => set(draft, $$value));
							delegated("click", button_8, () => set(fixing, false));
							append($$anchor, form);
						};
						if_block(node_25, ($$render) => {
							if (get(fixing)) $$render(consequent_9);
						});
						var node_26 = sibling(node_25, 2);
						var consequent_10 = ($$anchor) => {
							var p_2 = root_10$1();
							var node_27 = child(p_2);
							Inline(node_27, { get text() {
								return get(said);
							} });
							reset(p_2);
							append($$anchor, p_2);
						};
						if_block(node_26, ($$render) => {
							if (get(said)) $$render(consequent_10);
						});
						var node_28 = sibling(node_26, 2);
						var consequent_11 = ($$anchor) => {
							var p_3 = root_10$1();
							var node_29 = child(p_3);
							Inline(node_29, { get text() {
								return get(seenSaid);
							} });
							reset(p_3);
							append($$anchor, p_3);
						};
						if_block(node_28, ($$render) => {
							if (get(seenSaid)) $$render(consequent_11);
						});
						reset(header_1);
						var node_30 = sibling(header_1, 2);
						var consequent_12 = ($$anchor) => {
							var p_4 = root_11$1();
							var node_31 = child(p_4);
							Inline(node_31, { get text() {
								return get(file).body_says;
							} });
							reset(p_4);
							append($$anchor, p_4);
						};
						if_block(node_30, ($$render) => {
							if (get(file).body_says) $$render(consequent_12);
						});
						var node_32 = sibling(node_30, 2);
						{
							let $0 = user_derived(() => get(expanded).has(get(file).path));
							let $1 = user_derived(() => markOf(get(file)));
							Diff(node_32, {
								get hunks() {
									return get(file).hunks;
								},
								get mode() {
									return get(mode);
								},
								get current() {
									return get(current);
								},
								get expanded() {
									return get($0);
								},
								get mark() {
									return get($1);
								},
								onexpand: () => set(expanded, /* @__PURE__ */ new Set([...get(expanded), get(file).path]), true),
								onpick: (i) => set(current, i, true)
							});
						}
						template_effect(($0) => {
							set_text(text_3, get(file).path);
							set_text(text_4, `+${get(file).added ?? ""}`);
							set_text(text_5, `−${get(file).removed ?? ""}`);
							classes_4 = set_class(dd_2, 1, "svelte-15l7e6g", null, classes_4, { quiet: get(file).coverage?.coverage !== "covered" });
							classes_5 = set_class(dd_3, 1, "svelte-15l7e6g", null, classes_5, { wait: $0 });
							button_6.disabled = !get(r).latest_run;
						}, [() => get(file).task_says.startsWith("not asked")]);
						delegated("click", button_5, () => setMark("seen"));
						delegated("click", button_6, requestFix);
						append($$anchor, fragment_5);
					};
					if_block(node_11, ($$render) => {
						if (get(file)) $$render(consequent_13);
					});
					reset(div_6);
					append($$anchor, div_6);
				},
				$$slots: {
					pane: true,
					default: true
				}
			});
		}
		reset(div_4);
		reset(div);
		template_effect(($0) => {
			set_text(text, get(r).base);
			set_text(text_1, $0);
			set_text(text_2, get(unreadNow));
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
	append($$anchor, fragment);
	pop();
}
delegate(["click", "keydown"]);
//#endregion
//#region src/surfaces/change/Refusal.svelte
var root$1 = from_html(`<li class="svelte-1wz92j1"><code class="svelte-1wz92j1"> </code> <span class="svelte-1wz92j1"><!></span> <button class="svelte-1wz92j1"><!> Read it</button></li>`);
var root_1$1 = from_html(`<div class="refusal svelte-1wz92j1" role="alert"><p class="svelte-1wz92j1"><b>Not offered</b> — <!>. Nothing was pushed.</p> <ul class="svelte-1wz92j1"></ul> <p class="how svelte-1wz92j1">Mark each row read in the review, or from a terminal of your own: <code class="svelte-1wz92j1"> </code></p></div>`);
function Refusal($$anchor, $$props) {
	push($$props, true);
	var div = root_1$1();
	var p = child(div);
	var node = sibling(child(p), 2);
	Inline(node, { get text() {
		return $$props.refusal.says;
	} });
	next();
	reset(p);
	var ul = sibling(p, 2);
	each(ul, 21, () => $$props.refusal.rows, (row) => row.path + row.matched, ($$anchor, row) => {
		var li = root$1();
		var code = child(li);
		var text = only_child(code, true);
		var span = sibling(code, 2);
		var node_1 = child(span);
		Inline(node_1, { get text() {
			return get(row).why;
		} });
		reset(span);
		var button = sibling(span, 2);
		var node_2 = child(button);
		Icon(node_2, {
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
	var text_1 = only_child(code_1, true);
	reset(p_1);
	reset(div);
	template_effect(() => set_text(text_1, $$props.refusal.seen_with));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/change/Doc.svelte
var root = from_html(`<div class="loading svelte-178dv07"><!></div>`);
var root_1 = from_html(`<div class="failed svelte-178dv07"><!></div>`);
var root_2 = from_html(`<button class="chip mono svelte-178dv07" title="copy the branch name"><!> </button>`);
var root_3 = from_html(`<span class="chip mono svelte-178dv07"><!> </span>`);
var root_4 = from_html(`<span class="chip warn svelte-178dv07"><!> in place — no parallel safety</span>`);
var root_5 = from_html(`<span class="chip svelte-178dv07"> </span>`);
var root_6 = from_html(`<span><!></span>`);
var root_7 = from_html(`<span> </span>`);
var root_8 = from_html(`<p class="counts svelte-178dv07"> <!></p>`);
var root_9 = from_html(`<span class="count svelte-178dv07"> </span>`);
var root_10 = from_html(`<button class="svelte-178dv07"><!> Run gates</button>`);
var root_11 = from_html(`<button disabled="" aria-label="Offer as pull request" aria-describedby="offer-held" class="svelte-178dv07"><!> Offer</button> <span class="held svelte-178dv07" id="offer-held"> </span>`, 1);
var root_12 = from_html(`<button><!> Offer as pull request</button>`);
var root_13 = from_html(`<button class="svelte-178dv07"><!> Try again</button>`);
var root_14 = from_html(`<button class="svelte-178dv07"><!> Pick it back up</button> <!>`, 1);
var root_15 = from_html(`<button class="svelte-178dv07"><!> Finish</button>`);
var root_16 = from_html(`<button class="quiet svelte-178dv07" title="open the worktree in your editor" aria-label="Editor"><!> <span class="label svelte-178dv07">Editor</span></button> <button class="quiet svelte-178dv07" title="open a terminal in the worktree" aria-label="Terminal"><!> <span class="label svelte-178dv07">Terminal</span></button>`, 1);
var root_17 = from_html(`<button class="quiet svelte-178dv07" title="remove the worktree; the branch and the record are kept" aria-label="Archive"><!> <span class="label svelte-178dv07">Archive</span></button>`);
var root_18 = from_html(`<article class="doc svelte-178dv07"><header class="head svelte-178dv07"><div class="line1 svelte-178dv07"><h1 class="svelte-178dv07"> </h1> <!> <!> <span class="gap svelte-178dv07"></span> <!></div> <div class="chips svelte-178dv07"><span class="chip svelte-178dv07"><!> </span> <!> <!> <!> <!> <!></div> <!> <div role="toolbar" aria-label="what you can do to this change"><button><!> Review<!></button> <!> <!> <!> <!> <span class="gap svelte-178dv07"></span> <!> <!></div> <p role="status" aria-live="polite"><!></p> <!> <!> <!></header> <div class="tabbar svelte-178dv07"><!></div> <div class="view svelte-178dv07" role="tabpanel"><!></div></article>`);
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
	const view = user_derived(() => VIEWS.includes(asked()) ? asked() : get(local));
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
	const unreadNow = user_derived(() => unreadSays(unread(get(review), get(unseen))));
	const uncovered = user_derived(() => !!get(d)?.tree_now && get(d).tree_now.clean === false && !!get(d).gate?.ran_at?.tree && get(d).gate.ran_at.tree !== get(d).tree_now.tree);
	const standing = user_derived(() => get(d)?.standing_says && !(get(verified) && (get(leads) || get(uncovered))) ? get(d).standing_says : "");
	function readRow(path) {
		set(reviewAt, path, true);
		setView("review");
	}
	const lastRun = user_derived(() => get(d)?.runs?.[get(d).runs.length - 1] ?? "");
	const project = user_derived(() => (get(d)?.project_id ?? "").split("/").filter(Boolean).pop() ?? "");
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
		if (verb === "offer" && r?.offer !== "opened") return "Nothing was pushed — [github] pull_request is not set, so the pull request is yours to open. Run:";
		if (verb === "verify" && typeof r?.passed === "boolean") return `${r.passed ? "The gates exited zero" : "The gates failed"}${r.summary ? `: ${r.summary}` : "."}`;
		return (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "");
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
	const offerHeld = user_derived(() => get(verified) && get(unseen) > 0 ? `Read ${get(unseen)} weakened ${get(unseen) === 1 ? "check" : "checks"} first` : "");
	user_effect(() => {
		if (!get(d)) return;
		return offerCommands(get(d).title || get(d).id, [
			{
				label: `Review${get(unreadNow) ? ` — ${get(unreadNow)}` : ""}`,
				icon: "split",
				run: () => setView("review")
			},
			...get(d).worktree ? [{
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
	var alternate_3 = ($$anchor) => {
		var article = root_18();
		var header = child(article);
		var div_3 = child(header);
		var h1 = child(div_3);
		var text_1 = only_child(h1, true);
		var node_5 = sibling(h1, 2);
		ChangeState(node_5, {
			get state() {
				return get(d).state;
			},
			get q() {
				return get(d).qualifier;
			}
		});
		var node_6 = sibling(node_5, 2);
		var consequent_4 = ($$anchor) => {
			Pill($$anchor, {
				get word() {
					return get(d).waiting_says;
				},
				as: "wait"
			});
		};
		if_block(node_6, ($$render) => {
			if (get(d).waiting_says) $$render(consequent_4);
		});
		var node_7 = sibling(node_6, 4);
		{
			let $0 = user_derived(() => !!get(d).archived_at);
			let $1 = user_derived(() => !!get(d).pull_request);
			Stepper(node_7, {
				get state() {
					return get(d).state;
				},
				get archived() {
					return get($0);
				},
				get offered() {
					return get($1);
				},
				get weakUnread() {
					return get(unseen);
				}
			});
		}
		reset(div_3);
		var div_4 = sibling(div_3, 2);
		var span = child(div_4);
		var node_8 = child(span);
		Icon(node_8, {
			name: "folder",
			size: 12
		});
		var text_2 = sibling(node_8);
		reset(span);
		var node_9 = sibling(span, 2);
		var consequent_5 = ($$anchor) => {
			var button = root_2();
			var node_10 = child(button);
			Icon(node_10, {
				name: "change",
				size: 12
			});
			var text_3 = sibling(node_10);
			reset(button);
			template_effect(() => set_text(text_3, ` ${get(d).branch ?? ""}`));
			delegated("click", button, () => copy(get(d).branch, "the branch name"));
			append($$anchor, button);
		};
		if_block(node_9, ($$render) => {
			if (get(d).branch) $$render(consequent_5);
		});
		var node_11 = sibling(node_9, 2);
		var consequent_6 = ($$anchor) => {
			var span_1 = root_3();
			var node_12 = child(span_1);
			Icon(node_12, {
				name: "spec",
				size: 12
			});
			var text_4 = sibling(node_12);
			reset(span_1);
			template_effect(() => {
				set_attribute(span_1, "title", get(d).spec);
				set_text(text_4, ` ${get(d).spec ?? ""}`);
			});
			append($$anchor, span_1);
		};
		if_block(node_11, ($$render) => {
			if (get(d).spec) $$render(consequent_6);
		});
		var node_13 = sibling(node_11, 2);
		var consequent_7 = ($$anchor) => {
			var span_2 = root_4();
			var node_14 = child(span_2);
			Icon(node_14, {
				name: "alert",
				size: 12
			});
			next();
			reset(span_2);
			append($$anchor, span_2);
		};
		if_block(node_13, ($$render) => {
			if (get(d).in_place) $$render(consequent_7);
		});
		var node_15 = sibling(node_13, 2);
		var consequent_8 = ($$anchor) => {
			var span_3 = root_5();
			var text_5 = only_child(span_3, true);
			template_effect(($0) => set_text(text_5, $0), [() => shape(get(d).shape_says)]);
			append($$anchor, span_3);
		};
		if_block(node_15, ($$render) => {
			if (get(d).shape_says) $$render(consequent_8);
		});
		var node_16 = sibling(node_15, 2);
		var consequent_9 = ($$anchor) => {
			var span_4 = root_6();
			var node_17 = child(span_4);
			Inline(node_17, { get text() {
				return get(standing);
			} });
			reset(span_4);
			template_effect(() => set_class(span_4, 1, `standing ${get(verified) && !get(leads) ? "done" : ""}`, "svelte-178dv07"));
			append($$anchor, span_4);
		};
		if_block(node_16, ($$render) => {
			if (get(standing)) $$render(consequent_9);
		});
		reset(div_4);
		var node_18 = sibling(div_4, 2);
		var consequent_11 = ($$anchor) => {
			var p = root_8();
			var text_6 = child(p, true);
			var node_19 = sibling(text_6);
			var consequent_10 = ($$anchor) => {
				var span_5 = root_7();
				var text_7 = only_child(span_5);
				template_effect(() => {
					set_class(span_5, 1, `w-${get(d).base_checks.on_base.weight ?? ""}`, "svelte-178dv07");
					set_text(text_7, `· ${get(d).base_checks.on_base.says ?? ""}`);
				});
				append($$anchor, span_5);
			};
			if_block(node_19, ($$render) => {
				if (get(d).base_checks.on_base && !get(d).base_checks.stale) $$render(consequent_10);
			});
			reset(p);
			template_effect(() => set_text(text_6, get(d).base_checks.counts_says));
			append($$anchor, p);
		};
		if_block(node_18, ($$render) => {
			if (get(d).base_checks) $$render(consequent_11);
		});
		var div_5 = sibling(node_18, 2);
		let classes;
		var button_1 = child(div_5);
		let classes_1;
		var node_20 = child(button_1);
		Icon(node_20, {
			name: "split",
			size: 14
		});
		var node_21 = sibling(node_20, 2);
		var consequent_12 = ($$anchor) => {
			var span_6 = root_9();
			var text_8 = only_child(span_6, true);
			template_effect(() => set_text(text_8, get(unreadNow)));
			append($$anchor, span_6);
		};
		if_block(node_21, ($$render) => {
			if (get(unreadNow)) $$render(consequent_12);
		});
		reset(button_1);
		var node_22 = sibling(button_1, 2);
		var consequent_13 = ($$anchor) => {
			var button_2 = root_10();
			var node_23 = child(button_2);
			Icon(node_23, {
				name: "gate",
				size: 14
			});
			next();
			reset(button_2);
			template_effect(() => button_2.disabled = !!pending.busy);
			delegated("click", button_2, () => act("verify"));
			append($$anchor, button_2);
		};
		if_block(node_22, ($$render) => {
			if (get(d).worktree) $$render(consequent_13);
		});
		var node_24 = sibling(node_22, 2);
		var consequent_15 = ($$anchor) => {
			var fragment_4 = comment();
			var node_25 = first_child(fragment_4);
			var consequent_14 = ($$anchor) => {
				var fragment_5 = root_11();
				var button_3 = first_child(fragment_5);
				var node_26 = child(button_3);
				Icon(node_26, {
					name: "forge",
					size: 14
				});
				next();
				reset(button_3);
				var span_7 = sibling(button_3, 2);
				var text_9 = only_child(span_7, true);
				template_effect(() => set_text(text_9, get(offerHeld)));
				append($$anchor, fragment_5);
			};
			var alternate_1 = ($$anchor) => {
				var button_4 = root_12();
				let classes_2;
				var node_27 = child(button_4);
				Icon(node_27, {
					name: "forge",
					size: 14
				});
				next();
				reset(button_4);
				template_effect(() => {
					button_4.disabled = !!pending.busy;
					classes_2 = set_class(button_4, 1, "svelte-178dv07", null, classes_2, { primary: get(unseen) === 0 });
				});
				delegated("click", button_4, () => act("offer"));
				append($$anchor, button_4);
			};
			if_block(node_25, ($$render) => {
				if (get(offerHeld)) $$render(consequent_14);
				else $$render(alternate_1, -1);
			});
			append($$anchor, fragment_4);
		};
		if_block(node_24, ($$render) => {
			if (get(verified)) $$render(consequent_15);
		});
		var node_28 = sibling(node_24, 2);
		var consequent_17 = ($$anchor) => {
			var fragment_6 = root_14();
			var button_5 = first_child(fragment_6);
			var node_29 = child(button_5);
			Icon(node_29, {
				name: "play",
				size: 14
			});
			next();
			reset(button_5);
			var node_30 = sibling(button_5, 2);
			var consequent_16 = ($$anchor) => {
				var button_6 = root_13();
				var node_31 = child(button_6);
				Icon(node_31, {
					name: "refresh",
					size: 14
				});
				next();
				reset(button_6);
				template_effect(() => button_6.disabled = !!pending.busy);
				delegated("click", button_6, () => act("retry"));
				append($$anchor, button_6);
			};
			if_block(node_30, ($$render) => {
				if (get(d).can_retry) $$render(consequent_16);
			});
			template_effect(() => button_5.disabled = !!pending.busy);
			delegated("click", button_5, () => act("resume"));
			append($$anchor, fragment_6);
		};
		if_block(node_28, ($$render) => {
			if (get(d).stopped) $$render(consequent_17);
		});
		var node_32 = sibling(node_28, 2);
		var consequent_18 = ($$anchor) => {
			var button_7 = root_15();
			var node_33 = child(button_7);
			Icon(node_33, {
				name: "check",
				size: 14
			});
			next();
			reset(button_7);
			template_effect(() => button_7.disabled = !!pending.busy);
			delegated("click", button_7, () => act("finish"));
			append($$anchor, button_7);
		};
		if_block(node_32, ($$render) => {
			if (!get(d).completion && !get(d).archived_at && get(d).runs?.length) $$render(consequent_18);
		});
		var node_34 = sibling(node_32, 4);
		var consequent_19 = ($$anchor) => {
			var fragment_7 = root_16();
			var button_8 = first_child(fragment_7);
			var node_35 = child(button_8);
			Icon(node_35, {
				name: "external",
				size: 14
			});
			next(2);
			reset(button_8);
			var button_9 = sibling(button_8, 2);
			var node_36 = child(button_9);
			Icon(node_36, {
				name: "terminal",
				size: 14
			});
			next(2);
			reset(button_9);
			delegated("click", button_8, () => openIn("editor"));
			delegated("click", button_9, () => openIn("terminal"));
			append($$anchor, fragment_7);
		};
		if_block(node_34, ($$render) => {
			if (get(d).worktree) $$render(consequent_19);
		});
		var node_37 = sibling(node_34, 2);
		var consequent_20 = ($$anchor) => {
			var button_10 = root_17();
			var node_38 = child(button_10);
			Icon(node_38, {
				name: "folder",
				size: 14
			});
			next(2);
			reset(button_10);
			template_effect(() => button_10.disabled = !!pending.busy);
			delegated("click", button_10, () => act("archive"));
			append($$anchor, button_10);
		};
		if_block(node_37, ($$render) => {
			if (!get(d).archived_at && (get(d).completion || get(d).stopped)) $$render(consequent_20);
		});
		reset(div_5);
		var p_1 = sibling(div_5, 2);
		let classes_3;
		var node_39 = child(p_1);
		var consequent_21 = ($$anchor) => {
			var text_10 = text();
			template_effect(() => set_text(text_10, `${DOING[pending.busy] ?? ""} · ${pending.elapsed ?? ""} — nothing else can be started on this change until it is back.`));
			append($$anchor, text_10);
		};
		var alternate_2 = ($$anchor) => {
			Inline($$anchor, { get text() {
				return get(said);
			} });
		};
		if_block(node_39, ($$render) => {
			if (pending.busy) $$render(consequent_21);
			else $$render(alternate_2, -1);
		});
		reset(p_1);
		var node_40 = sibling(p_1, 2);
		var consequent_22 = ($$anchor) => {
			Commands($$anchor, { get commands() {
				return get(commands);
			} });
		};
		if_block(node_40, ($$render) => {
			if (get(commands).length && !pending.busy) $$render(consequent_22);
		});
		var node_41 = sibling(node_40, 2);
		var consequent_23 = ($$anchor) => {
			Failed($$anchor, {
				what: "this change",
				get failure() {
					return read.failure;
				},
				get at() {
					return read.at;
				},
				stale: true
			});
		};
		if_block(node_41, ($$render) => {
			if (read.phase === "stale" && read.failure) $$render(consequent_23);
		});
		var node_42 = sibling(node_41, 2);
		var consequent_24 = ($$anchor) => {
			Refusal($$anchor, {
				get refusal() {
					return get(refusal);
				},
				read: readRow
			});
		};
		if_block(node_42, ($$render) => {
			if (get(refusal)) $$render(consequent_24);
		});
		reset(header);
		var div_6 = sibling(header, 2);
		var node_43 = child(div_6);
		var bind_get = () => get(view);
		var bind_set = setView;
		Tabs(node_43, {
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
		var node_44 = child(div_7);
		var consequent_25 = ($$anchor) => {
			Overview($$anchor, {
				get d() {
					return get(d);
				},
				go: setView
			});
		};
		var consequent_26 = ($$anchor) => {
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
		var consequent_27 = ($$anchor) => {
			Pane($$anchor, {
				get id() {
					return get(key);
				},
				get focus() {
					return get(reviewAt);
				},
				get source() {
					return reviewRead;
				},
				get review() {
					return plantedReview();
				}
			});
		};
		var consequent_28 = ($$anchor) => {
			Gates($$anchor, {
				get d() {
					return get(d);
				},
				get id() {
					return get(key);
				},
				reload: () => read.reload()
			});
		};
		var consequent_29 = ($$anchor) => {
			Ledger($$anchor, { get id() {
				return get(key);
			} });
		};
		var consequent_30 = ($$anchor) => {
			Agent($$anchor, {
				get d() {
					return get(d);
				},
				get run() {
					return get(lastRun);
				}
			});
		};
		if_block(node_44, ($$render) => {
			if (get(view) === "overview") $$render(consequent_25);
			else if (get(view) === "tasks") $$render(consequent_26, 1);
			else if (get(view) === "review") $$render(consequent_27, 2);
			else if (get(view) === "gates") $$render(consequent_28, 3);
			else if (get(view) === "ledger") $$render(consequent_29, 4);
			else if (get(view) === "agent") $$render(consequent_30, 5);
		});
		reset(div_7);
		reset(article);
		template_effect(($0) => {
			set_attribute(h1, "title", get(d).title || get(d).id);
			set_text(text_1, get(d).title || get(d).id);
			set_text(text_2, ` ${get(project) ?? ""}`);
			classes = set_class(div_5, 1, "toolbar svelte-178dv07", null, classes, { "held-offer": !!get(offerHeld) });
			classes_1 = set_class(button_1, 1, "svelte-178dv07", null, classes_1, { primary: get(unseen) > 0 || !get(verified) });
			classes_3 = set_class(p_1, 1, "said svelte-178dv07", null, classes_3, { empty: !get(said) && !pending.busy });
			set_attribute(div_7, "aria-label", $0);
		}, [() => get(tabs).find((t) => t.id === get(view))?.label]);
		delegated("click", button_1, () => setView("review"));
		append($$anchor, article);
	};
	if_block(node, ($$render) => {
		if (!id()) $$render(consequent_1);
		else if (read.failure && !get(d)) $$render(consequent_2, 1);
		else if (!get(d)) $$render(consequent_3, 2);
		else $$render(alternate_3, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
export { Doc as default };

//# sourceMappingURL=Doc.js.map