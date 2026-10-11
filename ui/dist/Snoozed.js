import { $ as user_effect, B as append, G as delegate, H as from_html, J as get, K as delegated, L as if_block, P as each, Q as template_effect, V as comment, at as proxy, ct as user_derived, dt as pop, ft as push, g as prop, ht as reset, i as api, it as sibling, nt as first_child, ot as set, pt as snapshot, rt as only_child, st as state, tt as child, w as set_attribute, z as set_text } from "./api.js";
import { i as same, n as failure } from "./resource.svelte.js";
import { f as reread, n as Icon } from "./app.js";
import { r as bringBack } from "./actions.js";
//#region src/surfaces/inbox/narrow.svelte.ts
var EVERY_MS = 2e3;
var shared = proxy({
	project: "",
	data: null,
	failure: null,
	at: null
});
var inflight = 0;
var seq = 0;
var landed = 0;
var users = 0;
var timer = null;
async function read(project, force = false) {
	if (inflight > 0 && !force) return;
	const n = ++seq;
	inflight += 1;
	try {
		const r = await api(`/api/inbox?project=${encodeURIComponent(project)}`);
		if (shared.project !== project || n < landed) return;
		landed = n;
		if (!same(snapshot(shared.data), r)) shared.data = r;
		shared.failure = null;
		shared.at = Date.now();
	} catch (e) {
		if (shared.project !== project || n < landed) return;
		landed = n;
		shared.failure = failure(e, `devplane inbox --project ${project}`);
	} finally {
		inflight = Math.max(0, inflight - 1);
	}
}
function narrowed(project) {
	const key = user_derived(() => project().trim());
	user_effect(() => {
		const want = get(key);
		if (!want) return;
		if (shared.project !== want) {
			shared.project = want;
			shared.data = null;
			shared.failure = null;
			shared.at = null;
			landed = seq;
		}
		users += 1;
		if (users === 1 || shared.data === null) read(want);
		if (timer === null) timer = setInterval(() => {
			if (!document.hidden && shared.project) read(shared.project);
		}, EVERY_MS);
		return () => {
			users -= 1;
			if (users === 0 && timer !== null) {
				clearInterval(timer);
				timer = null;
			}
		};
	});
	const mine = () => !!get(key) && shared.project === get(key);
	return {
		get on() {
			return !!get(key);
		},
		get data() {
			return mine() ? shared.data : null;
		},
		get failure() {
			return mine() ? shared.failure : null;
		},
		get at() {
			return mine() ? shared.at : null;
		},
		get phase() {
			const d = mine() ? shared.data : null;
			const f = mine() ? shared.failure : null;
			return d ? f ? "stale" : "ok" : f ? "failed" : "loading";
		},
		get missing() {
			return mine() && shared.data?.narrowed?.no_such_project === true;
		},
		reload: () => get(key) ? read(get(key), true) : Promise.resolve()
	};
}
//#endregion
//#region src/surfaces/inbox/Snoozed.svelte
var root = from_html(`<li class="svelte-1lpcje5"><span class="title svelte-1lpcje5"> </span> <span class="says svelte-1lpcje5"> </span> <button class="svelte-1lpcje5">bring back</button></li>`);
var root_1 = from_html(`<ul id="snoozed-rows" aria-label="snoozed" class="svelte-1lpcje5"></ul>`);
var root_2 = from_html(`<p class="said svelte-1lpcje5" role="status"> </p>`);
var root_3 = from_html(`<div class="snoozed svelte-1lpcje5"><button class="toggle svelte-1lpcje5" aria-controls="snoozed-rows"><!> <span class="svelte-1lpcje5"> </span></button> <!> <!></div>`);
function Snoozed($$anchor, $$props) {
	push($$props, true);
	let count = prop($$props, "count", 3, 0), rows = prop($$props, "rows", 19, () => []), whole = prop($$props, "whole", 3, false);
	let open = state(false);
	let said = state("");
	let busy = state(false);
	async function back(row) {
		set(busy, true);
		set(said, (await bringBack(row)).said, true);
		set(busy, false);
		reread();
	}
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_2 = ($$anchor) => {
		var div = root_3();
		var button = child(div);
		var node_1 = child(button);
		Icon(node_1, {
			name: "clock",
			size: 13
		});
		var text = sibling(node_1);
		var span = sibling(text);
		var text_1 = only_child(span, true);
		reset(button);
		var node_2 = sibling(button, 2);
		var consequent = ($$anchor) => {
			var ul = root_1();
			each(ul, 21, rows, (r) => r.id, ($$anchor, r) => {
				var li = root();
				var span_1 = child(li);
				var text_2 = only_child(span_1, true);
				var span_2 = sibling(span_1, 2);
				var text_3 = only_child(span_2);
				var button_1 = sibling(span_2, 2);
				reset(li);
				template_effect(($0) => {
					set_text(text_2, get(r).title);
					set_text(text_3, `${$0 ?? ""} · ${get(r).says ?? ""}`);
					button_1.disabled = get(busy);
				}, [() => get(r).kind.replace(/_/g, " ")]);
				delegated("click", button_1, () => back(get(r)));
				append($$anchor, li);
			});
			reset(ul);
			append($$anchor, ul);
		};
		if_block(node_2, ($$render) => {
			if (get(open)) $$render(consequent);
		});
		var node_3 = sibling(node_2, 2);
		var consequent_1 = ($$anchor) => {
			var p = root_2();
			var text_4 = only_child(p, true);
			template_effect(() => set_text(text_4, get(said)));
			append($$anchor, p);
		};
		if_block(node_3, ($$render) => {
			if (get(said)) $$render(consequent_1);
		});
		reset(div);
		template_effect(() => {
			set_attribute(button, "aria-expanded", get(open));
			set_text(text, ` ${count() ?? ""} snoozed${whole() ? " across the inbox" : ""} `);
			set_text(text_1, get(open) ? "hide" : "list them");
		});
		delegated("click", button, () => set(open, !get(open)));
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (count() > 0) $$render(consequent_2);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
export { narrowed as n, Snoozed as t };

//# sourceMappingURL=Snoozed.js.map