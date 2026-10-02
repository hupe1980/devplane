import { A as append, D as if_block, G as child, H as template_effect, J as sibling, K as first_child, M as from_html, Q as user_derived, R as get, U as user_effect, X as set, Y as proxy, Z as state, at as reset, it as next, j as comment, k as set_text, m as set_attribute, nt as push, o as prop, ot as noop, q as only_child, tt as pop, w as each } from "./api.js";
import { h as resource, i as Skeleton, l as listed, n as Icon, s as ago, u as plural } from "./app.js";
import { t as cursor } from "./cursor.js";
import { t as Failed } from "./Failed.js";
import { t as safeHref } from "./href.js";
import { t as Grid } from "./Grid.js";
import { t as Empty } from "./Empty.js";
import { t as Tabs } from "./Tabs.js";
//#region src/surfaces/github/GithubList.svelte
var act = ($$anchor, kind = noop) => {
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var a = root$1();
		var node_1 = child(a);
		Icon(node_1, {
			name: "person",
			size: 13
		});
		next();
		reset(a);
		append($$anchor, a);
	};
	var consequent_1 = ($$anchor) => {
		var span = root_1$1();
		append($$anchor, span);
	};
	var consequent_2 = ($$anchor) => {
		var span_1 = root_2();
		append($$anchor, span_1);
	};
	var alternate = ($$anchor) => {
		var span_2 = root_3();
		append($$anchor, span_2);
	};
	if_block(node, ($$render) => {
		if (kind() === "sign_in") $$render(consequent);
		else if (kind() === "wait") $$render(consequent_1, 1);
		else if (kind() === "doctor") $$render(consequent_2, 2);
		else $$render(alternate, -1);
	});
	append($$anchor, fragment);
};
function stateSays(g, now = Date.now()) {
	if (!g) return null;
	switch (g.state) {
		case "signed_out": return {
			title: "Not signed in to GitHub",
			body: g.said ? g.said : "Nothing is read from GitHub until you sign in; the token is kept only in this machine's credential store.",
			action: "sign_in",
			blocks: true
		};
		case "pending": return {
			title: "Signing in to GitHub",
			body: `Enter the code ${g.user_code ?? ""} at ${g.verification_uri ?? "GitHub's device page"}.`,
			action: "sign_in",
			blocks: true
		};
		case "expired": return {
			title: "GitHub sign-in expired",
			body: "GitHub no longer accepts the token, so it was removed from this machine.",
			action: "sign_in",
			blocks: true
		};
		case "rate_limited": return {
			title: "GitHub's rate limit is spent",
			body: `It resets at ${clock(g.until)}; Devplane asks again then.`,
			action: "wait",
			blocks: false
		};
		case "unreachable": return {
			title: "GitHub unreachable",
			body: `${g.why ? `${g.why}. ` : ""}${g.since ? `Since ${clock(g.since)}.` : ""}`.trim(),
			action: "doctor",
			blocks: false
		};
		case "not_github": return {
			title: "Not a GitHub repository",
			body: g.why ?? "Its git remote is not on a GitHub host this machine signs in to.",
			action: "remote",
			blocks: true
		};
		default: return null;
	}
}
function clock(at) {
	if (!at) return "a time GitHub did not say";
	const d = new Date(at);
	if (Number.isNaN(d.getTime())) return at;
	return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
var root$1 = from_html(`<a class="act svelte-sff017" href="#setup"><!> Sign in</a>`);
var root_1$1 = from_html(`<span class="quiet svelte-sff017">nothing to do — it resumes on its own</span>`);
var root_2 = from_html(`<span class="quiet svelte-sff017">check the network, then <code class="svelte-sff017">devplane doctor</code></span>`);
var root_3 = from_html(`<span class="quiet svelte-sff017">add a GitHub remote: <code class="svelte-sff017">git remote add origin git@github.com:owner/name.git</code></span>`);
var root_4 = from_html(`<span class="cov svelte-sff017"> </span>`);
var root_5 = from_html(`<div class="state"><!></div>`);
var root_6 = from_html(`<div class="state" data-state="not_github"><!></div>`);
var root_7 = from_html(`<span class="stale svelte-sff017"> </span>`);
var root_8 = from_html(`<p class="banner svelte-sff017"><!> <b> </b> <!> <!></p>`);
var root_9 = from_html(`<p class="quiet svelte-sff017"> </p>`);
var root_10 = from_html(`<p class="banner svelte-sff017" data-state="project-failed"><!> <b> </b> </p>`);
var root_11 = from_html(`<p class="quiet svelte-sff017">Nothing was read before GitHub stopped answering, so there is no list to show.</p>`);
var root_12 = from_html(`<p class="quiet svelte-sff017">Not read yet: Devplane has not asked GitHub since it started. The list appears once it has.</p>`);
var root_13 = from_html(`<span class="you svelte-sff017" title="needs you"><!></span>`);
var root_14 = from_html(`<span class="t svelte-sff017"> </span>`);
var root_15 = from_html(`<span class="dim svelte-sff017"> </span>`);
var root_16 = from_html(`<a target="_blank" rel="noopener noreferrer" class="open svelte-sff017"><!> open</a>`);
var root_17 = from_html(`<p class="quiet more svelte-sff017"> </p>`);
var root_18 = from_html(`<div class="frame svelte-sff017"><!></div> <!>`, 1);
var root_19 = from_html(`<!> <!> <!> <!>`, 1);
var root_20 = from_html(`<div class="page svelte-sff017"><header class="head svelte-sff017"><h1 class="svelte-sff017">Issues and pull requests</h1> <!></header> <!> <!></div>`);
function GithubList($$anchor, $$props) {
	push($$props, true);
	let issues = prop($$props, "issues", 19, () => []), pulls = prop($$props, "pulls", 19, () => []), loaded = prop($$props, "loaded", 3, false), failed = prop($$props, "failed", 3, ""), tab = prop($$props, "tab", 3, "issues"), coverage = prop($$props, "coverage", 3, null), github = prop($$props, "github", 3, null), projects = prop($$props, "projects", 19, () => []), readAt = prop($$props, "readAt", 3, null);
	let showing = state("issues");
	user_effect(() => {
		set(showing, tab());
	});
	const shown = user_derived(() => get(showing) === "issues" ? issues() : pulls());
	const absent = user_derived(() => coverage() ? Math.max(0, coverage().projects - coverage().configured) : 0);
	const says = user_derived(() => stateSays(github()));
	const elsewhere = user_derived(() => projects().filter((p) => p.github?.state === "not_github"));
	const onGithub = user_derived(() => projects().filter((p) => p.github?.state !== "not_github"));
	const allElsewhere = user_derived(() => projects().length > 0 && get(onGithub).length === 0);
	const blocking = user_derived(() => !!get(says)?.blocks);
	const stale = user_derived(() => !!get(says) && !get(says).blocks);
	const more = user_derived(() => projects().reduce((n, p) => n + ((get(showing) === "issues" ? p.issues_more : p.pull_requests_more) ?? 0), 0));
	const age = user_derived(() => readAt() ? ago((Date.now() - new Date(readAt()).getTime()) / 1e3) : "");
	const unread = user_derived(() => projects().filter((p) => p.stale && p.github?.state !== "not_github"));
	const signIn = user_derived(() => /sign(ed)? ?in|auth login|not signed/i.test(failed()));
	const counted = user_derived(() => loaded() && !failed() && !get(blocking) && !get(allElsewhere) && !get(stale));
	let selected = state(null);
	let order = state(proxy([]));
	const openRow = (url) => {
		const to = safeHref(url);
		if (to) window.open(to, "_blank", "noopener");
	};
	user_effect(() => cursor({
		scope: "github",
		size: () => get(order).length,
		at: () => get(order).indexOf(get(selected) ?? ""),
		go: (i) => set(selected, get(order)[i], true),
		open: (i) => openRow(get(order)[i])
	}));
	const columns = [
		{
			key: "you",
			label: "",
			width: 36
		},
		{
			key: "title",
			label: "Title",
			width: 520,
			sort: (r) => r.title
		},
		{
			key: "project",
			label: "Project",
			width: 160,
			sort: (r) => r.project_name
		},
		{
			key: "open",
			label: "",
			align: "end"
		}
	];
	var div = root_20();
	var header = child(div);
	var node_2 = sibling(child(header), 2);
	var consequent_3 = ($$anchor) => {
		var span_3 = root_4();
		var text = only_child(span_3);
		template_effect(($0) => set_text(text, `${coverage().configured ?? ""} ${$0 ?? ""} on GitHub${get(absent) ? ` · ${get(absent)} not, and not seen here` : ""}`), [() => plural(coverage().configured, "project", "projects")]);
		append($$anchor, span_3);
	};
	if_block(node_2, ($$render) => {
		if (coverage() && !get(blocking)) $$render(consequent_3);
	});
	reset(header);
	var node_3 = sibling(header, 2);
	{
		let $0 = user_derived(() => [{
			id: "issues",
			label: "Issues",
			icon: "dot",
			count: get(counted) ? issues().length : null
		}, {
			id: "pulls",
			label: "Pull requests",
			icon: "forge",
			count: get(counted) ? pulls().length : null
		}]);
		Tabs(node_3, {
			get tabs() {
				return get($0);
			},
			label: "what to show",
			get active() {
				return get(showing);
			},
			set active($$value) {
				set(showing, $$value, true);
			}
		});
	}
	var node_4 = sibling(node_3, 2);
	var consequent_4 = ($$anchor) => {
		Skeleton($$anchor, {});
	};
	var consequent_6 = ($$anchor) => {
		{
			const action = ($$anchor) => {
				var fragment_3 = comment();
				var node_5 = first_child(fragment_3);
				var consequent_5 = ($$anchor) => {
					act($$anchor, () => "sign_in");
				};
				var alternate_1 = ($$anchor) => {
					act($$anchor, () => "doctor");
				};
				if_block(node_5, ($$render) => {
					if (get(signIn)) $$render(consequent_5);
					else $$render(alternate_1, -1);
				});
				append($$anchor, fragment_3);
			};
			let $0 = user_derived(() => get(signIn) ? "Not signed in to GitHub" : "The forge could not be read");
			Empty($$anchor, {
				icon: "alert",
				get title() {
					return get($0);
				},
				get body() {
					return failed();
				},
				action,
				$$slots: { action: true }
			});
		}
	};
	var consequent_7 = ($$anchor) => {
		var div_1 = root_5();
		var node_6 = child(div_1);
		{
			const action = ($$anchor) => {
				act($$anchor, () => get(says).action);
			};
			Empty(node_6, {
				icon: "alert",
				get title() {
					return get(says).title;
				},
				get body() {
					return get(says).body;
				},
				action,
				$$slots: { action: true }
			});
		}
		reset(div_1);
		template_effect(() => set_attribute(div_1, "data-state", github()?.state));
		append($$anchor, div_1);
	};
	var consequent_8 = ($$anchor) => {
		var div_2 = root_6();
		var node_7 = child(div_2);
		{
			const action = ($$anchor) => {
				act($$anchor, () => "remote");
			};
			let $0 = user_derived(() => `${listed(get(elsewhere).map((p) => p.project_name))} ${plural(get(elsewhere).length, "has", "have")} no remote on a GitHub host this machine signs in to.`);
			Empty(node_7, {
				icon: "forge",
				title: "Not a GitHub repository",
				get body() {
					return get($0);
				},
				action,
				$$slots: { action: true }
			});
		}
		reset(div_2);
		append($$anchor, div_2);
	};
	var alternate_3 = ($$anchor) => {
		var fragment_8 = root_19();
		var node_8 = first_child(fragment_8);
		var consequent_10 = ($$anchor) => {
			var p_1 = root_8();
			var node_9 = child(p_1);
			Icon(node_9, {
				name: "alert",
				size: 13
			});
			var b = sibling(node_9, 2);
			var text_1 = only_child(b, true);
			var text_2 = sibling(b);
			var node_10 = sibling(text_2);
			var consequent_9 = ($$anchor) => {
				var span_4 = root_7();
				var text_3 = only_child(span_4);
				template_effect(() => set_text(text_3, `stale, read ${get(age) ?? ""} ago`));
				append($$anchor, span_4);
			};
			if_block(node_10, ($$render) => {
				if (get(age)) $$render(consequent_9);
			});
			act(sibling(node_10, 2), () => get(says).action);
			reset(p_1);
			template_effect(() => {
				set_attribute(p_1, "data-state", github()?.state);
				set_text(text_1, get(says).title);
				set_text(text_2, ` — ${get(says).body ?? ""} `);
			});
			append($$anchor, p_1);
		};
		if_block(node_8, ($$render) => {
			if (get(says) && get(stale)) $$render(consequent_10);
		});
		var node_12 = sibling(node_8, 2);
		var consequent_11 = ($$anchor) => {
			var p_2 = root_9();
			var text_4 = only_child(p_2);
			template_effect(($0) => set_text(text_4, `${$0 ?? ""}: not a GitHub repository.`), [() => listed(get(elsewhere).map((p) => p.project_name))]);
			append($$anchor, p_2);
		};
		if_block(node_12, ($$render) => {
			if (get(elsewhere).length) $$render(consequent_11);
		});
		var node_13 = sibling(node_12, 2);
		each(node_13, 17, () => get(unread), (p) => p.project, ($$anchor, p) => {
			var p_3 = root_10();
			var node_14 = child(p_3);
			Icon(node_14, {
				name: "alert",
				size: 13
			});
			var b_1 = sibling(node_14, 2);
			var text_5 = only_child(b_1, true);
			var text_6 = sibling(b_1);
			reset(p_3);
			template_effect(() => {
				set_text(text_5, get(p).project_name);
				set_text(text_6, ` could not be read — ${get(p).stale ?? ""}`);
			});
			append($$anchor, p_3);
		});
		var node_15 = sibling(node_13, 2);
		var consequent_12 = ($$anchor) => {
			var p_4 = root_11();
			append($$anchor, p_4);
		};
		var consequent_13 = ($$anchor) => {
			var p_5 = root_12();
			append($$anchor, p_5);
		};
		var consequent_14 = ($$anchor) => {
			var p_6 = root_9();
			var text_7 = only_child(p_6);
			template_effect(($0) => set_text(text_7, `Nothing open was read, and ${$0 ?? ""} could not be read at all, so this is not every project.`), [() => listed(get(unread).map((p) => p.project_name))]);
			append($$anchor, p_6);
		};
		var consequent_15 = ($$anchor) => {
			{
				let $0 = user_derived(() => get(showing) === "issues" ? "No open issue" : "No open pull request");
				let $1 = user_derived(() => get(absent) ? `${get(absent)} projects have no GitHub remote and are not counted.` : "");
				Empty($$anchor, {
					icon: "forge",
					get title() {
						return get($0);
					},
					body: "Across every registered project that has a GitHub remote.",
					get limit() {
						return get($1);
					}
				});
			}
		};
		var alternate_2 = ($$anchor) => {
			var fragment_10 = root_18();
			var div_3 = first_child(fragment_10);
			var node_16 = child(div_3);
			{
				const cell = ($$anchor, r = noop, c = noop) => {
					var fragment_11 = comment();
					var node_17 = first_child(fragment_11);
					var consequent_17 = ($$anchor) => {
						var fragment_12 = comment();
						var node_18 = first_child(fragment_12);
						var consequent_16 = ($$anchor) => {
							var span_5 = root_13();
							var node_19 = child(span_5);
							Icon(node_19, {
								name: "person",
								size: 13
							});
							reset(span_5);
							append($$anchor, span_5);
						};
						if_block(node_18, ($$render) => {
							if (r().needs_you) $$render(consequent_16);
						});
						append($$anchor, fragment_12);
					};
					var consequent_18 = ($$anchor) => {
						var span_6 = root_14();
						var text_8 = only_child(span_6, true);
						template_effect(() => set_text(text_8, r().title));
						append($$anchor, span_6);
					};
					var consequent_19 = ($$anchor) => {
						var span_7 = root_15();
						var text_9 = only_child(span_7, true);
						template_effect(() => set_text(text_9, r().project_name));
						append($$anchor, span_7);
					};
					var consequent_20 = ($$anchor) => {
						var a_1 = root_16();
						var node_20 = child(a_1);
						Icon(node_20, {
							name: "external",
							size: 13
						});
						next();
						reset(a_1);
						template_effect(($0) => set_attribute(a_1, "href", $0), [() => safeHref(r().url)]);
						append($$anchor, a_1);
					};
					var d_1 = user_derived(() => safeHref(r().url));
					if_block(node_17, ($$render) => {
						if (c().key === "you") $$render(consequent_17);
						else if (c().key === "title") $$render(consequent_18, 1);
						else if (c().key === "project") $$render(consequent_19, 2);
						else if (get(d_1)) $$render(consequent_20, 3);
					});
					append($$anchor, fragment_11);
				};
				Grid(node_16, {
					id: "forge",
					get columns() {
						return columns;
					},
					get rows() {
						return get(shown);
					},
					key: (r) => r.url,
					group: (r) => r.needs_you ? "Needs you" : "Everything else",
					open: (r) => openRow(r.url),
					get label() {
						return get(showing);
					},
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
			reset(div_3);
			var node_21 = sibling(div_3, 2);
			var consequent_21 = ($$anchor) => {
				var p_7 = root_17();
				var text_10 = only_child(p_7);
				template_effect(($0) => set_text(text_10, `${get(more) ?? ""} more open ${$0 ?? ""} on GitHub — only the newest are read each time.`), [() => get(showing) === "issues" ? plural(get(more), "issue", "issues") : plural(get(more), "pull request", "pull requests")]);
				append($$anchor, p_7);
			};
			if_block(node_21, ($$render) => {
				if (get(more) > 0) $$render(consequent_21);
			});
			append($$anchor, fragment_10);
		};
		if_block(node_15, ($$render) => {
			if (get(shown).length === 0 && get(stale)) $$render(consequent_12);
			else if (get(shown).length === 0 && !readAt()) $$render(consequent_13, 1);
			else if (get(shown).length === 0 && get(unread).length) $$render(consequent_14, 2);
			else if (get(shown).length === 0) $$render(consequent_15, 3);
			else $$render(alternate_2, -1);
		});
		append($$anchor, fragment_8);
	};
	if_block(node_4, ($$render) => {
		if (!loaded()) $$render(consequent_4);
		else if (failed()) $$render(consequent_6, 1);
		else if (get(says) && get(blocking)) $$render(consequent_7, 2);
		else if (get(allElsewhere)) $$render(consequent_8, 3);
		else $$render(alternate_3, -1);
	});
	reset(div);
	append($$anchor, div);
	pop();
}
//#endregion
//#region src/surfaces/github/Github.svelte
var root = from_html(`<div class="stale svelte-1n13jp3"><!></div>`);
var root_1 = from_html(`<!> <!>`, 1);
function Github($$anchor, $$props) {
	push($$props, true);
	let coverage = prop($$props, "coverage", 3, null);
	const read = resource(() => "/api/forge", {
		every: 3e4,
		tell: () => "devplane issue list"
	});
	const issues = user_derived(() => read.data?.issues ?? []);
	const pulls = user_derived(() => read.data?.pull_requests ?? []);
	const loaded = user_derived(() => read.phase !== "loading");
	const failed = user_derived(() => read.data?.error ?? (read.phase === "failed" ? read.failure?.down ? "Not read — Devplane is not answering." : read.failure?.says ?? "" : ""));
	var fragment = root_1();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		var div = root();
		var node_1 = child(div);
		Failed(node_1, {
			what: "the forge",
			get failure() {
				return read.failure;
			},
			get at() {
				return read.at;
			},
			stale: true
		});
		reset(div);
		append($$anchor, div);
	};
	if_block(node, ($$render) => {
		if (read.phase === "stale" && read.failure) $$render(consequent);
	});
	var node_2 = sibling(node, 2);
	{
		let $0 = user_derived(() => read.data?.github ?? null);
		let $1 = user_derived(() => read.data?.projects ?? []);
		let $2 = user_derived(() => read.data?.fetched_at ?? null);
		GithubList(node_2, {
			get issues() {
				return get(issues);
			},
			get pulls() {
				return get(pulls);
			},
			get loaded() {
				return get(loaded);
			},
			get failed() {
				return get(failed);
			},
			get coverage() {
				return coverage();
			},
			get github() {
				return get($0);
			},
			get projects() {
				return get($1);
			},
			get readAt() {
				return get($2);
			}
		});
	}
	append($$anchor, fragment);
	pop();
}
//#endregion
export { Github as default };

//# sourceMappingURL=Github.js.map