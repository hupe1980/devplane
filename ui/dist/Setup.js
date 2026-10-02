import { A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, J as proxy, K as only_child, L as get, N as text, O as set_text, P as delegate, V as template_effect, W as child, X as state, Y as set, Z as user_derived, at as noop, et as pop, i as api, it as reset, j as from_html, k as append, m as set_attribute, o as prop, q as sibling, rt as next, tt as push, v as set_class } from "./api.js";
import { a as Inline, h as resource, i as Skeleton, n as Icon, p as copyText, t as Pill, w as go } from "./app.js";
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
var root$1 = from_html(`<button>Sign out</button>`);
var root_1$1 = from_html(`<button>Sign in</button>`);
var root_2$1 = from_html(`<p class="said svelte-t93vvx"> </p>`);
var root_3$1 = from_html(`<p class="quiet svelte-t93vvx"> <code class="svelte-t93vvx"> </code> signs in with GitHub CLI's token, or pipe a fine-grained personal access token to the same command.</p>`);
var root_4$1 = from_html(`<div class="code svelte-t93vvx"><p class="svelte-t93vvx">Open <a target="_blank" rel="noopener noreferrer"> </a> and enter:</p> <p class="big svelte-t93vvx"> </p> <button>Copy code</button></div>`);
var root_5$1 = from_html(`<div class="host svelte-t93vvx"><div class="line svelte-t93vvx"><b> </b> <span> </span> <!></div> <!> <!> <!></div>`);
var root_6$1 = from_html(`<p class="quiet svelte-t93vvx"> </p>`);
var root_7$1 = from_html(`<p class="said svelte-t93vvx"><!></p>`);
var root_8$1 = from_html(`<section class="card wide svelte-t93vvx" id="github"><h2 class="svelte-t93vvx"><!> GitHub</h2> <!> <!> <!> <p class="quiet svelte-t93vvx">The token is kept only in this machine's credential store — never in a file, a log or this page.</p></section>`);
function GitHubPanel($$anchor, $$props) {
	push($$props, true);
	let hosts = prop($$props, "hosts", 19, () => []), clientId = prop($$props, "clientId", 3, true), busy = prop($$props, "busy", 3, ""), said = prop($$props, "said", 3, ""), signIn = prop($$props, "signIn", 3, () => {}), signOut = prop($$props, "signOut", 3, () => {}), copy = prop($$props, "copy", 3, () => {});
	var section = root_8$1();
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
		var div = root_5$1();
		var div_1 = child(div);
		var b = child(div_1);
		var text = only_child(b, true);
		var span = sibling(b, 2);
		let classes;
		var text_1 = only_child(span, true);
		var node_2 = sibling(span, 2);
		var consequent = ($$anchor) => {
			var button = root$1();
			template_effect(() => button.disabled = !!busy());
			delegated("click", button, () => signOut()(get(h).host));
			append($$anchor, button);
		};
		var consequent_1 = ($$anchor) => {
			var button_1 = root_1$1();
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
			var p = root_2$1();
			var text_2 = only_child(p, true);
			template_effect(() => set_text(text_2, get(h).said));
			append($$anchor, p);
		};
		if_block(node_3, ($$render) => {
			if (get(h).said) $$render(consequent_2);
		});
		var node_4 = sibling(node_3, 2);
		var consequent_3 = ($$anchor) => {
			var p_1 = root_3$1();
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
			var div_2 = root_4$1();
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
		var p_4 = root_6$1();
		var text_7 = only_child(p_4);
		template_effect(() => set_text(text_7, `${busy() ?? ""}…`));
		append($$anchor, p_4);
	};
	if_block(node_6, ($$render) => {
		if (busy()) $$render(consequent_5);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_6 = ($$anchor) => {
		var p_5 = root_7$1();
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
//#region src/surfaces/setup/Setup.svelte
var root = from_html(`<p class="quiet svelte-1y95tkv">No agent settings file was found. <code class="svelte-1y95tkv">devplane agent connect claude</code> writes the hooks, and <code class="svelte-1y95tkv">devplane agent disconnect claude</code> removes exactly what it wrote.</p>`);
var root_1 = from_html(`<p class="quiet svelte-1y95tkv">No <code class="svelte-1y95tkv">~/.devplane/policy.toml</code>. Rules live in each project's <code class="svelte-1y95tkv">devplane.toml</code>; a rule you want everywhere goes in that file, in the same shape.</p>`);
var root_2 = from_html(`<p class="fail svelte-1y95tkv"><!> </p>`);
var root_3 = from_html(`<code class="rule deny svelte-1y95tkv"> </code>`);
var root_4 = from_html(`<span class="quiet svelte-1y95tkv">none</span>`);
var root_5 = from_html(`<code class="rule ask svelte-1y95tkv"> </code>`);
var root_6 = from_html(`<p class="file svelte-1y95tkv"><code class="svelte-1y95tkv"> </code></p> <div class="rules svelte-1y95tkv"><div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Never</h3> <!></div> <div class="svelte-1y95tkv"><h3 class="svelte-1y95tkv">Always ask</h3> <!></div></div>`, 1);
var root_7 = from_html(`<p class="quiet svelte-1y95tkv">None registered. A project is registered the first time an agent works in it, or with <code class="svelte-1y95tkv">devplane project trust &lt;path&gt;</code>.</p>`);
var root_8 = from_html(`<b> </b>`);
var root_9 = from_html(`<span class="fail svelte-1y95tkv">unreadable</span>`);
var root_10 = from_html(`<span class="ok svelte-1y95tkv">declared</span>`);
var root_11 = from_html(`<span class="quiet svelte-1y95tkv">none declared</span>`);
var root_12 = from_html(`<div class="frame svelte-1y95tkv"><!></div> <!>`, 1);
var root_13 = from_html(`<div class="grid svelte-1y95tkv"><section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> This machine</h2> <!></section> <section class="card svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Your agent</h2> <!></section> <!> <section class="card wide svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Rules for every project</h2> <!></section></div> <section class="projects svelte-1y95tkv"><h2 class="svelte-1y95tkv"><!> Projects <span class="svelte-1y95tkv"> </span></h2> <!></section>`, 1);
var root_14 = from_html(`<div class="page svelte-1y95tkv"><h1 class="svelte-1y95tkv">Setup</h1> <!></div>`);
function Setup($$anchor, $$props) {
	push($$props, true);
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
	var div = root_14();
	var node = sibling(child(div), 2);
	var consequent = ($$anchor) => {
		Failed($$anchor, {
			what: "what is configured",
			get failure() {
				return read.failure;
			}
		});
	};
	var consequent_1 = ($$anchor) => {
		Skeleton($$anchor, { rows: 5 });
	};
	var alternate_6 = ($$anchor) => {
		var fragment_2 = root_13();
		var div_1 = first_child(fragment_2);
		var section = child(div_1);
		var h2 = child(section);
		var node_1 = child(h2);
		Icon(node_1, {
			name: "settings",
			size: 14
		});
		next();
		reset(h2);
		var node_2 = sibling(h2, 2);
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
			Props(node_2, { get rows() {
				return get($0);
			} });
		}
		reset(section);
		var section_1 = sibling(section, 2);
		var h2_1 = child(section_1);
		var node_3 = child(h2_1);
		Icon(node_3, {
			name: "agent",
			size: 14
		});
		next();
		reset(h2_1);
		var node_4 = sibling(h2_1, 2);
		var consequent_2 = ($$anchor) => {
			var p_1 = root();
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
		if_block(node_4, ($$render) => {
			if (!get(data).connect) $$render(consequent_2);
			else $$render(alternate, -1);
		});
		reset(section_1);
		var node_5 = sibling(section_1, 2);
		var consequent_3 = ($$anchor) => {
			Failed($$anchor, {
				what: "the GitHub sign-in",
				get failure() {
					return github.failure;
				}
			});
		};
		var alternate_1 = ($$anchor) => {
			{
				let $0 = user_derived(() => github.data?.hosts ?? []);
				let $1 = user_derived(() => github.data?.client_id ?? true);
				GitHubPanel($$anchor, {
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
		};
		if_block(node_5, ($$render) => {
			if (github.phase === "failed" && github.failure) $$render(consequent_3);
			else $$render(alternate_1, -1);
		});
		var section_2 = sibling(node_5, 2);
		var h2_2 = child(section_2);
		var node_6 = child(h2_2);
		Icon(node_6, {
			name: "shield",
			size: 14
		});
		next();
		reset(h2_2);
		var node_7 = sibling(h2_2, 2);
		var consequent_4 = ($$anchor) => {
			var p_2 = root_1();
			append($$anchor, p_2);
		};
		var consequent_5 = ($$anchor) => {
			var p_3 = root_2();
			var node_8 = child(p_3);
			Icon(node_8, {
				name: "alert",
				size: 13
			});
			var text = sibling(node_8);
			reset(p_3);
			template_effect(() => set_text(text, ` ${get(policy).path ?? ""} will not parse — every gated call asks until it does: ${get(policy).error ?? ""}`));
			append($$anchor, p_3);
		};
		var alternate_2 = ($$anchor) => {
			var fragment_6 = root_6();
			var p_4 = first_child(fragment_6);
			var code_1 = child(p_4);
			var text_1 = only_child(code_1, true);
			reset(p_4);
			var div_2 = sibling(p_4, 2);
			var div_3 = child(div_2);
			var node_9 = sibling(child(div_3), 2);
			each(node_9, 16, () => get(policy).deny ?? [], (r) => r, ($$anchor, r) => {
				var code_2 = root_3();
				var text_2 = only_child(code_2, true);
				template_effect(() => set_text(text_2, r));
				append($$anchor, code_2);
			}, ($$anchor) => {
				var span = root_4();
				append($$anchor, span);
			});
			reset(div_3);
			var div_4 = sibling(div_3, 2);
			var node_10 = sibling(child(div_4), 2);
			each(node_10, 16, () => get(policy).ask ?? [], (r) => r, ($$anchor, r) => {
				var code_3 = root_5();
				var text_3 = only_child(code_3, true);
				template_effect(() => set_text(text_3, r));
				append($$anchor, code_3);
			}, ($$anchor) => {
				var span_1 = root_4();
				append($$anchor, span_1);
			});
			reset(div_4);
			reset(div_2);
			template_effect(() => set_text(text_1, get(policy).path));
			append($$anchor, fragment_6);
		};
		if_block(node_7, ($$render) => {
			if (!get(policy) || !get(policy).exists) $$render(consequent_4);
			else if (get(policy).error) $$render(consequent_5, 1);
			else $$render(alternate_2, -1);
		});
		reset(section_2);
		reset(div_1);
		var section_3 = sibling(div_1, 2);
		var h2_3 = child(section_3);
		var node_11 = child(h2_3);
		Icon(node_11, {
			name: "folder",
			size: 14
		});
		var span_2 = sibling(node_11, 2);
		var text_4 = only_child(span_2, true);
		reset(h2_3);
		var node_12 = sibling(h2_3, 2);
		var consequent_6 = ($$anchor) => {
			var p_5 = root_7();
			append($$anchor, p_5);
		};
		var alternate_5 = ($$anchor) => {
			var fragment_7 = root_12();
			var div_5 = first_child(fragment_7);
			var node_13 = child(div_5);
			{
				const cell = ($$anchor, r = noop, c = noop) => {
					var fragment_8 = comment();
					var node_14 = first_child(fragment_8);
					var consequent_7 = ($$anchor) => {
						var b = root_8();
						var text_5 = only_child(b, true);
						template_effect(() => set_text(text_5, r().name));
						append($$anchor, b);
					};
					var consequent_8 = ($$anchor) => {
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
					var consequent_11 = ($$anchor) => {
						var fragment_10 = comment();
						var node_15 = first_child(fragment_10);
						var consequent_9 = ($$anchor) => {
							var span_3 = root_9();
							append($$anchor, span_3);
						};
						var consequent_10 = ($$anchor) => {
							var span_4 = root_10();
							append($$anchor, span_4);
						};
						var alternate_3 = ($$anchor) => {
							var span_5 = root_11();
							append($$anchor, span_5);
						};
						if_block(node_15, ($$render) => {
							if (r().error) $$render(consequent_9);
							else if (r().declares_gates) $$render(consequent_10, 1);
							else $$render(alternate_3, -1);
						});
						append($$anchor, fragment_10);
					};
					var consequent_12 = ($$anchor) => {
						var text_6 = text();
						template_effect(() => set_text(text_6, r().config?.exists ? "present" : "absent"));
						append($$anchor, text_6);
					};
					var alternate_4 = ($$anchor) => {
						var text_7 = text();
						template_effect(() => set_text(text_7, r().root));
						append($$anchor, text_7);
					};
					if_block(node_14, ($$render) => {
						if (c().key === "name") $$render(consequent_7);
						else if (c().key === "trusted") $$render(consequent_8, 1);
						else if (c().key === "gates") $$render(consequent_11, 2);
						else if (c().key === "config") $$render(consequent_12, 3);
						else $$render(alternate_4, -1);
					});
					append($$anchor, fragment_8);
				};
				let $0 = user_derived(() => get(data).projects ?? []);
				Grid(node_13, {
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
			var node_16 = sibling(div_5, 2);
			each(node_16, 17, () => (get(data).projects ?? []).filter((p) => p.error), (p) => p.id, ($$anchor, p) => {
				var p_6 = root_2();
				var node_17 = child(p_6);
				Icon(node_17, {
					name: "alert",
					size: 13
				});
				var text_8 = sibling(node_17);
				reset(p_6);
				template_effect(() => set_text(text_8, ` ${get(p).name ?? ""}: ${get(p).error ?? ""}`));
				append($$anchor, p_6);
			});
			append($$anchor, fragment_7);
		};
		if_block(node_12, ($$render) => {
			if ((get(data).projects ?? []).length === 0) $$render(consequent_6);
			else $$render(alternate_5, -1);
		});
		reset(section_3);
		template_effect(() => set_text(text_4, get(data).projects?.length ?? 0));
		append($$anchor, fragment_2);
	};
	if_block(node, ($$render) => {
		if (read.phase === "failed" && read.failure) $$render(consequent);
		else if (!get(data)) $$render(consequent_1, 1);
		else $$render(alternate_6, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
export { Setup as default };

//# sourceMappingURL=Setup.js.map