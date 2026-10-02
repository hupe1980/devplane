import { $ as user_effect, B as append, C as remove_input_defaults, D as set_selected, E as init_select, F as index, G as delegate, H as from_html, J as get, K as delegated, L as if_block, P as each, Q as template_effect, S as bind_value, T as bind_select_value, V as comment, W as text, X as untrack, _t as to_array, at as proxy, ct as user_derived, dt as pop, f as run, ft as push, g as prop, gt as noop, ht as reset, i as api, it as sibling, k as set_class, mt as next, nt as first_child, ot as set, q as event, rt as only_child, st as state, t as Refused, tt as child, ut as remove_textarea_child, w as set_attribute, x as bind_checked, z as set_text } from "./api.js";
import { r as resource, t as copyText } from "./resource.svelte.js";
import { a as Inline, g as skippedSteps, h as keepSkipped, i as Skeleton, m as incomplete, n as Icon, p as TITLES, s as ago, t as Pill, w as go } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Failed } from "./Failed.js";
import { t as safeHref } from "./href.js";
import { t as Grid } from "./Grid.js";
import { t as Props } from "./Props.js";
//#region src/surfaces/setup/GitHubPanel.svelte
function hostSays(h) {
	switch (h.state) {
		case "signed_in": return `signed in as ${h.login ?? "?"}${h.scopes?.length ? ` · ${h.scopes.join(", ")}` : ""}`;
		case "pending": return "waiting for the code to be entered at GitHub";
		case "expired": return "sign-in expired — GitHub no longer accepts the token, so it was removed";
		case "rate_limited": return `signed in; the rate limit is spent until ${h.until ?? "GitHub's reset"}`;
		case "unreachable": return `signed in; GitHub unreachable${h.why ? ` — ${h.why}` : ""}`;
		default: return "not signed in";
	}
}
var root$6 = from_html(`<button>Sign out</button>`);
var root_1$7 = from_html(`<button>Sign in</button>`);
var root_2$7 = from_html(`<p class="said svelte-t93vvx"> </p>`);
var root_3$7 = from_html(`<p class="quiet svelte-t93vvx"> <code class="svelte-t93vvx"> </code> signs in with GitHub CLI's token, or pipe a fine-grained personal access token to the same command.</p>`);
var root_4$7 = from_html(`<div class="code svelte-t93vvx"><p class="svelte-t93vvx">Open <a target="_blank" rel="noopener noreferrer"> </a> and enter:</p> <p class="big svelte-t93vvx"> </p> <button>Copy code</button></div>`);
var root_5$7 = from_html(`<div class="host svelte-t93vvx"><div class="line svelte-t93vvx"><b> </b> <span> </span> <!></div> <!> <!> <!></div>`);
var root_6$7 = from_html(`<p class="quiet svelte-t93vvx"> </p>`);
var root_7$7 = from_html(`<p class="said svelte-t93vvx"><!></p>`);
var root_8$6 = from_html(`<section class="card wide svelte-t93vvx" id="github"><h2 class="svelte-t93vvx"><!> GitHub</h2> <!> <!> <!> <p class="quiet svelte-t93vvx">The token is kept only in this machine's credential store — never in a file, a log or this page.</p></section>`);
function GitHubPanel($$anchor, $$props) {
	push($$props, true);
	let hosts = prop($$props, "hosts", 19, () => []), clientId = prop($$props, "clientId", 3, true), busy = prop($$props, "busy", 3, ""), said = prop($$props, "said", 3, ""), signIn = prop($$props, "signIn", 3, () => {}), signOut = prop($$props, "signOut", 3, () => {}), copy = prop($$props, "copy", 3, () => {});
	var section = root_8$6();
	var h2 = child(section);
	var node = child(h2);
	Icon(node, {
		name: "forge",
		size: 14
	});
	next();
	reset(h2);
	var node_1 = sibling(h2, 2);
	each(node_1, 17, hosts, (h) => h.host, ($$anchor, h) => {
		var div = root_5$7();
		var div_1 = child(div);
		var b = child(div_1);
		var text = only_child(b, true);
		var span = sibling(b, 2);
		let classes;
		var text_1 = only_child(span, true);
		var node_2 = sibling(span, 2);
		var consequent = ($$anchor) => {
			var button = root$6();
			template_effect(() => button.disabled = !!busy());
			delegated("click", button, () => signOut()(get(h).host));
			append($$anchor, button);
		};
		var consequent_1 = ($$anchor) => {
			var button_1 = root_1$7();
			template_effect(() => button_1.disabled = !!busy() || !(get(h).device_flow ?? clientId()));
			delegated("click", button_1, () => signIn()(get(h).host));
			append($$anchor, button_1);
		};
		if_block(node_2, ($$render) => {
			if (get(h).state === "signed_in" || get(h).state === "rate_limited" || get(h).state === "unreachable") $$render(consequent);
			else if (get(h).state !== "pending") $$render(consequent_1, 1);
		});
		reset(div_1);
		var node_3 = sibling(div_1, 2);
		var consequent_2 = ($$anchor) => {
			var p = root_2$7();
			var text_2 = only_child(p, true);
			template_effect(() => set_text(text_2, get(h).said));
			append($$anchor, p);
		};
		if_block(node_3, ($$render) => {
			if (get(h).said) $$render(consequent_2);
		});
		var node_4 = sibling(node_3, 2);
		var consequent_3 = ($$anchor) => {
			var p_1 = root_3$7();
			var text_3 = child(p_1);
			var code = sibling(text_3);
			var text_4 = only_child(code);
			next();
			reset(p_1);
			template_effect(() => {
				set_text(text_3, `No GitHub app is registered for ${get(h).host ?? ""}, so the window cannot start a sign-in. In a terminal, `);
				set_text(text_4, `gh auth token | devplane auth login --with-token${get(h).host === "github.com" ? "" : ` --host ${get(h).host}`}`);
			});
			append($$anchor, p_1);
		};
		if_block(node_4, ($$render) => {
			if (!(get(h).device_flow ?? clientId()) && get(h).state !== "signed_in") $$render(consequent_3);
		});
		var node_5 = sibling(node_4, 2);
		var consequent_4 = ($$anchor) => {
			var div_2 = root_4$7();
			var p_2 = child(div_2);
			var a = sibling(child(p_2));
			var text_5 = only_child(a, true);
			next();
			reset(p_2);
			var p_3 = sibling(p_2, 2);
			var text_6 = only_child(p_3, true);
			var button_2 = sibling(p_3, 2);
			reset(div_2);
			template_effect(($0) => {
				set_attribute(a, "href", $0);
				set_text(text_5, get(h).verification_uri);
				set_text(text_6, get(h).user_code);
			}, [() => safeHref(get(h).verification_uri) ?? "#setup"]);
			delegated("click", button_2, () => copy()(get(h).user_code ?? ""));
			append($$anchor, div_2);
		};
		if_block(node_5, ($$render) => {
			if (get(h).state === "pending" && get(h).user_code) $$render(consequent_4);
		});
		reset(div);
		template_effect(($0) => {
			set_attribute(div, "data-state", get(h).state);
			set_text(text, get(h).host);
			classes = set_class(span, 1, "svelte-t93vvx", null, classes, {
				ok: get(h).state === "signed_in",
				warn: get(h).state !== "signed_in"
			});
			set_text(text_1, $0);
		}, [() => hostSays(get(h))]);
		append($$anchor, div);
	}, ($$anchor) => {
		Skeleton($$anchor, { rows: 2 });
	});
	var node_6 = sibling(node_1, 2);
	var consequent_5 = ($$anchor) => {
		var p_4 = root_6$7();
		var text_7 = only_child(p_4);
		template_effect(() => set_text(text_7, `${busy() ?? ""}…`));
		append($$anchor, p_4);
	};
	if_block(node_6, ($$render) => {
		if (busy()) $$render(consequent_5);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_6 = ($$anchor) => {
		var p_5 = root_7$7();
		var node_8 = child(p_5);
		Inline(node_8, { get text() {
			return said();
		} });
		reset(p_5);
		append($$anchor, p_5);
	};
	if_block(node_7, ($$render) => {
		if (said()) $$render(consequent_6);
	});
	next(2);
	reset(section);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/lib/PendingWrite.svelte
var root$5 = from_html(`<span class="tag svelte-14cazgd">a new file</span>`);
var root_1$6 = from_html(`<span class="tag svelte-14cazgd">removed</span>`);
var root_2$6 = from_html(`<span class="tag warn svelte-14cazgd">has uncommitted changes — the diff is against the file as it is on disk</span>`);
var root_3$6 = from_html(`<p class="quiet svelte-14cazgd">Already exactly this; nothing to write.</p>`);
var root_4$6 = from_html(`<span> </span>`);
var root_5$6 = from_html(`<pre class="diff svelte-14cazgd" data-quoted=""></pre>`);
var root_6$6 = from_html(`<p class="said svelte-14cazgd" role="status"> </p>`);
var root_7$6 = from_html(`<div class="pending svelte-14cazgd" role="group"><p class="file svelte-14cazgd"><!> <code class="svelte-14cazgd"> </code> <!> <!> <!></p> <!> <!> <div class="actions svelte-14cazgd"><button type="button" class="primary svelte-14cazgd"><!> </button> <button type="button">Cancel — write nothing</button></div></div>`);
function PendingWrite($$anchor, $$props) {
	push($$props, true);
	let confirmLabel = prop($$props, "confirmLabel", 3, "Confirm"), step = prop($$props, "step", 3, "");
	let shown = state(null);
	const p = user_derived(() => get(shown) ?? $$props.pending);
	let busy = state(false);
	let said = state("");
	async function confirm() {
		if (get(busy)) return;
		set(busy, true);
		set(said, "");
		try {
			const r = await api($$props.url, {
				method: "POST",
				body: JSON.stringify({
					...$$props.body,
					confirm: true,
					diff: get(p).diff
				})
			});
			$$props.ondone?.(r);
		} catch (e) {
			if (e instanceof Refused && e.status === 409) {
				const b = e.body;
				if (typeof b?.diff === "string") {
					set(shown, {
						path: b.path ?? get(p).path,
						diff: b.diff,
						writes: b.writes ?? true,
						uncommitted: b.uncommitted
					}, true);
					set(said, "The file changed since this was shown; this is the change now. Nothing was written.");
					return;
				}
			}
			set(said, `Nothing was written: ${e instanceof Error ? e.message : String(e)}`);
		} finally {
			set(busy, false);
		}
	}
	const lines = user_derived(() => get(p).diff.replace(/\n$/, "").split("\n"));
	var div = root_7$6();
	var p_1 = child(div);
	var node = child(p_1);
	Icon(node, {
		name: "file",
		size: 13
	});
	var code = sibling(node, 2);
	var text = only_child(code, true);
	var node_1 = sibling(code, 2);
	var consequent = ($$anchor) => {
		var span = root$5();
		append($$anchor, span);
	};
	if_block(node_1, ($$render) => {
		if (get(p).new_file) $$render(consequent);
	});
	var node_2 = sibling(node_1, 2);
	var consequent_1 = ($$anchor) => {
		var span_1 = root_1$6();
		append($$anchor, span_1);
	};
	if_block(node_2, ($$render) => {
		if (get(p).removes) $$render(consequent_1);
	});
	var node_3 = sibling(node_2, 2);
	var consequent_2 = ($$anchor) => {
		var span_2 = root_2$6();
		append($$anchor, span_2);
	};
	if_block(node_3, ($$render) => {
		if (get(p).uncommitted) $$render(consequent_2);
	});
	reset(p_1);
	var node_4 = sibling(p_1, 2);
	var consequent_3 = ($$anchor) => {
		var p_2 = root_3$6();
		append($$anchor, p_2);
	};
	var alternate = ($$anchor) => {
		var pre = root_5$6();
		each(pre, 21, () => get(lines), index, ($$anchor, l) => {
			var span_3 = root_4$6();
			let classes;
			var text_1 = only_child(span_3);
			template_effect(($0, $1) => {
				classes = set_class(span_3, 1, "svelte-14cazgd", null, classes, {
					add: $0,
					del: $1
				});
				set_text(text_1, `${get(l) ?? ""}
`);
			}, [() => get(l).startsWith("+"), () => get(l).startsWith("-")]);
			append($$anchor, span_3);
		});
		reset(pre);
		append($$anchor, pre);
	};
	if_block(node_4, ($$render) => {
		if (!get(p).writes) $$render(consequent_3);
		else $$render(alternate, -1);
	});
	var node_5 = sibling(node_4, 2);
	var consequent_4 = ($$anchor) => {
		var p_3 = root_6$6();
		var text_2 = only_child(p_3, true);
		template_effect(() => set_text(text_2, get(said)));
		append($$anchor, p_3);
	};
	if_block(node_5, ($$render) => {
		if (get(said)) $$render(consequent_4);
	});
	var div_1 = sibling(node_5, 2);
	var button = child(div_1);
	var node_6 = child(button);
	Icon(node_6, {
		name: "check",
		size: 13
	});
	var text_3 = sibling(node_6);
	reset(button);
	var button_1 = sibling(button, 2);
	reset(div_1);
	reset(div);
	template_effect(() => {
		set_attribute(div, "aria-label", `the change to ${get(p).path ?? ""}`);
		set_text(text, get(p).path);
		button.disabled = get(busy) || !get(p).writes;
		set_attribute(button, "data-first-run-step", step() || void 0);
		set_text(text_3, ` ${(get(busy) ? "Writing…" : confirmLabel()) ?? ""}`);
		button_1.disabled = get(busy);
	});
	delegated("click", button, confirm);
	delegated("click", button_1, () => $$props.oncancel?.());
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Connect.svelte
var root$4 = from_html(`<p class="quiet svelte-mew5m0"> <code class="svelte-mew5m0">npm install -g @anthropic-ai/claude-code</code> — and come back; until then Devplane watches nothing and gates nothing.</p>`);
var root_1$5 = from_html(`<span class="off svelte-mew5m0"> </span>`);
var root_2$5 = from_html(`<span class="on svelte-mew5m0">connected</span>`);
var root_3$5 = from_html(`<span class="quiet svelte-mew5m0">not connected</span>`);
var root_4$5 = from_html(`<button type="button"> </button>`);
var root_5$5 = from_html(`<button type="button">Disconnect — show the change</button>`);
var root_6$5 = from_html(`<div class="actions svelte-mew5m0"><!> <!></div>`);
var root_7$5 = from_html(`<div class="agent svelte-mew5m0"><p class="who svelte-mew5m0"><!> <b> </b> <!> <code class="path svelte-mew5m0"> </code></p> <!></div>`);
var root_8$5 = from_html(`<p class="fail svelte-mew5m0" role="alert"><!></p>`);
var root_9$5 = from_html(`<div><button type="button">Skip — watch sessions only</button></div>`);
var root_10$5 = from_html(`<div class="connect svelte-mew5m0"><!> <!> <!> <!></div>`);
function Connect($$anchor, $$props) {
	push($$props, true);
	let firstRun = prop($$props, "firstRun", 3, false);
	const route = (verb, agent) => verb === "connect" ? `/api/connect/${encodeURIComponent(agent)}` : `/api/disconnect/${encodeURIComponent(agent)}`;
	let open = state(null);
	let said = state("");
	let asking = state("");
	async function preview(agent, verb) {
		if (get(asking)) return;
		set(asking, agent, true);
		set(said, "");
		try {
			const pending = await api(route(verb, agent), {
				method: "POST",
				body: JSON.stringify({ confirm: false })
			});
			set(open, {
				agent,
				verb,
				pending
			}, true);
		} catch (e) {
			const b = e instanceof Refused ? e.body : null;
			set(said, b?.refused === "unstable_path" && b.says ? b.says : e instanceof Error ? e.message : String(e), true);
		} finally {
			set(asking, "");
		}
	}
	const shown = user_derived(() => $$props.agents.filter((a) => a.present || a.connected));
	var div = root_10$5();
	var node = child(div);
	var consequent = ($$anchor) => {
		var p = root$4();
		var text = child(p);
		next(2);
		reset(p);
		template_effect(($0) => set_text(text, `No agent found on this machine. Looked for ${$0 ?? ""}. Install one — Claude Code with `), [() => $$props.agents.map((a) => `${a.name} (${a.settings_path})`).join(", ")]);
		append($$anchor, p);
	};
	if_block(node, ($$render) => {
		if (get(shown).length === 0) $$render(consequent);
	});
	var node_1 = sibling(node, 2);
	each(node_1, 17, () => get(shown), (a) => a.agent, ($$anchor, a) => {
		var div_1 = root_7$5();
		var p_1 = child(div_1);
		var node_2 = child(p_1);
		Icon(node_2, {
			name: "agent",
			size: 13
		});
		var b_1 = sibling(node_2, 2);
		var text_1 = only_child(b_1, true);
		var node_3 = sibling(b_1, 2);
		var consequent_1 = ($$anchor) => {
			var span = root_1$5();
			var text_2 = only_child(span);
			template_effect(($0) => set_text(text_2, `gate off for ${get(a).name ?? ""} — the binary its hooks run is gone (${$0 ?? ""})`), [() => get(a).gate_off.join(", ")]);
			append($$anchor, span);
		};
		var consequent_2 = ($$anchor) => {
			var span_1 = root_2$5();
			append($$anchor, span_1);
		};
		var alternate = ($$anchor) => {
			var span_2 = root_3$5();
			append($$anchor, span_2);
		};
		if_block(node_3, ($$render) => {
			if (get(a).gate_off.length) $$render(consequent_1);
			else if (get(a).connected) $$render(consequent_2, 1);
			else $$render(alternate, -1);
		});
		var code = sibling(node_3, 2);
		var text_3 = only_child(code, true);
		reset(p_1);
		var node_4 = sibling(p_1, 2);
		var consequent_3 = ($$anchor) => {
			{
				let $0 = user_derived(() => route(get(open).verb, get(a).agent));
				let $1 = user_derived(() => get(open).verb === "connect" ? `Connect ${get(a).name}` : `Disconnect ${get(a).name}`);
				PendingWrite($$anchor, {
					get pending() {
						return get(open).pending;
					},
					get url() {
						return get($0);
					},
					body: {},
					get confirmLabel() {
						return get($1);
					},
					oncancel: () => set(open, null),
					ondone: () => {
						set(open, null);
						$$props.onchanged?.();
					}
				});
			}
		};
		var alternate_1 = ($$anchor) => {
			var div_2 = root_6$5();
			var node_5 = child(div_2);
			var consequent_4 = ($$anchor) => {
				var button = root_4$5();
				var text_4 = only_child(button, true);
				template_effect(() => {
					button.disabled = !!get(asking);
					set_text(text_4, get(a).gate_off.length ? "Connect again — show the change" : "Connect — show the change");
				});
				delegated("click", button, () => preview(get(a).agent, "connect"));
				append($$anchor, button);
			};
			if_block(node_5, ($$render) => {
				if (!get(a).connected || get(a).gate_off.length) $$render(consequent_4);
			});
			var node_6 = sibling(node_5, 2);
			var consequent_5 = ($$anchor) => {
				var button_1 = root_5$5();
				template_effect(() => button_1.disabled = !!get(asking));
				delegated("click", button_1, () => preview(get(a).agent, "disconnect"));
				append($$anchor, button_1);
			};
			if_block(node_6, ($$render) => {
				if (get(a).connected) $$render(consequent_5);
			});
			reset(div_2);
			append($$anchor, div_2);
		};
		if_block(node_4, ($$render) => {
			if (get(open) && get(open).agent === get(a).agent) $$render(consequent_3);
			else $$render(alternate_1, -1);
		});
		reset(div_1);
		template_effect(() => {
			set_text(text_1, get(a).name);
			set_text(text_3, get(a).settings_path);
		});
		append($$anchor, div_1);
	});
	var node_7 = sibling(node_1, 2);
	var consequent_6 = ($$anchor) => {
		var p_2 = root_8$5();
		var node_8 = child(p_2);
		Inline(node_8, { get text() {
			return get(said);
		} });
		reset(p_2);
		append($$anchor, p_2);
	};
	if_block(node_7, ($$render) => {
		if (get(said)) $$render(consequent_6);
	});
	var node_9 = sibling(node_7, 2);
	var consequent_7 = ($$anchor) => {
		var div_3 = root_9$5();
		var button_2 = only_child(div_3);
		template_effect(() => set_attribute(button_2, "data-first-run-step", firstRun() ? "skip-connect" : void 0));
		delegated("click", button_2, function(...$$args) {
			$$props.onskip?.apply(this, $$args);
		});
		append($$anchor, div_3);
	};
	if_block(node_9, ($$render) => {
		if ($$props.onskip) $$render(consequent_7);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Trust.svelte
var root_1$4 = from_html(`<div class="row svelte-1eg9zt8"><!> <b> </b> <code class="svelte-1eg9zt8"> </code> <button type="button">Read what it would allow</button></div>`);
var root_2$4 = from_html(`<div class="found"><p class="label svelte-1eg9zt8">Found from sessions on this machine</p> <!></div>`);
var root_3$4 = from_html(`<span class="warn svelte-1eg9zt8">not a git repository — a change here cannot be isolated</span>`);
var root_4$4 = from_html(`<p class="quiet svelte-1eg9zt8">Already trusted.</p>`);
var root_5$4 = from_html(`<li><code data-quoted="" class="svelte-1eg9zt8"> </code> <span class="src svelte-1eg9zt8"> </span> <span class="detail svelte-1eg9zt8"><!></span></li>`);
var root_6$4 = from_html(`<div class="kind"><p class="label svelte-1eg9zt8"> </p> <ul class="svelte-1eg9zt8"></ul></div>`);
var root_7$4 = from_html(`<p class="warn svelte-1eg9zt8"> </p>`);
var root_8$4 = from_html(`<p class="quiet svelte-1eg9zt8">Trusting lets an agent Devplane starts here run this repository's own configuration without asking. <!></p> <!> <!> <div><button type="button" class="primary svelte-1eg9zt8"><!> </button></div>`, 1);
var root_9$4 = from_html(`<div class="scan svelte-1eg9zt8"><p class="svelte-1eg9zt8"><b> </b> <code class="svelte-1eg9zt8"> </code><!></p> <!></div>`);
var root_10$4 = from_html(`<p class="fail svelte-1eg9zt8" role="alert"><!></p>`);
var root_11$4 = from_html(`<div class="trust svelte-1eg9zt8"><!> <form class="add"><label for="trust-path" class="label svelte-1eg9zt8">Add a repository by its path</label> <div class="line svelte-1eg9zt8"><input id="trust-path" placeholder="/path/to/your/repository" autocomplete="off" spellcheck="false" class="svelte-1eg9zt8"/> <button type="submit"> </button></div></form> <!> <!></div>`);
function Trust($$anchor, $$props) {
	push($$props, true);
	let firstRun = prop($$props, "firstRun", 3, false);
	const projects = resource(() => "/api/projects", { tell: () => "devplane doctor" });
	const found = user_derived(() => (Array.isArray(projects.data) ? projects.data : []).filter((p) => !p.trusted));
	let path = state("");
	let scan = state(null);
	let reading = state(false);
	let trusting = state(false);
	let said = state("");
	async function read(p) {
		const want = p.trim();
		if (!want || get(reading)) return;
		set(reading, true);
		set(said, "");
		set(scan, null);
		try {
			set(scan, await api(`/api/projects/scan?path=${encodeURIComponent(want)}`), true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(reading, false);
		}
	}
	async function trust() {
		if (!get(scan) || get(trusting)) return;
		set(trusting, true);
		set(said, "");
		try {
			await api("/api/projects/trust", {
				method: "POST",
				body: JSON.stringify({ path: get(scan).root })
			});
			const root = get(scan).root;
			set(scan, null);
			set(path, "");
			projects.reload();
			$$props.ontrusted?.(root);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(trusting, false);
		}
	}
	const kinds = [
		["hook", "Hooks it runs"],
		["mcp", "MCP servers it starts"],
		["skill", "Skills that pre-approve tools"],
		["policy", "Allow rules it brings"]
	];
	var div = root_11$4();
	var node = child(div);
	var consequent = ($$anchor) => {
		var div_1 = root_2$4();
		var node_1 = sibling(child(div_1), 2);
		each(node_1, 17, () => get(found), (p) => p.id, ($$anchor, p) => {
			var div_2 = root_1$4();
			var node_2 = child(div_2);
			Icon(node_2, {
				name: "folder",
				size: 13
			});
			var b = sibling(node_2, 2);
			var text = only_child(b, true);
			var code = sibling(b, 2);
			var text_1 = only_child(code, true);
			var button = sibling(code, 2);
			reset(div_2);
			template_effect(() => {
				set_text(text, get(p).name);
				set_text(text_1, get(p).root);
				button.disabled = get(reading);
			});
			delegated("click", button, () => read(get(p).root));
			append($$anchor, div_2);
		});
		reset(div_1);
		append($$anchor, div_1);
	};
	if_block(node, ($$render) => {
		if (get(found).length) $$render(consequent);
	});
	var form = sibling(node, 2);
	var div_3 = sibling(child(form), 2);
	var input = child(div_3);
	remove_input_defaults(input);
	var button_1 = sibling(input, 2);
	var text_2 = only_child(button_1, true);
	reset(div_3);
	reset(form);
	var node_3 = sibling(form, 2);
	var consequent_6 = ($$anchor) => {
		var div_4 = root_9$4();
		var p_1 = child(div_4);
		var b_1 = child(p_1);
		var text_3 = only_child(b_1, true);
		var code_1 = sibling(b_1, 2);
		var text_4 = only_child(code_1, true);
		var node_4 = sibling(code_1);
		var consequent_1 = ($$anchor) => {
			var span = root_3$4();
			append($$anchor, span);
		};
		if_block(node_4, ($$render) => {
			if (!get(scan).repository) $$render(consequent_1);
		});
		reset(p_1);
		var node_5 = sibling(p_1, 2);
		var consequent_2 = ($$anchor) => {
			var p_2 = root_4$4();
			append($$anchor, p_2);
		};
		var alternate = ($$anchor) => {
			var fragment = root_8$4();
			var p_3 = first_child(fragment);
			var node_6 = sibling(child(p_3));
			var consequent_3 = ($$anchor) => {
				var text_5 = text();
				template_effect(() => set_text(text_5, `Nothing in it asks to run anything${get(scan).skills ? `; it ships ${get(scan).skills} skill${get(scan).skills === 1 ? "" : "s"}, none pre-approving tools` : ""}.`));
				append($$anchor, text_5);
			};
			if_block(node_6, ($$render) => {
				if (get(scan).findings.length === 0 && get(scan).unreadable.length === 0) $$render(consequent_3);
			});
			reset(p_3);
			var node_7 = sibling(p_3, 2);
			each(node_7, 17, () => kinds, ([k, title]) => k, ($$anchor, $$item) => {
				var $$array = user_derived(() => to_array(get($$item), 2));
				let k = () => get($$array)[0];
				let title = () => get($$array)[1];
				const rows = user_derived(() => get(scan).findings.filter((f) => f.kind === k()));
				var fragment_2 = comment();
				var node_8 = first_child(fragment_2);
				var consequent_4 = ($$anchor) => {
					var div_5 = root_6$4();
					var p_4 = child(div_5);
					var text_6 = only_child(p_4, true);
					var ul = sibling(p_4, 2);
					each(ul, 21, () => get(rows), index, ($$anchor, f) => {
						var li = root_5$4();
						var code_2 = child(li);
						var text_7 = only_child(code_2, true);
						var span_1 = sibling(code_2, 2);
						var text_8 = only_child(span_1, true);
						var span_2 = sibling(span_1, 2);
						var node_9 = child(span_2);
						Inline(node_9, { get text() {
							return get(f).detail;
						} });
						reset(span_2);
						reset(li);
						template_effect(() => {
							set_text(text_7, get(f).subject);
							set_text(text_8, get(f).source);
						});
						append($$anchor, li);
					});
					reset(ul);
					reset(div_5);
					template_effect(() => set_text(text_6, title()));
					append($$anchor, div_5);
				};
				if_block(node_8, ($$render) => {
					if (get(rows).length) $$render(consequent_4);
				});
				append($$anchor, fragment_2);
			});
			var node_10 = sibling(node_7, 2);
			var consequent_5 = ($$anchor) => {
				var p_5 = root_7$4();
				var text_9 = only_child(p_5);
				template_effect(($0) => set_text(text_9, `Could not be read, so what they run is not shown here and is still run by the agent: ${$0 ?? ""}`), [() => get(scan).unreadable.join(", ")]);
				append($$anchor, p_5);
			};
			if_block(node_10, ($$render) => {
				if (get(scan).unreadable.length) $$render(consequent_5);
			});
			var div_6 = sibling(node_10, 2);
			var button_2 = child(div_6);
			var node_11 = child(button_2);
			Icon(node_11, {
				name: "shield",
				size: 13
			});
			var text_10 = sibling(node_11);
			reset(button_2);
			reset(div_6);
			template_effect(() => {
				button_2.disabled = get(trusting);
				set_attribute(button_2, "data-first-run-step", firstRun() ? "trust" : void 0);
				set_text(text_10, ` ${get(trusting) ? "Trusting…" : `Trust ${get(scan).name ?? "this repository"}`}`);
			});
			delegated("click", button_2, trust);
			append($$anchor, fragment);
		};
		if_block(node_5, ($$render) => {
			if (get(scan).trusted) $$render(consequent_2);
			else $$render(alternate, -1);
		});
		reset(div_4);
		template_effect(() => {
			set_attribute(div_4, "aria-label", `what trusting ${get(scan).root ?? ""} would allow`);
			set_text(text_3, get(scan).name ?? get(scan).root);
			set_text(text_4, get(scan).root);
		});
		append($$anchor, div_4);
	};
	if_block(node_3, ($$render) => {
		if (get(scan)) $$render(consequent_6);
	});
	var node_12 = sibling(node_3, 2);
	var consequent_7 = ($$anchor) => {
		var p_6 = root_10$4();
		var node_13 = child(p_6);
		Inline(node_13, { get text() {
			return get(said);
		} });
		reset(p_6);
		append($$anchor, p_6);
	};
	if_block(node_12, ($$render) => {
		if (get(said)) $$render(consequent_7);
	});
	reset(div);
	template_effect(($0) => {
		set_attribute(input, "data-first-run-step", firstRun() ? "add-repository" : void 0);
		button_1.disabled = $0;
		set_text(text_2, get(reading) ? "Reading…" : "Read it");
	}, [() => !get(path).trim() || get(reading)]);
	event("submit", form, (e) => {
		e.preventDefault();
		read(get(path));
	});
	bind_value(input, () => get(path), ($$value) => set(path, $$value));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Declare.svelte
var root$3 = from_html(`<p class="head svelte-be32sg"><!> </p>`);
var root_1$3 = from_html(`<p class="head ok svelte-be32sg"> </p>`);
var root_2$3 = from_html(`<p class="done svelte-be32sg" role="status"><!> Check declared — nothing has run yet. It runs when a change's agent finishes, or when you run the gates.</p>`);
var root_3$3 = from_html(`<p class="fail svelte-be32sg" role="alert"><!> <code class="svelte-be32sg"> </code> </p>`);
var root_4$3 = from_html(`<p class="quiet svelte-be32sg"> </p>`);
var root_5$3 = from_html(`<textarea aria-label="the command" class="svelte-be32sg"></textarea>`);
var root_6$3 = from_html(`<code data-quoted="" class="svelte-be32sg"> </code>`);
var root_7$3 = from_html(`<li><input type="checkbox"/> <div class="cmd svelte-be32sg"><!> <span class="src svelte-be32sg"> </span></div> <div class="tools svelte-be32sg"><button type="button"> </button> <button type="button" aria-label="move up">↑</button> <button type="button" aria-label="move down">↓</button></div></li>`);
var root_8$3 = from_html(`<ol class="rows svelte-be32sg" aria-label="commands for check, in order"></ol>`);
var root_9$3 = from_html(`<label class="row svelte-be32sg"><input type="checkbox"/> <code data-quoted="" class="svelte-be32sg"> </code> <span class="src svelte-be32sg"> </span></label>`);
var root_10$3 = from_html(`<div class="named svelte-be32sg"><p class="label svelte-be32sg">A named gate — evidence on the change, never part of check</p> <!></div>`);
var root_11$3 = from_html(`<p class="fail svelte-be32sg"> </p>`);
var root_12$3 = from_html(`<!> <form class="add svelte-be32sg"><input placeholder="Add a command — e.g. make test" aria-label="add a command to check" class="svelte-be32sg"/> <button type="submit"><!> Add</button></form> <!> <!> <div class="go svelte-be32sg"><button type="button" class="primary svelte-be32sg"> </button> <span class="quiet svelte-be32sg">Offering runs nothing. Only the commands you confirm become the gate.</span></div>`, 1);
var root_13$3 = from_html(`<!> <!> <!>`, 1);
var root_14$3 = from_html(`<div class="declare svelte-be32sg"><!></div>`);
function Declare($$anchor, $$props) {
	push($$props, true);
	let firstRun = prop($$props, "firstRun", 3, false), planted = prop($$props, "candidates", 3, null);
	const read = resource(() => planted() ? null : `/api/projects/${encodeURIComponent($$props.project)}/gates/candidates`, { tell: () => "devplane config check" });
	const data = user_derived(() => read.data ?? planted());
	let n = 0;
	function seed(d) {
		if (!d) return {
			rows: [],
			named: []
		};
		const current = d.current.check.map((c) => ({
			key: n++,
			command: c,
			source: "devplane.toml",
			line: null,
			picked: true,
			editing: false
		}));
		const offered = d.candidates.filter((c) => c.joins === "check" && !d.current.check.includes(c.command)).map((c) => ({
			key: n++,
			command: c.command,
			source: c.source,
			line: c.line,
			picked: false,
			editing: false
		}));
		const named = d.candidates.filter((c) => c.joins.startsWith("named:")).map((c) => ({
			key: n++,
			command: c.command,
			source: c.source,
			line: c.line,
			picked: !!d.current.named.openspec,
			editing: false
		}));
		return {
			rows: [...current, ...offered],
			named
		};
	}
	const first = untrack(() => seed(planted()));
	let rows = state(proxy(first.rows));
	let named = state(proxy(first.named));
	let typed = state("");
	let seeded = state(proxy(untrack(() => planted()?.project.id ?? "")));
	user_effect(() => {
		if (!get(data) || get(seeded) === get(data).project.id) return;
		set(seeded, get(data).project.id, true);
		(($$value) => {
			set(rows, $$value.rows, true);
			set(named, $$value.named, true);
		})(seed(get(data)));
	});
	function move(i, by) {
		const j = i + by;
		if (j < 0 || j >= get(rows).length) return;
		const next = [...get(rows)];
		[next[i], next[j]] = [next[j], next[i]];
		set(rows, next, true);
	}
	function add(e) {
		e?.preventDefault();
		const c = get(typed).trim();
		if (!c) return;
		set(rows, [...get(rows), {
			key: n++,
			command: c,
			source: "typed here",
			line: null,
			picked: true,
			editing: false
		}], true);
		set(typed, "");
	}
	const chosen = user_derived(() => ({
		check: get(rows).filter((r) => r.picked && r.command.trim()).map((r) => r.command),
		named: get(named).some((r) => r.picked) ? { openspec: get(named).filter((r) => r.picked).map((r) => r.command) } : {}
	}));
	const anything = user_derived(() => get(chosen).check.length > 0 || Object.keys(get(chosen).named).length > 0);
	let pending = state(null);
	let said = state("");
	let done = state(false);
	let asking = state(false);
	const url = user_derived(() => `/api/projects/${encodeURIComponent($$props.project)}/gates`);
	async function preview() {
		if (!get(anything) || get(asking)) return;
		set(asking, true);
		set(said, "");
		try {
			set(pending, await api(get(url), {
				method: "POST",
				body: JSON.stringify({
					...get(chosen),
					confirm: false
				})
			}), true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(asking, false);
		}
	}
	function written() {
		set(pending, null);
		set(done, true);
		read.reload();
		$$props.ondone?.();
	}
	const where = (r) => r.source === "typed here" ? "typed here" : `found: ${r.source}${r.line ? `:${r.line}` : ""}`;
	const mark = (s) => firstRun() ? s : void 0;
	const firstOffered = user_derived(() => get(rows).findIndex((r) => r.source !== "devplane.toml"));
	var div = root_14$3();
	var node = child(div);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "what this repository offers as a check",
			get failure() {
				return read.failure;
			},
			get retry() {
				return read.reload;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		Skeleton($$anchor, { rows: 3 });
	};
	var alternate_4 = ($$anchor) => {
		var fragment_2 = root_13$3();
		var node_1 = first_child(fragment_2);
		var consequent_2 = ($$anchor) => {};
		var consequent_3 = ($$anchor) => {
			var p = root$3();
			var node_2 = child(p);
			Icon(node_2, {
				name: "alert",
				size: 13
			});
			var text = sibling(node_2);
			reset(p);
			template_effect(() => set_text(text, ` ${get(data).project.name ?? ""}: no gates declared — changes here cannot become verified until a check is declared.`));
			append($$anchor, p);
		};
		var alternate = ($$anchor) => {
			var p_1 = root_1$3();
			var text_1 = only_child(p_1);
			template_effect(() => set_text(text_1, `${get(data).project.name ?? ""}: check is declared. Change it below; the edit is shown as a diff before anything is written.`));
			append($$anchor, p_1);
		};
		if_block(node_1, ($$render) => {
			if (firstRun()) $$render(consequent_2);
			else if (get(data).current.check.length === 0) $$render(consequent_3, 1);
			else $$render(alternate, -1);
		});
		var node_3 = sibling(node_1, 2);
		var consequent_4 = ($$anchor) => {
			var p_2 = root_2$3();
			var node_4 = child(p_2);
			Icon(node_4, {
				name: "check",
				size: 13
			});
			next();
			reset(p_2);
			append($$anchor, p_2);
		};
		if_block(node_3, ($$render) => {
			if (get(done)) $$render(consequent_4);
		});
		var node_5 = sibling(node_3, 2);
		var consequent_5 = ($$anchor) => {
			var p_3 = root_3$3();
			var node_6 = child(p_3);
			Icon(node_6, {
				name: "alert",
				size: 13
			});
			var code = sibling(node_6, 2);
			var text_2 = only_child(code, true);
			var text_3 = sibling(code);
			reset(p_3);
			template_effect(() => {
				set_text(text_2, get(data).config.path);
				set_text(text_3, ` does not parse${get(data).config.line ? ` at line ${get(data).config.line}` : ""}: ${get(data).config.error ?? ""}. Nothing can be written until it does; fix it by hand.`);
			});
			append($$anchor, p_3);
		};
		var consequent_6 = ($$anchor) => {
			{
				let $0 = user_derived(() => firstRun() ? "confirm-diff" : "");
				PendingWrite($$anchor, {
					get pending() {
						return get(pending);
					},
					get url() {
						return get(url);
					},
					get body() {
						return get(chosen);
					},
					confirmLabel: "Write devplane.toml",
					get step() {
						return get($0);
					},
					oncancel: () => set(pending, null),
					ondone: written
				});
			}
		};
		var alternate_3 = ($$anchor) => {
			var fragment_4 = root_12$3();
			var node_7 = first_child(fragment_4);
			var consequent_7 = ($$anchor) => {
				var p_4 = root_4$3();
				var text_4 = only_child(p_4);
				template_effect(($0) => set_text(text_4, `Found nothing to offer — looked for: ${$0 ?? ""}. Type the command that says this project is done.`), [() => get(data).looked_for.join(" · ")]);
				append($$anchor, p_4);
			};
			var alternate_2 = ($$anchor) => {
				var ol = root_8$3();
				each(ol, 23, () => get(rows), (r) => r.key, ($$anchor, r, i) => {
					var li = root_7$3();
					let classes;
					var input = child(li);
					remove_input_defaults(input);
					var div_1 = sibling(input, 2);
					var node_8 = child(div_1);
					var consequent_8 = ($$anchor) => {
						var textarea = root_5$3();
						remove_textarea_child(textarea);
						template_effect(($0) => set_attribute(textarea, "rows", $0), [() => Math.max(1, get(r).command.split("\n").length)]);
						bind_value(textarea, () => get(r).command, ($$value) => get(r).command = $$value);
						append($$anchor, textarea);
					};
					var alternate_1 = ($$anchor) => {
						var code_1 = root_6$3();
						var text_5 = only_child(code_1, true);
						template_effect(() => set_text(text_5, get(r).command));
						append($$anchor, code_1);
					};
					if_block(node_8, ($$render) => {
						if (get(r).editing) $$render(consequent_8);
						else $$render(alternate_1, -1);
					});
					var span = sibling(node_8, 2);
					var text_6 = only_child(span, true);
					reset(div_1);
					var div_2 = sibling(div_1, 2);
					var button = child(div_2);
					var text_7 = only_child(button, true);
					var button_1 = sibling(button, 2);
					var button_2 = sibling(button_1, 2);
					reset(div_2);
					reset(li);
					template_effect(($0, $1) => {
						classes = set_class(li, 1, "svelte-be32sg", null, classes, { on: get(r).picked });
						set_attribute(input, "aria-label", `use ${get(r).command ?? ""} in check`);
						set_attribute(input, "data-first-run-step", $0);
						set_text(text_6, $1);
						set_text(text_7, get(r).editing ? "Done" : "Edit");
						button_1.disabled = get(i) === 0;
						button_2.disabled = get(i) === get(rows).length - 1;
					}, [() => get(i) === get(firstOffered) ? mark("pick-check") : void 0, () => where(get(r))]);
					bind_checked(input, () => get(r).picked, ($$value) => get(r).picked = $$value);
					delegated("click", button, () => get(r).editing = !get(r).editing);
					delegated("click", button_1, () => move(get(i), -1));
					delegated("click", button_2, () => move(get(i), 1));
					append($$anchor, li);
				});
				reset(ol);
				append($$anchor, ol);
			};
			if_block(node_7, ($$render) => {
				if (get(rows).length === 0) $$render(consequent_7);
				else $$render(alternate_2, -1);
			});
			var form = sibling(node_7, 2);
			var input_1 = child(form);
			remove_input_defaults(input_1);
			var button_3 = sibling(input_1, 2);
			var node_9 = child(button_3);
			Icon(node_9, {
				name: "plus",
				size: 12
			});
			next();
			reset(button_3);
			reset(form);
			var node_10 = sibling(form, 2);
			var consequent_9 = ($$anchor) => {
				var div_3 = root_10$3();
				var node_11 = sibling(child(div_3), 2);
				each(node_11, 17, () => get(named), (r) => r.key, ($$anchor, r, $$index_1) => {
					var label = root_9$3();
					var input_2 = child(label);
					remove_input_defaults(input_2);
					var code_2 = sibling(input_2, 2);
					var text_8 = only_child(code_2, true);
					var span_1 = sibling(code_2, 2);
					var text_9 = only_child(span_1, true);
					reset(label);
					template_effect(($0) => {
						set_text(text_8, get(r).command);
						set_text(text_9, $0);
					}, [() => where(get(r))]);
					bind_checked(input_2, () => get(r).picked, ($$value) => get(r).picked = $$value);
					append($$anchor, label);
				});
				reset(div_3);
				append($$anchor, div_3);
			};
			if_block(node_10, ($$render) => {
				if (get(named).length) $$render(consequent_9);
			});
			var node_12 = sibling(node_10, 2);
			var consequent_10 = ($$anchor) => {
				var p_5 = root_11$3();
				var text_10 = only_child(p_5, true);
				template_effect(() => set_text(text_10, get(said)));
				append($$anchor, p_5);
			};
			if_block(node_12, ($$render) => {
				if (get(said)) $$render(consequent_10);
			});
			var div_4 = sibling(node_12, 2);
			var button_4 = child(div_4);
			var text_11 = only_child(button_4, true);
			next(2);
			reset(div_4);
			template_effect(($0, $1) => {
				button_3.disabled = $0;
				button_4.disabled = !get(anything) || get(asking);
				set_attribute(button_4, "data-first-run-step", $1);
				set_text(text_11, get(asking) ? "Reading…" : "Continue — show the change to devplane.toml");
			}, [() => !get(typed).trim(), () => mark("show-diff")]);
			event("submit", form, add);
			bind_value(input_1, () => get(typed), ($$value) => set(typed, $$value));
			delegated("click", button_4, preview);
			append($$anchor, fragment_4);
		};
		if_block(node_5, ($$render) => {
			if (!get(data).config.parses) $$render(consequent_5);
			else if (get(pending)) $$render(consequent_6, 1);
			else $$render(alternate_3, -1);
		});
		append($$anchor, fragment_2);
	};
	if_block(node, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent);
		else if (!get(data)) $$render(consequent_1, 1);
		else $$render(alternate_4, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Updates.svelte
var root$2 = from_html(`<p class="quiet svelte-kayi80">Reading…</p>`);
var root_1$2 = from_html(`<p class="quiet svelte-kayi80">Update checks are in the desktop app. This page is served by <code>devplane up</code>, which never checks.</p>`);
var root_2$2 = from_html(`<p class="svelte-kayi80">May Devplane check for a newer version when the app starts?</p> <p class="quiet svelte-kayi80">It fetches <code> </code> <b>Install and restart</b>. Until you answer, nothing is checked.</p>`, 1);
var root_3$2 = from_html(`<p class="svelte-kayi80">Update checks off. Releases are published at <code> </code>.</p>`);
var root_4$2 = from_html(`<p class="svelte-kayi80"> <code> </code>.</p>`);
var root_5$2 = from_html(`<p class="quiet svelte-kayi80">Checking for a newer version…</p>`);
var root_6$2 = from_html(`<p class="svelte-kayi80"> </p>`);
var root_7$2 = from_html(`<p class="fail svelte-kayi80"> </p>`);
var root_8$2 = from_html(`<pre class="notes svelte-kayi80" aria-label="release notes"> </pre>`);
var root_9$2 = from_html(`<pre class="stops svelte-kayi80" role="alert"> </pre> <div class="actions svelte-kayi80"><button type="button" class="primary">Stop them, install and restart</button> <button type="button">Keep running</button></div>`, 1);
var root_10$2 = from_html(`<button type="button">Not now</button>`);
var root_11$2 = from_html(`<div class="actions svelte-kayi80"><button type="button" class="primary">Install and restart</button> <!></div>`);
var root_12$2 = from_html(`<p class="svelte-kayi80"><b> </b> is available<!>. Installing it restarts the app.</p> <!> <!> <!>`, 1);
var root_13$2 = from_html(`<p class="svelte-kayi80"><b> </b> <code> </code>.</p>`);
var root_14$2 = from_html(`<button type="button"> </button>`);
var root_15$2 = from_html(`<div class="actions svelte-kayi80"><!> <!></div>`);
var root_16$2 = from_html(`<p class="fail svelte-kayi80" role="alert"> </p>`);
var root_17$2 = from_html(`<!> <!> <!>`, 1);
var root_18 = from_html(`<div class="updates svelte-kayi80"><!></div>`);
function Updates($$anchor, $$props) {
	push($$props, true);
	const read = resource(() => $$props.shown ? null : "/api/app/update", { tell: () => "devplane doctor" });
	const s = user_derived(() => $$props.shown ?? read.data);
	let pending = state(null);
	let stops = state("");
	let said = state("");
	let busy = state(false);
	async function preview(pref) {
		set(said, "");
		try {
			set(pending, {
				pref,
				p: await api("/api/app/updates", {
					method: "POST",
					body: JSON.stringify({
						updates: pref,
						confirm: false
					})
				})
			}, true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		}
	}
	async function install(version, confirm = false) {
		if (get(busy)) return;
		set(busy, true);
		set(said, "");
		try {
			await api("/api/app/update/install", {
				method: "POST",
				body: JSON.stringify({
					version,
					confirm
				})
			});
			set(stops, "");
		} catch (e) {
			const b = e instanceof Refused ? e.body : null;
			if (b?.refused === "stops" && b.says) set(stops, b.says, true);
			else set(said, b?.why ?? (e instanceof Error ? e.message : String(e)), true);
		} finally {
			set(busy, false);
			await read.reload();
		}
	}
	async function dismiss(version) {
		set(said, "");
		try {
			await api("/api/app/update/dismiss", {
				method: "POST",
				body: JSON.stringify({ version })
			});
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		}
		await read.reload();
	}
	var div = root_18();
	var node = child(div);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "the update check",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		var p = root$2();
		append($$anchor, p);
	};
	var consequent_2 = ($$anchor) => {
		var p_1 = root_1$2();
		append($$anchor, p_1);
	};
	var alternate_1 = ($$anchor) => {
		var fragment_1 = root_17$2();
		var node_1 = first_child(fragment_1);
		var consequent_3 = ($$anchor) => {
			var fragment_2 = root_2$2();
			var p_2 = sibling(first_child(fragment_2), 2);
			var code = sibling(child(p_2));
			var text = only_child(code, true);
			var text_1 = sibling(code);
			next(2);
			reset(p_2);
			template_effect(() => {
				set_text(text, get(s).fetches);
				set_text(text_1, ` once per launch — never on a timer — and sends ${get(s).sends ?? ""}. Nothing is
        downloaded until you choose `);
			});
			append($$anchor, fragment_2);
		};
		var consequent_4 = ($$anchor) => {
			var p_3 = root_3$2();
			var code_1 = sibling(child(p_3));
			var text_2 = only_child(code_1, true);
			next();
			reset(p_3);
			template_effect(() => set_text(text_2, get(s).releases));
			append($$anchor, p_3);
		};
		var consequent_5 = ($$anchor) => {
			var p_4 = root_4$2();
			var text_3 = child(p_4);
			var code_2 = sibling(text_3);
			var text_4 = only_child(code_2, true);
			next();
			reset(p_4);
			template_effect(() => {
				set_text(text_3, `${get(s).why ?? ""}. Releases are published at `);
				set_text(text_4, get(s).releases);
			});
			append($$anchor, p_4);
		};
		var consequent_6 = ($$anchor) => {
			var p_5 = root_5$2();
			append($$anchor, p_5);
		};
		var consequent_7 = ($$anchor) => {
			var p_6 = root_6$2();
			var text_5 = only_child(p_6);
			template_effect(() => set_text(text_5, `Update checks on. Version ${get(s).running ?? ""} is the newest.`));
			append($$anchor, p_6);
		};
		var consequent_8 = ($$anchor) => {
			var p_7 = root_6$2();
			var text_6 = only_child(p_7);
			template_effect(() => set_text(text_6, `Update checks on. ${get(s).why ?? ""}; nothing was downloaded.`));
			append($$anchor, p_7);
		};
		var consequent_9 = ($$anchor) => {
			var p_8 = root_7$2();
			var text_7 = only_child(p_8);
			template_effect(() => set_text(text_7, `The last check found nothing it could trust: ${get(s).why ?? ""}. Nothing was downloaded.`));
			append($$anchor, p_8);
		};
		var consequent_15 = ($$anchor) => {
			var fragment_3 = root_12$2();
			var p_9 = first_child(fragment_3);
			var b_1 = child(p_9);
			var text_8 = only_child(b_1);
			var node_2 = sibling(b_1, 2);
			var consequent_10 = ($$anchor) => {
				var text_9 = text();
				template_effect(() => set_text(text_9, `(you run ${get(s).running ?? ""})`));
				append($$anchor, text_9);
			};
			if_block(node_2, ($$render) => {
				if (get(s).state === "offer") $$render(consequent_10);
			});
			next();
			reset(p_9);
			var node_3 = sibling(p_9, 2);
			var consequent_11 = ($$anchor) => {
				var pre = root_8$2();
				var text_10 = only_child(pre, true);
				template_effect(() => set_text(text_10, get(s).notes));
				append($$anchor, pre);
			};
			if_block(node_3, ($$render) => {
				if (get(s).state === "offer" && get(s).notes) $$render(consequent_11);
			});
			var node_4 = sibling(node_3, 2);
			var consequent_12 = ($$anchor) => {
				var p_10 = root_7$2();
				var text_11 = only_child(p_10, true);
				template_effect(() => set_text(text_11, get(s).why));
				append($$anchor, p_10);
			};
			if_block(node_4, ($$render) => {
				if (get(s).state === "failed") $$render(consequent_12);
			});
			var node_5 = sibling(node_4, 2);
			var consequent_13 = ($$anchor) => {
				var fragment_5 = root_9$2();
				var pre_1 = first_child(fragment_5);
				var text_12 = only_child(pre_1, true);
				var div_1 = sibling(pre_1, 2);
				var button = child(div_1);
				var button_1 = sibling(button, 2);
				reset(div_1);
				template_effect(() => {
					set_text(text_12, get(stops));
					button.disabled = get(busy);
					button_1.disabled = get(busy);
				});
				delegated("click", button, () => install(get(s).version, true));
				delegated("click", button_1, () => set(stops, ""));
				append($$anchor, fragment_5);
			};
			var alternate = ($$anchor) => {
				var div_2 = root_11$2();
				var button_2 = child(div_2);
				var node_6 = sibling(button_2, 2);
				var consequent_14 = ($$anchor) => {
					var button_3 = root_10$2();
					delegated("click", button_3, () => dismiss(get(s).version));
					append($$anchor, button_3);
				};
				if_block(node_6, ($$render) => {
					if (get(s).state === "offer") $$render(consequent_14);
				});
				reset(div_2);
				template_effect(() => button_2.disabled = get(busy));
				delegated("click", button_2, () => install(get(s).version));
				append($$anchor, div_2);
			};
			if_block(node_5, ($$render) => {
				if (get(stops)) $$render(consequent_13);
				else $$render(alternate, -1);
			});
			template_effect(() => set_text(text_8, `Devplane ${get(s).version ?? ""}`));
			append($$anchor, fragment_3);
		};
		var consequent_16 = ($$anchor) => {
			var p_11 = root_13$2();
			var b_2 = child(p_11);
			var text_13 = only_child(b_2);
			var text_14 = sibling(b_2);
			var code_3 = sibling(text_14);
			var text_15 = only_child(code_3, true);
			next();
			reset(p_11);
			template_effect(() => {
				set_text(text_13, `Devplane ${get(s).version ?? ""}`);
				set_text(text_14, ` is available, but this copy cannot replace itself: ${get(s).why ?? ""}. Get it from `);
				set_text(text_15, get(s).where);
			});
			append($$anchor, p_11);
		};
		var consequent_17 = ($$anchor) => {
			var p_12 = root_6$2();
			var text_16 = only_child(p_12);
			template_effect(() => set_text(text_16, `Downloading and verifying ${get(s).version ?? ""}. Nothing is replaced unless its signature verifies.`));
			append($$anchor, p_12);
		};
		if_block(node_1, ($$render) => {
			if (get(s).state === "not_asked") $$render(consequent_3);
			else if (get(s).state === "off") $$render(consequent_4, 1);
			else if (get(s).state === "unavailable") $$render(consequent_5, 2);
			else if (get(s).state === "checking") $$render(consequent_6, 3);
			else if (get(s).state === "current") $$render(consequent_7, 4);
			else if (get(s).state === "check_failed") $$render(consequent_8, 5);
			else if (get(s).state === "untrusted") $$render(consequent_9, 6);
			else if (get(s).state === "offer" || get(s).state === "dismissed" || get(s).state === "failed") $$render(consequent_15, 7);
			else if (get(s).state === "cannot_replace") $$render(consequent_16, 8);
			else if (get(s).state === "installing") $$render(consequent_17, 9);
		});
		var node_7 = sibling(node_1, 2);
		var consequent_18 = ($$anchor) => {
			{
				let $0 = user_derived(() => ({ updates: get(pending).pref }));
				let $1 = user_derived(() => get(pending).pref === "on" ? "Turn update checks on" : "Turn update checks off");
				PendingWrite($$anchor, {
					get pending() {
						return get(pending).p;
					},
					url: "/api/app/updates",
					get body() {
						return get($0);
					},
					get confirmLabel() {
						return get($1);
					},
					oncancel: () => set(pending, null),
					ondone: () => {
						set(pending, null);
						read.reload();
						$$props.onchanged?.();
					}
				});
			}
		};
		var consequent_21 = ($$anchor) => {
			var div_3 = root_15$2();
			var node_8 = child(div_3);
			var consequent_19 = ($$anchor) => {
				var button_4 = root_14$2();
				var text_17 = only_child(button_4, true);
				template_effect(() => set_text(text_17, get(s).state === "not_asked" ? "Yes, check — show the change" : "Turn checks on — show the change"));
				delegated("click", button_4, () => preview("on"));
				append($$anchor, button_4);
			};
			if_block(node_8, ($$render) => {
				if (get(s).state === "not_asked" || get(s).state === "off") $$render(consequent_19);
			});
			var node_9 = sibling(node_8, 2);
			var consequent_20 = ($$anchor) => {
				var button_5 = root_14$2();
				var text_18 = only_child(button_5, true);
				template_effect(() => set_text(text_18, get(s).state === "not_asked" ? "No — show the change" : "Turn checks off — show the change"));
				delegated("click", button_5, () => preview("off"));
				append($$anchor, button_5);
			};
			if_block(node_9, ($$render) => {
				if (get(s).state !== "off") $$render(consequent_20);
			});
			reset(div_3);
			append($$anchor, div_3);
		};
		if_block(node_7, ($$render) => {
			if (get(pending)) $$render(consequent_18);
			else if (get(s).state !== "unavailable" && get(s).state !== "installing") $$render(consequent_21, 1);
		});
		var node_10 = sibling(node_7, 2);
		var consequent_22 = ($$anchor) => {
			var p_13 = root_16$2();
			var text_19 = only_child(p_13, true);
			template_effect(() => set_text(text_19, get(said)));
			append($$anchor, p_13);
		};
		if_block(node_10, ($$render) => {
			if (get(said)) $$render(consequent_22);
		});
		append($$anchor, fragment_1);
	};
	if_block(node, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent);
		else if (!get(s)) $$render(consequent_1, 1);
		else if (get(s).state === "not_this_build") $$render(consequent_2, 2);
		else $$render(alternate_1, -1);
	});
	reset(div);
	template_effect(() => set_attribute(div, "data-update-state", get(s)?.state ?? ""));
	append($$anchor, div);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Checklist.svelte
var root$1 = from_html(`<span class="count svelte-vlt0ea"> </span>`);
var root_1$1 = from_html(`<option> </option>`);
var root_2$1 = from_html(`<label class="pick svelte-vlt0ea">Project <select aria-label="the project the last steps are about"></select></label>`);
var root_3$1 = from_html(`<p class="lead svelte-vlt0ea">Nothing is configured until you confirm the exact change it makes. Every step can be skipped.</p>`);
var root_4$1 = from_html(`<p class="quiet svelte-vlt0ea">Every step is done or skipped. <button type="button" class="small svelte-vlt0ea">Show the first-run checklist</button></p>`);
var root_5$1 = from_html(`<span class="writes svelte-vlt0ea"> </span>`);
var root_6$1 = from_html(`<button type="button" class="small svelte-vlt0ea">Go back</button>`);
var root_7$1 = from_html(`<button type="button" class="small svelte-vlt0ea">Skip</button>`);
var root_8$1 = from_html(`<span class="sr-only svelte-vlt0ea" data-first-run-step="update" data-first-run-done="true"> </span>`);
var root_9$1 = from_html(`<p class="svelte-vlt0ea">A registered project's remote is on GitHub. Signing in lets Devplane read its issues and pull requests; skipping holds nothing else.</p> <div><button type="button">Open the GitHub sign-in</button></div>`, 1);
var root_10$1 = from_html(`<a> </a>`);
var root_11$1 = from_html(`<b> </b>`);
var root_12$1 = from_html(`<p class="latest svelte-vlt0ea"><!> <!></p>`);
var root_13$1 = from_html(`<!> <div class="go svelte-vlt0ea"><button type="button" class="primary svelte-vlt0ea"><!> New change</button> <span class="quiet svelte-vlt0ea">The checklist ends when the change reads verified.</span></div>`, 1);
var root_14$1 = from_html(`<div class="body svelte-vlt0ea"><!></div>`);
var root_15$1 = from_html(`<li><div class="line svelte-vlt0ea"><span class="mark svelte-vlt0ea" aria-hidden="true"><!></span> <span class="title svelte-vlt0ea"> </span> <span class="state svelte-vlt0ea"> </span> <span class="says svelte-vlt0ea"><!></span> <!> <span class="gap svelte-vlt0ea"></span> <!></div> <!> <!></li>`);
var root_16$1 = from_html(`<ol class="steps svelte-vlt0ea"></ol>`);
var root_17$1 = from_html(`<section class="checklist svelte-vlt0ea" aria-labelledby="first-run-title"><header class="svelte-vlt0ea"><h2 id="first-run-title" class="svelte-vlt0ea"><!> First run</h2> <!> <span class="gap svelte-vlt0ea"></span> <!></header> <!> <!></section>`);
function Checklist($$anchor, $$props) {
	push($$props, true);
	let changeSurface = prop($$props, "changeSurface", 3, "");
	let chosen = state("");
	const read = resource(() => get(chosen) ? `/api/first-run?project=${encodeURIComponent(get(chosen))}` : "/api/first-run", {
		every: 3e3,
		tell: () => "devplane doctor"
	});
	const data = user_derived(() => read.data);
	let skipped = state(proxy(skippedSteps()));
	function skip(id) {
		set(skipped, [.../* @__PURE__ */ new Set([...get(skipped), id])], true);
		keepSkipped(get(skipped));
	}
	function back(id) {
		set(skipped, get(skipped).filter((x) => x !== id), true);
		keepSkipped(get(skipped));
	}
	const open = user_derived(() => get(data)?.steps.find((s) => s.state !== "done" && !get(skipped).includes(s.id))?.id ?? "");
	const doneCount = user_derived(() => get(data) ? get(data).steps.filter((s) => s.state === "done").length : 0);
	const complete = user_derived(() => !!get(data) && !incomplete(get(data).steps, get(skipped)));
	let expanded = state(false);
	let now = state(proxy(Date.now()));
	user_effect(() => {
		const t = setInterval(() => set(now, Date.now(), true), 1e3);
		return () => clearInterval(t);
	});
	function newChange() {
		try {
			if (get(data)?.project) sessionStorage.setItem("vp-new-project", get(data).project);
		} catch {}
		run("new-change", "");
	}
	const reload = () => void read.reload();
	const word = (s) => get(skipped).includes(s.id) && s.state !== "done" ? "skipped" : s.id === get(open) ? "next" : s.state;
	var section = root_17$1();
	var header = child(section);
	var h2 = child(header);
	var node = child(h2);
	Icon(node, {
		name: "check",
		size: 14
	});
	next();
	reset(h2);
	var node_1 = sibling(h2, 2);
	var consequent = ($$anchor) => {
		var span = root$1();
		var text = only_child(span);
		template_effect(($0) => set_text(text, `${get(doneCount) ?? ""} of ${get(data).steps.length ?? ""} done${$0 ?? ""}`), [() => get(skipped).length ? ` · ${get(data).steps.filter((s) => get(skipped).includes(s.id) && s.state !== "done").length} skipped` : ""]);
		append($$anchor, span);
	};
	if_block(node_1, ($$render) => {
		if (get(data)) $$render(consequent);
	});
	var node_2 = sibling(node_1, 4);
	var consequent_1 = ($$anchor) => {
		var label = root_2$1();
		var select = sibling(child(label));
		each(select, 21, () => get(data).trusted, (p) => p.id, ($$anchor, p) => {
			var option = root_1$1();
			var text_1 = only_child(option, true);
			var option_value = {};
			template_effect(() => {
				set_selected(option, get(p).id === get(data).project);
				set_text(text_1, get(p).name);
				if (option_value !== (option_value = get(p).id)) option.value = (option.__value = option_value) ?? "";
			});
			append($$anchor, option);
		});
		reset(select);
		init_select(select);
		reset(label);
		bind_select_value(select, () => get(chosen), ($$value) => set(chosen, $$value));
		append($$anchor, label);
	};
	if_block(node_2, ($$render) => {
		if (get(data) && get(data).trusted.length > 1) $$render(consequent_1);
	});
	reset(header);
	var node_3 = sibling(header, 2);
	var consequent_2 = ($$anchor) => {
		var p_1 = root_3$1();
		append($$anchor, p_1);
	};
	if_block(node_3, ($$render) => {
		if (!get(complete) || get(expanded)) $$render(consequent_2);
	});
	var node_4 = sibling(node_3, 2);
	var consequent_3 = ($$anchor) => {
		Failed($$anchor, {
			what: "the first-run checklist",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_4 = ($$anchor) => {
		Skeleton($$anchor, { rows: 7 });
	};
	var consequent_5 = ($$anchor) => {
		var p_2 = root_4$1();
		var button = sibling(child(p_2));
		reset(p_2);
		delegated("click", button, () => set(expanded, true));
		append($$anchor, p_2);
	};
	var alternate_2 = ($$anchor) => {
		var ol = root_16$1();
		each(ol, 23, () => get(data).steps, (s) => s.id, ($$anchor, s, i) => {
			const w = user_derived(() => word(get(s)));
			var li = root_15$1();
			var div = child(li);
			var span_1 = child(div);
			var node_5 = child(span_1);
			var consequent_6 = ($$anchor) => {
				Icon($$anchor, {
					name: "check",
					size: 13
				});
			};
			var consequent_7 = ($$anchor) => {
				Icon($$anchor, {
					name: "skip",
					size: 13
				});
			};
			var alternate = ($$anchor) => {
				var text_2 = text();
				template_effect(() => set_text(text_2, get(i) + 1));
				append($$anchor, text_2);
			};
			if_block(node_5, ($$render) => {
				if (get(w) === "done") $$render(consequent_6);
				else if (get(w) === "skipped") $$render(consequent_7, 1);
				else $$render(alternate, -1);
			});
			reset(span_1);
			var span_2 = sibling(span_1, 2);
			var text_3 = only_child(span_2, true);
			var span_3 = sibling(span_2, 2);
			var text_4 = only_child(span_3, true);
			var span_4 = sibling(span_3, 2);
			var node_6 = child(span_4);
			Inline(node_6, { get text() {
				return get(s).says;
			} });
			reset(span_4);
			var node_7 = sibling(span_4, 2);
			var consequent_8 = ($$anchor) => {
				var span_5 = root_5$1();
				var text_5 = only_child(span_5);
				template_effect(() => set_text(text_5, `would write ${get(s).writes ?? ""}`));
				append($$anchor, span_5);
			};
			if_block(node_7, ($$render) => {
				if (get(s).writes && get(w) !== "done") $$render(consequent_8);
			});
			var node_8 = sibling(node_7, 4);
			var consequent_9 = ($$anchor) => {
				var button_1 = root_6$1();
				delegated("click", button_1, () => back(get(s).id));
				append($$anchor, button_1);
			};
			var consequent_10 = ($$anchor) => {
				var button_2 = root_7$1();
				delegated("click", button_2, () => skip(get(s).id));
				append($$anchor, button_2);
			};
			if_block(node_8, ($$render) => {
				if (get(w) === "skipped") $$render(consequent_9);
				else if (get(w) !== "done" && !get(s).advances_without_input && get(s).id !== "connect") $$render(consequent_10, 1);
			});
			reset(div);
			var node_9 = sibling(div, 2);
			var consequent_11 = ($$anchor) => {
				var span_6 = root_8$1();
				var text_6 = only_child(span_6);
				template_effect(() => set_text(text_6, `update checks: ${get(s).says ?? ""}`));
				append($$anchor, span_6);
			};
			if_block(node_9, ($$render) => {
				if (get(s).id === "updates" && get(s).state === "done") $$render(consequent_11);
			});
			var node_10 = sibling(node_9, 2);
			var consequent_22 = ($$anchor) => {
				var div_1 = root_14$1();
				var node_11 = child(div_1);
				var consequent_12 = ($$anchor) => {
					Updates($$anchor, { onchanged: reload });
				};
				var consequent_13 = ($$anchor) => {
					Connect($$anchor, {
						get agents() {
							return get(data).agents;
						},
						firstRun: true,
						onchanged: reload,
						onskip: () => skip("connect")
					});
				};
				var consequent_14 = ($$anchor) => {
					Trust($$anchor, {
						firstRun: true,
						ontrusted: reload
					});
				};
				var consequent_15 = ($$anchor) => {
					var fragment_8 = root_9$1();
					var div_2 = sibling(first_child(fragment_8), 2);
					var button_3 = only_child(div_2);
					delegated("click", button_3, () => $$props.ongithub?.());
					append($$anchor, fragment_8);
				};
				var consequent_16 = ($$anchor) => {
					Declare($$anchor, {
						get project() {
							return get(data).project;
						},
						firstRun: true,
						ondone: reload
					});
				};
				var consequent_21 = ($$anchor) => {
					var fragment_10 = root_13$1();
					var node_12 = first_child(fragment_10);
					var consequent_20 = ($$anchor) => {
						var p_3 = root_12$1();
						var node_13 = child(p_3);
						var consequent_17 = ($$anchor) => {
							var a = root_10$1();
							var text_7 = only_child(a, true);
							template_effect(($0) => {
								set_attribute(a, "href", $0);
								set_text(text_7, get(data).latest.title);
							}, [() => `#${changeSurface()}/${encodeURIComponent(get(data).latest.id)}`]);
							append($$anchor, a);
						};
						var alternate_1 = ($$anchor) => {
							var b = root_11$1();
							var text_8 = only_child(b, true);
							template_effect(() => set_text(text_8, get(data).latest.title));
							append($$anchor, b);
						};
						if_block(node_13, ($$render) => {
							if (changeSurface()) $$render(consequent_17);
							else $$render(alternate_1, -1);
						});
						var text_9 = sibling(node_13);
						var node_14 = sibling(text_9);
						var consequent_18 = ($$anchor) => {
							var text_10 = text();
							template_effect(() => set_text(text_10, `· not verified — ${get(data).latest.check_failed ?? ""}; the gate's output is on the change's Gates tab`));
							append($$anchor, text_10);
						};
						var consequent_19 = ($$anchor) => {
							var text_11 = text();
							template_effect(($0) => set_text(text_11, `· started ${$0 ?? ""} ago`), [() => ago(Math.max(0, (get(now) - Date.parse(get(data).latest.created_at)) / 1e3))]);
							append($$anchor, text_11);
						};
						if_block(node_14, ($$render) => {
							if (get(data).latest.check_failed) $$render(consequent_18);
							else if (get(data).latest.state !== "verified" && get(data).latest.created_at) $$render(consequent_19, 1);
						});
						reset(p_3);
						template_effect(() => set_text(text_9, ` · ${get(data).latest.state ?? ""} `));
						append($$anchor, p_3);
					};
					if_block(node_12, ($$render) => {
						if (get(data).latest) $$render(consequent_20);
					});
					var div_3 = sibling(node_12, 2);
					var button_4 = child(div_3);
					var node_15 = child(button_4);
					Icon(node_15, {
						name: "plus",
						size: 13
					});
					next();
					reset(button_4);
					next(2);
					reset(div_3);
					delegated("click", button_4, newChange);
					append($$anchor, fragment_10);
				};
				if_block(node_11, ($$render) => {
					if (get(s).id === "updates") $$render(consequent_12);
					else if (get(s).id === "connect") $$render(consequent_13, 1);
					else if (get(s).id === "trust") $$render(consequent_14, 2);
					else if (get(s).id === "github") $$render(consequent_15, 3);
					else if (get(s).id === "declare" && get(data).project) $$render(consequent_16, 4);
					else if (get(s).id === "first_change") $$render(consequent_21, 5);
				});
				reset(div_1);
				append($$anchor, div_1);
			};
			if_block(node_10, ($$render) => {
				if (get(s).id === get(open)) $$render(consequent_22);
			});
			reset(li);
			template_effect(() => {
				set_class(li, 1, `step ${get(w) ?? ""}`, "svelte-vlt0ea");
				set_attribute(li, "data-first-run-row", get(s).id);
				set_text(text_3, TITLES[get(s).id] ?? get(s).id);
				set_text(text_4, get(w));
			});
			append($$anchor, li);
		});
		reset(ol);
		append($$anchor, ol);
	};
	if_block(node_4, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_3);
		else if (!get(data)) $$render(consequent_4, 1);
		else if (get(complete) && !get(expanded)) $$render(consequent_5, 2);
		else $$render(alternate_2, -1);
	});
	reset(section);
	append($$anchor, section);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/setup/Setup.svelte
var root = from_html(`<section class="card wide declare-at svelte-1y95tkv" aria-label="declare a check"><h2 class="svelte-1y95tkv"><!> Declare a check</h2> <!> <p class="svelte-1y95tkv"><a href="#setup">Back to Setup</a></p></section>`);
var root_1 = from_html(`<p class="quiet svelte-1y95tkv">No agent settings file was found. <code class="svelte-1y95tkv">devplane agent connect claude</code> writes the hooks, and <code class="svelte-1y95tkv">devplane agent disconnect claude</code> removes exactly what it wrote.</p>`);
var root_2 = from_html(`<span id="setup-github" class="anchor svelte-1y95tkv"></span> <!>`, 1);
var root_3 = from_html(`<p class="quiet svelte-1y95tkv">No <code class="svelte-1y95tkv">~/.devplane/policy.toml</code>. Rules live in each project's <code class="svelte-1y95tkv">devplane.toml</code>; a rule you want everywhere goes in that file, in the same shape.</p>`);
var root_4 = from_html(`<p class="fail svelte-1y95tkv"><!> </p>`);
var root_5 = from_html(`<code class="rule deny svelte-1y95tkv"> </code>`);
var root_6 = from_html(`<span class="quiet svelte-1y95tkv">none</span>`);
var root_7 = from_html(`<code class="rule ask svelte-1y95tkv"> </code>`);
var root_8 = from_html(`<p class="file svelte-1y95tkv"><code class="svelte-1y95tkv"> </code></p> <div class="rules svelte-1y95tkv"><div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Never</h3> <!></div> <div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Always ask</h3> <!></div></div>`, 1);
var root_9 = from_html(`<p class="nogates svelte-1y95tkv"><!> <a>Declare a check</a></p>`);
var root_10 = from_html(`<p class="quiet svelte-1y95tkv">None registered. A project is registered the first time an agent works in it, or with <code class="svelte-1y95tkv">devplane project trust &lt;path&gt;</code>.</p>`);
var root_11 = from_html(`<b> </b>`);
var root_12 = from_html(`<span class="fail svelte-1y95tkv">unreadable</span>`);
var root_13 = from_html(`<span class="ok svelte-1y95tkv">check declared</span>`);
var root_14 = from_html(`<a>Declare a check</a>`);
var root_15 = from_html(`<div class="frame svelte-1y95tkv"><!></div> <!>`, 1);
var root_16 = from_html(`<div class="grid svelte-1y95tkv"><section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> This machine</h2> <!></section> <section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Your agent</h2> <!></section> <section class="card svelte-1y95tkv" aria-label="update checks"><h2 class="svelte-1y95tkv"><!> Update checks</h2> <!></section> <!> <section class="card wide svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Rules for every project</h2> <!></section></div> <section class="projects svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Projects <span class="svelte-1y95tkv"> </span></h2> <!> <!></section>`, 1);
var root_17 = from_html(`<div class="page svelte-1y95tkv"><h1 class="svelte-1y95tkv">Setup</h1> <!> <!> <!></div>`);
function Setup($$anchor, $$props) {
	push($$props, true);
	let focus = prop($$props, "focus", 3, ""), view = prop($$props, "view", 3, ""), changeSurface = prop($$props, "changeSurface", 3, "");
	const declaring = user_derived(() => focus() === "declare" && view() ? view() : "");
	function toGitHub() {
		document.getElementById("setup-github")?.scrollIntoView({ block: "center" });
	}
	const declareHref = (p) => `#setup/declare/${encodeURIComponent(p.id)}`;
	const read = resource(() => "/api/setup", { tell: () => "devplane doctor" });
	const github = resource(() => "/api/github", {
		every: 3e3,
		tell: () => "devplane doctor"
	});
	let busy = state("");
	let said = state("");
	async function signIn(host) {
		if (get(busy)) return;
		set(busy, `starting a sign-in to ${host}`);
		set(said, "");
		try {
			await api("/api/github/login", {
				method: "POST",
				body: JSON.stringify({ host })
			});
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(busy, "");
			await github.reload();
		}
	}
	async function signOut(host) {
		if (get(busy)) return;
		set(busy, `signing out of ${host}`);
		set(said, "");
		try {
			const r = await api("/api/github/logout", {
				method: "POST",
				body: JSON.stringify({ host })
			});
			set(said, r.deleted_from ? `Signed out of ${host}: the token is deleted from ${r.deleted_from}. The grant still exists at GitHub; revoke it at ${r.revoke_at ?? "GitHub's settings"}.` : `Not signed in to ${host}; there was no token to delete.`, true);
		} catch (e) {
			set(said, e instanceof Error ? e.message : String(e), true);
		} finally {
			set(busy, "");
			await github.reload();
		}
	}
	async function copy(code) {
		set(said, await copyText(code) ? `copied ${code}` : "this page cannot reach the clipboard — type the code as shown", true);
	}
	const data = user_derived(() => read.data);
	const hooks = user_derived(() => get(data)?.connect?.hooks_installed ?? []);
	const policy = user_derived(() => get(data)?.machine_policy ?? null);
	let selected = state(null);
	let order = state(proxy([]));
	user_effect(() => cursor({
		scope: "setup",
		size: () => get(order).length,
		at: () => get(order).indexOf(get(selected) ?? ""),
		go: (i) => set(selected, get(order)[i], true),
		open: (i) => {
			const p = (get(data)?.projects ?? []).find((x) => x.id === get(order)[i]);
			if (p) go(`#inbox/${encodeURIComponent(p.name)}`);
		}
	}));
	const columns = [
		{
			key: "name",
			label: "Project",
			width: 180,
			sort: (r) => r.name
		},
		{
			key: "trusted",
			label: "Trusted",
			width: 140,
			sort: (r) => r.trusted ? 0 : 1
		},
		{
			key: "gates",
			label: "Gates",
			width: 130,
			sort: (r) => r.declares_gates ? 0 : 1
		},
		{
			key: "config",
			label: "devplane.toml",
			width: 150
		},
		{
			key: "root",
			label: "Where",
			mono: true
		}
	];
	var div = root_17();
	var node = sibling(child(div), 2);
	var consequent = ($$anchor) => {
		var section = root();
		var h2 = child(section);
		var node_1 = child(h2);
		Icon(node_1, {
			name: "gate",
			size: 14
		});
		next();
		reset(h2);
		Declare(sibling(h2, 2), {
			get project() {
				return get(declaring);
			},
			ondone: () => void read.reload()
		});
		next(2);
		reset(section);
		append($$anchor, section);
	};
	if_block(node, ($$render) => {
		if (get(declaring)) $$render(consequent);
	});
	var node_3 = sibling(node, 2);
	Checklist(node_3, {
		get changeSurface() {
			return changeSurface();
		},
		ongithub: toGitHub
	});
	var node_4 = sibling(node_3, 2);
	var consequent_1 = ($$anchor) => {
		Failed($$anchor, {
			what: "what is configured",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_2 = ($$anchor) => {
		Skeleton($$anchor, { rows: 5 });
	};
	var alternate_6 = ($$anchor) => {
		var fragment_2 = root_16();
		var div_1 = first_child(fragment_2);
		var section_1 = child(div_1);
		var h2_1 = child(section_1);
		var node_5 = child(h2_1);
		Icon(node_5, {
			name: "settings",
			size: 14
		});
		next();
		reset(h2_1);
		var node_6 = sibling(h2_1, 2);
		{
			let $0 = user_derived(() => [
				{
					label: "Version",
					value: get(data).machine?.version ?? null,
					missing: "unknown"
				},
				{
					label: "Home",
					value: get(data).machine?.home ?? null,
					mono: true
				},
				{
					label: "Record",
					value: get(data).machine?.database ?? null,
					mono: true
				}
			]);
			Props(node_6, { get rows() {
				return get($0);
			} });
		}
		reset(section_1);
		var section_2 = sibling(section_1, 2);
		var h2_2 = child(section_2);
		var node_7 = child(h2_2);
		Icon(node_7, {
			name: "agent",
			size: 14
		});
		next();
		reset(h2_2);
		var node_8 = sibling(h2_2, 2);
		var consequent_3 = ($$anchor) => {
			var p_1 = root_1();
			append($$anchor, p_1);
		};
		var alternate = ($$anchor) => {
			{
				let $0 = user_derived(() => [
					{
						label: "Settings",
						value: get(data).connect.settings_path ?? null,
						mono: true
					},
					{
						label: "Hooks",
						value: get(hooks).length ? get(hooks).join(", ") : null,
						missing: "none installed — devplane agent connect claude"
					},
					{
						label: "Telemetry",
						value: !get(data).connect.telemetry_endpoint ? "not exporting" : get(data).connect.telemetry_is_ours ? "to Devplane" : "to somebody else's collector — left alone"
					}
				]);
				Props($$anchor, { get rows() {
					return get($0);
				} });
			}
		};
		if_block(node_8, ($$render) => {
			if (!get(data).connect) $$render(consequent_3);
			else $$render(alternate, -1);
		});
		reset(section_2);
		var section_3 = sibling(section_2, 2);
		var h2_3 = child(section_3);
		var node_9 = child(h2_3);
		Icon(node_9, {
			name: "refresh",
			size: 14
		});
		next();
		reset(h2_3);
		Updates(sibling(h2_3, 2), {});
		reset(section_3);
		var node_11 = sibling(section_3, 2);
		var consequent_4 = ($$anchor) => {
			Failed($$anchor, {
				what: "the GitHub sign-in",
				get failure() {
					return github.failure;
				}
			});
		};
		var alternate_1 = ($$anchor) => {
			var fragment_5 = root_2();
			var node_12 = sibling(first_child(fragment_5), 2);
			{
				let $0 = user_derived(() => github.data?.hosts ?? []);
				let $1 = user_derived(() => github.data?.client_id ?? true);
				GitHubPanel(node_12, {
					get hosts() {
						return get($0);
					},
					get clientId() {
						return get($1);
					},
					get busy() {
						return get(busy);
					},
					get said() {
						return get(said);
					},
					signIn,
					signOut,
					copy
				});
			}
			append($$anchor, fragment_5);
		};
		if_block(node_11, ($$render) => {
			if (github.phase === "failed" && github.failure) $$render(consequent_4);
			else $$render(alternate_1, -1);
		});
		var section_4 = sibling(node_11, 2);
		var h2_4 = child(section_4);
		var node_13 = child(h2_4);
		Icon(node_13, {
			name: "shield",
			size: 14
		});
		next();
		reset(h2_4);
		var node_14 = sibling(h2_4, 2);
		var consequent_5 = ($$anchor) => {
			var p_2 = root_3();
			append($$anchor, p_2);
		};
		var consequent_6 = ($$anchor) => {
			var p_3 = root_4();
			var node_15 = child(p_3);
			Icon(node_15, {
				name: "alert",
				size: 13
			});
			var text = sibling(node_15);
			reset(p_3);
			template_effect(() => set_text(text, ` ${get(policy).path ?? ""} will not parse — every gated call asks until it does: ${get(policy).error ?? ""}`));
			append($$anchor, p_3);
		};
		var alternate_2 = ($$anchor) => {
			var fragment_6 = root_8();
			var p_4 = first_child(fragment_6);
			var code_1 = child(p_4);
			var text_1 = only_child(code_1, true);
			reset(p_4);
			var div_2 = sibling(p_4, 2);
			var div_3 = child(div_2);
			var node_16 = sibling(child(div_3), 2);
			each(node_16, 16, () => get(policy).deny ?? [], (r) => r, ($$anchor, r) => {
				var code_2 = root_5();
				var text_2 = only_child(code_2, true);
				template_effect(() => set_text(text_2, r));
				append($$anchor, code_2);
			}, ($$anchor) => {
				var span = root_6();
				append($$anchor, span);
			});
			reset(div_3);
			var div_4 = sibling(div_3, 2);
			var node_17 = sibling(child(div_4), 2);
			each(node_17, 16, () => get(policy).ask ?? [], (r) => r, ($$anchor, r) => {
				var code_3 = root_7();
				var text_3 = only_child(code_3, true);
				template_effect(() => set_text(text_3, r));
				append($$anchor, code_3);
			}, ($$anchor) => {
				var span_1 = root_6();
				append($$anchor, span_1);
			});
			reset(div_4);
			reset(div_2);
			template_effect(() => set_text(text_1, get(policy).path));
			append($$anchor, fragment_6);
		};
		if_block(node_14, ($$render) => {
			if (!get(policy) || !get(policy).exists) $$render(consequent_5);
			else if (get(policy).error) $$render(consequent_6, 1);
			else $$render(alternate_2, -1);
		});
		reset(section_4);
		reset(div_1);
		var section_5 = sibling(div_1, 2);
		var h2_5 = child(section_5);
		var node_18 = child(h2_5);
		Icon(node_18, {
			name: "folder",
			size: 14
		});
		var span_2 = sibling(node_18, 2);
		var text_4 = only_child(span_2, true);
		reset(h2_5);
		var node_19 = sibling(h2_5, 2);
		each(node_19, 17, () => (get(data).projects ?? []).filter((p) => p.trusted && !p.error && !p.declares_gates), (p) => p.id, ($$anchor, p) => {
			var p_5 = root_9();
			var node_20 = child(p_5);
			Icon(node_20, {
				name: "alert",
				size: 13
			});
			var text_5 = sibling(node_20);
			var a = sibling(text_5);
			reset(p_5);
			template_effect(($0) => {
				set_text(text_5, ` ${get(p).name ?? ""}: no gates declared — changes here cannot become verified. `);
				set_attribute(a, "href", $0);
			}, [() => declareHref(get(p))]);
			append($$anchor, p_5);
		});
		var node_21 = sibling(node_19, 2);
		var consequent_7 = ($$anchor) => {
			var p_6 = root_10();
			append($$anchor, p_6);
		};
		var alternate_5 = ($$anchor) => {
			var fragment_7 = root_15();
			var div_5 = first_child(fragment_7);
			var node_22 = child(div_5);
			{
				const cell = ($$anchor, r = noop, c = noop) => {
					var fragment_8 = comment();
					var node_23 = first_child(fragment_8);
					var consequent_8 = ($$anchor) => {
						var b = root_11();
						var text_6 = only_child(b, true);
						template_effect(() => set_text(text_6, r().name));
						append($$anchor, b);
					};
					var consequent_9 = ($$anchor) => {
						{
							let $0 = user_derived(() => r().trusted ? "trusted" : "not trusted");
							let $1 = user_derived(() => r().trusted ? "none" : "wait");
							Pill($$anchor, {
								get word() {
									return get($0);
								},
								get as() {
									return get($1);
								}
							});
						}
					};
					var consequent_12 = ($$anchor) => {
						var fragment_10 = comment();
						var node_24 = first_child(fragment_10);
						var consequent_10 = ($$anchor) => {
							var span_3 = root_12();
							append($$anchor, span_3);
						};
						var consequent_11 = ($$anchor) => {
							var span_4 = root_13();
							append($$anchor, span_4);
						};
						var alternate_3 = ($$anchor) => {
							var a_1 = root_14();
							template_effect(($0) => set_attribute(a_1, "href", $0), [() => declareHref(r())]);
							append($$anchor, a_1);
						};
						if_block(node_24, ($$render) => {
							if (r().error) $$render(consequent_10);
							else if (r().declares_gates) $$render(consequent_11, 1);
							else $$render(alternate_3, -1);
						});
						append($$anchor, fragment_10);
					};
					var consequent_13 = ($$anchor) => {
						var text_7 = text();
						template_effect(() => set_text(text_7, r().config?.exists ? "present" : "absent"));
						append($$anchor, text_7);
					};
					var alternate_4 = ($$anchor) => {
						var text_8 = text();
						template_effect(() => set_text(text_8, r().root));
						append($$anchor, text_8);
					};
					if_block(node_23, ($$render) => {
						if (c().key === "name") $$render(consequent_8);
						else if (c().key === "trusted") $$render(consequent_9, 1);
						else if (c().key === "gates") $$render(consequent_12, 2);
						else if (c().key === "config") $$render(consequent_13, 3);
						else $$render(alternate_4, -1);
					});
					append($$anchor, fragment_8);
				};
				let $0 = user_derived(() => get(data).projects ?? []);
				Grid(node_22, {
					id: "setup-projects",
					get columns() {
						return columns;
					},
					get rows() {
						return get($0);
					},
					key: (r) => r.id,
					label: "projects",
					dense: true,
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
					$$slots: { cell: true }
				});
			}
			reset(div_5);
			var node_25 = sibling(div_5, 2);
			each(node_25, 17, () => (get(data).projects ?? []).filter((p) => p.error), (p) => p.id, ($$anchor, p) => {
				var p_7 = root_4();
				var node_26 = child(p_7);
				Icon(node_26, {
					name: "alert",
					size: 13
				});
				var text_9 = sibling(node_26);
				reset(p_7);
				template_effect(() => set_text(text_9, ` ${get(p).name ?? ""}: ${get(p).error ?? ""}`));
				append($$anchor, p_7);
			});
			append($$anchor, fragment_7);
		};
		if_block(node_21, ($$render) => {
			if ((get(data).projects ?? []).length === 0) $$render(consequent_7);
			else $$render(alternate_5, -1);
		});
		reset(section_5);
		template_effect(() => set_text(text_4, get(data).projects?.length ?? 0));
		append($$anchor, fragment_2);
	};
	if_block(node_4, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent_1);
		else if (!get(data)) $$render(consequent_2, 1);
		else $$render(alternate_6, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
export { Setup as default };

//# sourceMappingURL=Setup.js.map