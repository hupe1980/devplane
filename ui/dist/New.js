import { $ as user_effect, B as append, C as remove_input_defaults, E as init_select, F as index, G as delegate, H as from_html, J as get, K as delegated, L as if_block, P as each, Q as template_effect, S as bind_value, T as bind_select_value, V as comment, at as proxy, ct as user_derived, dt as pop, f as run, ft as push, g as prop, ht as reset, i as api, it as sibling, k as set_class, lt as autofocus, nt as first_child, ot as set, q as event, rt as only_child, st as state, tt as child, ut as remove_textarea_child, w as set_attribute, x as bind_checked, z as set_text } from "./api.js";
import { n as failure, r as resource } from "./resource.svelte.js";
import { O as surfaces, i as Skeleton, n as Icon, w as go } from "./app.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
//#region src/surfaces/new/Draft.svelte
var root$1 = from_html(`<option> </option>`);
var root_1$1 = from_html(`<section class="svelte-e5wt26"><label class="label svelte-e5wt26" for="draft-methodology">Methodology <span class="hint svelte-e5wt26">this project has both</span></label> <select id="draft-methodology" class="svelte-e5wt26"><option>choose one</option><option>Spec Kit</option><option>OpenSpec</option></select></section>`);
var root_2$1 = from_html(`<p class="quiet svelte-e5wt26">Choose where and write what it is for, and the exact text sent is shown here.</p>`);
var root_3$1 = from_html(`<p class="quiet svelte-e5wt26">Checking…</p>`);
var root_4$1 = from_html(`<p class="quiet svelte-e5wt26"> </p>`);
var root_5$1 = from_html(`<pre class="sent svelte-e5wt26"> </pre> <!>`, 1);
var root_6$1 = from_html(`<p class="fail svelte-e5wt26"><!> </p>`);
var root_7$1 = from_html(`<p class="fail svelte-e5wt26"> </p>`);
var root_8$1 = from_html(`<form class="new svelte-e5wt26" aria-label="draft a specification"><header class="svelte-e5wt26"><!> <h1 class="svelte-e5wt26">Draft a specification</h1> <button type="button" class="mode svelte-e5wt26">Start from a prompt instead</button> <button type="button" class="x svelte-e5wt26" aria-label="close"><!></button></header> <div class="body svelte-e5wt26"><p class="quiet svelte-e5wt26">Your words go to the agent after the project's own specification command, and nothing else. Devplane writes no word of the
      specification; what the command writes is this change's diff, which you review and then accept or reject.</p> <section class="svelte-e5wt26"><label class="label svelte-e5wt26" for="draft-project">Where</label> <select id="draft-project" class="svelte-e5wt26"><option>choose a project</option><!></select></section> <section class="svelte-e5wt26"><label class="label svelte-e5wt26" for="draft-name">Name <span class="hint svelte-e5wt26">the branch, and the folder Spec Kit writes into</span></label> <input id="draft-name" placeholder="Rate limit" class="svelte-e5wt26"/></section> <section class="svelte-e5wt26"><label class="label svelte-e5wt26" for="draft-intent">What it is for <span class="hint svelte-e5wt26">sent verbatim</span></label> <textarea id="draft-intent" rows="4" placeholder="Limit login attempts per account. Lock the account after five tries in a row." class="svelte-e5wt26"></textarea></section> <div class="row svelte-e5wt26"><section class="svelte-e5wt26"><label class="label svelte-e5wt26" for="draft-agent">Agent</label> <select id="draft-agent" class="svelte-e5wt26"><option>the project's default</option><!></select></section> <!></div> <section class="preflight svelte-e5wt26" aria-live="polite"><span class="label svelte-e5wt26">What the agent will be sent</span> <!></section> <!></div> <footer class="svelte-e5wt26"><span class="quiet svelte-e5wt26"> </span> <button type="button" class="svelte-e5wt26">Cancel</button> <button type="submit" class="primary svelte-e5wt26"><!> </button></footer></form>`);
function Draft($$anchor, $$props) {
	push($$props, true);
	let changeSurface = prop($$props, "changeSurface", 3, "change");
	let project = state("");
	let name = state("");
	let intent = state("");
	let agent = state("");
	let methodology = state("");
	user_effect(() => {
		if (!get(project) && $$props.projects.length === 1) set(project, $$props.projects[0].id, true);
	});
	const body = user_derived(() => ({
		project: get(project),
		name: get(name).trim(),
		intent: get(intent),
		agent: get(agent) || null,
		methodology: get(methodology) || null
	}));
	let preview = state(null);
	let checking = state(false);
	user_effect(() => {
		const b = get(body);
		if (!b.project || !b.intent.trim()) {
			set(preview, null);
			return;
		}
		set(checking, true);
		let live = true;
		const t = setTimeout(() => {
			api("/api/changes/draft", {
				method: "POST",
				body: JSON.stringify({
					...b,
					preview: true
				})
			}).then((r) => {
				if (live) set(preview, r, true);
			}).catch((e) => {
				const said = e instanceof Error ? e.message : String(e);
				const refusal = e.body ?? null;
				if (live) set(preview, refusal ?? { error: said }, true);
			}).finally(() => {
				if (live) set(checking, false);
			});
		}, 350);
		return () => {
			live = false;
			clearTimeout(t);
		};
	});
	const offered = user_derived(() => get(preview)?.offered ?? []);
	const ready = user_derived(() => !!get(preview)?.sent && !get(preview)?.error && !!get(body).name && !get(checking));
	const starting = writer();
	let said = state("");
	async function start(e) {
		e.preventDefault();
		if (!get(ready) || starting.busy) return;
		await starting.run("draft", async () => {
			try {
				const r = await api("/api/changes/draft", {
					method: "POST",
					body: JSON.stringify(get(body))
				});
				if (r.error) set(said, r.error, true);
				if (r.change) go(`#${changeSurface()}/${encodeURIComponent(r.change)}`);
			} catch (err) {
				const f = failure(err, "devplane change list");
				set(said, `Nothing was started: ${f.says}.`);
			}
		});
	}
	var form = root_8$1();
	var header = child(form);
	var node = child(header);
	Icon(node, {
		name: "spec",
		size: 16
	});
	var button = sibling(node, 4);
	var button_1 = sibling(button, 2);
	var node_1 = child(button_1);
	Icon(node_1, {
		name: "x",
		size: 14
	});
	reset(button_1);
	reset(header);
	var div = sibling(header, 2);
	var section = sibling(child(div), 2);
	var select = sibling(child(section), 2);
	var option = child(select);
	option.value = option.__value = "";
	var node_2 = sibling(option);
	each(node_2, 17, () => $$props.projects, (p) => p.id, ($$anchor, p) => {
		var option_1 = root$1();
		var text = only_child(option_1);
		var option_1_value = {};
		template_effect(() => {
			option_1.disabled = !get(p).trusted;
			set_text(text, `${get(p).name ?? ""}${get(p).trusted ? "" : " · not trusted"}`);
			if (option_1_value !== (option_1_value = get(p).id)) option_1.value = (option_1.__value = option_1_value) ?? "";
		});
		append($$anchor, option_1);
	});
	reset(select);
	init_select(select);
	reset(section);
	var section_1 = sibling(section, 2);
	var input = sibling(child(section_1), 2);
	remove_input_defaults(input);
	reset(section_1);
	var section_2 = sibling(section_1, 2);
	var textarea = sibling(child(section_2), 2);
	remove_textarea_child(textarea);
	reset(section_2);
	var div_1 = sibling(section_2, 2);
	var section_3 = child(div_1);
	var select_1 = sibling(child(section_3), 2);
	var option_2 = child(select_1);
	option_2.value = option_2.__value = "";
	var node_3 = sibling(option_2);
	each(node_3, 17, () => $$props.agents, (a) => a.id, ($$anchor, a) => {
		var option_3 = root$1();
		var text_1 = only_child(option_3, true);
		var option_3_value = {};
		template_effect(() => {
			set_text(text_1, get(a).name ?? get(a).id);
			if (option_3_value !== (option_3_value = get(a).id)) option_3.value = (option_3.__value = option_3_value) ?? "";
		});
		append($$anchor, option_3);
	});
	reset(select_1);
	init_select(select_1);
	reset(section_3);
	var node_4 = sibling(section_3, 2);
	var consequent = ($$anchor) => {
		var section_4 = root_1$1();
		var select_2 = sibling(child(section_4), 2);
		var option_4 = child(select_2);
		option_4.value = option_4.__value = "";
		var option_5 = sibling(option_4);
		option_5.value = option_5.__value = "speckit";
		var option_6 = sibling(option_5);
		option_6.value = option_6.__value = "openspec";
		reset(select_2);
		init_select(select_2);
		reset(section_4);
		bind_select_value(select_2, () => get(methodology), ($$value) => set(methodology, $$value));
		append($$anchor, section_4);
	};
	if_block(node_4, ($$render) => {
		if (get(offered).length > 1) $$render(consequent);
	});
	reset(div_1);
	var section_5 = sibling(div_1, 2);
	var node_5 = sibling(child(section_5), 2);
	var consequent_1 = ($$anchor) => {
		var p_1 = root_2$1();
		append($$anchor, p_1);
	};
	var d = user_derived(() => !get(project) || !get(intent).trim());
	var consequent_2 = ($$anchor) => {
		var p_2 = root_3$1();
		append($$anchor, p_2);
	};
	var consequent_4 = ($$anchor) => {
		var fragment = root_5$1();
		var pre = first_child(fragment);
		var text_2 = only_child(pre, true);
		var node_6 = sibling(pre, 2);
		var consequent_3 = ($$anchor) => {
			var p_3 = root_4$1();
			var text_3 = only_child(p_3, true);
			template_effect(() => set_text(text_3, get(preview).folder_says));
			append($$anchor, p_3);
		};
		if_block(node_6, ($$render) => {
			if (get(preview).folder_says) $$render(consequent_3);
		});
		template_effect(() => set_text(text_2, get(preview).sent));
		append($$anchor, fragment);
	};
	var consequent_5 = ($$anchor) => {
		var p_4 = root_6$1();
		var node_7 = child(p_4);
		Icon(node_7, {
			name: "x",
			size: 13
		});
		var text_4 = sibling(node_7);
		reset(p_4);
		template_effect(() => set_text(text_4, ` ${get(preview).says ?? get(preview).error ?? ""}`));
		append($$anchor, p_4);
	};
	if_block(node_5, ($$render) => {
		if (get(d)) $$render(consequent_1);
		else if (get(checking) && !get(preview)) $$render(consequent_2, 1);
		else if (get(preview)?.sent) $$render(consequent_4, 2);
		else if (get(preview)) $$render(consequent_5, 3);
	});
	reset(section_5);
	var node_8 = sibling(section_5, 2);
	var consequent_6 = ($$anchor) => {
		var p_5 = root_7$1();
		var text_5 = only_child(p_5, true);
		template_effect(() => set_text(text_5, get(said)));
		append($$anchor, p_5);
	};
	if_block(node_8, ($$render) => {
		if (get(said)) $$render(consequent_6);
	});
	reset(div);
	var footer = sibling(div, 2);
	var span = child(footer);
	var text_6 = only_child(span, true);
	var button_2 = sibling(span, 2);
	var button_3 = sibling(button_2, 2);
	var node_9 = child(button_3);
	Icon(node_9, {
		name: "play",
		size: 13
	});
	var text_7 = sibling(node_9);
	reset(button_3);
	reset(footer);
	reset(form);
	template_effect(() => {
		set_text(text_6, get(body).name ? "" : "Name the change to draft it");
		button_3.disabled = !get(ready) || !!starting.busy;
		set_text(text_7, ` ${starting.busy ? `Starting · ${starting.elapsed}` : "Draft it"}`);
	});
	event("submit", form, start);
	delegated("click", button, function(...$$args) {
		$$props.implement?.apply(this, $$args);
	});
	delegated("click", button_1, function(...$$args) {
		$$props.close?.apply(this, $$args);
	});
	bind_select_value(select, () => get(project), ($$value) => set(project, $$value));
	bind_value(input, () => get(name), ($$value) => set(name, $$value));
	bind_value(textarea, () => get(intent), ($$value) => set(intent, $$value));
	bind_select_value(select_1, () => get(agent), ($$value) => set(agent, $$value));
	delegated("click", button_2, function(...$$args) {
		$$props.close?.apply(this, $$args);
	});
	append($$anchor, form);
	pop();
}
delegate(["click"]);
//#endregion
//#region src/surfaces/new/New.svelte
function startable(targets, checking, failed) {
	return !!targets && targets.length > 0 && targets.every((t) => !t.refusal) && !checking && !failed;
}
var root = from_html(`<input class="filter svelte-wpxmn" placeholder="Filter projects" aria-label="filter projects"/>`);
var root_1 = from_html(`<span class="untrusted svelte-wpxmn">not trusted</span>`);
var root_2 = from_html(`<button type="button"><!> <!></button>`);
var root_3 = from_html(`<p class="quiet svelte-wpxmn">No project is registered yet. <code class="svelte-wpxmn">devplane project trust &lt;path&gt;</code> adds one.</p>`);
var root_4 = from_html(`<option> </option>`);
var root_5 = from_html(`<section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="spec">Specification <span class="hint svelte-wpxmn">optional — the change works to it, and its tasks are traced</span></label> <select id="spec" class="svelte-wpxmn"><option>none — just the words below</option><!></select></section>`);
var root_6 = from_html(`<p class="quiet svelte-wpxmn">Pick where and give it a title, and what starting it would do is shown here.</p>`);
var root_7 = from_html(`<p class="fail svelte-wpxmn"><!> </p>`);
var root_8 = from_html(`<p class="quiet svelte-wpxmn">Checking…</p>`);
var root_9 = from_html(`<span class="note svelte-wpxmn"> </span>`);
var root_10 = from_html(`· <a>Declare a check</a>`, 1);
var root_11 = from_html(`<span class="note svelte-wpxmn"> <!></span>`);
var root_12 = from_html(`<li><!> <b class="svelte-wpxmn"> </b> <span class="svelte-wpxmn"> </span> <!> <!></li>`);
var root_13 = from_html(`<ul class="svelte-wpxmn"></ul>`);
var root_14 = from_html(`<p class="fail svelte-wpxmn"> </p>`);
var root_15 = from_html(`<form class="new svelte-wpxmn" aria-label="start a new change"><header class="svelte-wpxmn"><!> <h1 class="svelte-wpxmn">New change</h1> <button type="button" class="mode svelte-wpxmn">Draft a specification instead</button> <button type="button" class="x svelte-wpxmn" aria-label="close"><!></button></header> <div class="body svelte-wpxmn"><section class="svelte-wpxmn"><span class="label svelte-wpxmn">Where <span class="hint svelte-wpxmn">one project, or the same change in several</span></span> <!> <div class="projects svelte-wpxmn" role="group" aria-label="projects"></div></section> <!> <!> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="title">Title <span class="hint svelte-wpxmn">also the branch name</span></label> <input id="title" placeholder="Rate-limit the login route" class="svelte-wpxmn"/></section> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="prompt">What to do <span class="hint svelte-wpxmn">the agent's first prompt; the title is used when empty</span></label> <textarea id="prompt" rows="5" placeholder="Five failed logins a minute per account; the sixth is rejected with Retry-After." class="svelte-wpxmn"></textarea></section> <div class="row svelte-wpxmn"><section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="agent">Agent</label> <select id="agent" class="svelte-wpxmn"><option> </option><!></select></section> <section class="svelte-wpxmn"><span class="label svelte-wpxmn">Isolation</span> <label class="check svelte-wpxmn"><input type="checkbox" class="svelte-wpxmn"/> Its own worktree <span class="hint svelte-wpxmn"> </span></label></section></div> <section class="preflight svelte-wpxmn" aria-live="polite"><span class="label svelte-wpxmn">Before anything is created</span> <!></section> <!></div> <footer class="svelte-wpxmn"><span class="quiet svelte-wpxmn"> </span> <button type="button" class="svelte-wpxmn">Cancel</button> <button type="submit" class="primary svelte-wpxmn"><!> </button></footer></form>`);
function New($$anchor, $$props) {
	push($$props, true);
	let changeSurface = prop($$props, "changeSurface", 3, "change"), planted = prop($$props, "targets", 3, null);
	const projectsRead = resource(() => "/api/projects", { tell: () => "devplane doctor" });
	const agentsRead = resource(() => "/api/agents", { tell: () => "devplane agent list" });
	const specsRead = resource(() => "/api/specs", { tell: () => "devplane doctor" });
	const projects = user_derived(() => Array.isArray(projectsRead.data) ? projectsRead.data : []);
	const agents = user_derived(() => Array.isArray(agentsRead.data) ? agentsRead.data : []);
	const specs = user_derived(() => specsRead.data?.projects ?? []);
	let chosen = state(proxy([]));
	let prechosen = false;
	user_effect(() => {
		if (prechosen || get(projects).length === 0) return;
		prechosen = true;
		let hint = "";
		try {
			hint = sessionStorage.getItem("vp-new-project") ?? "";
			sessionStorage.removeItem("vp-new-project");
		} catch {}
		const trusted = get(projects).filter((p) => p.trusted);
		const pick = get(projects).find((p) => p.id === hint) ?? (trusted.length === 1 && get(projects).length === 1 ? trusted[0] : null);
		if (pick && get(chosen).length === 0) set(chosen, [pick.id], true);
	});
	let title = state("");
	let prompt = state("");
	let agent = state("");
	let spec = state("");
	let worktree = state(true);
	let drafting = state(false);
	let filter = state("");
	const shownProjects = user_derived(() => get(projects).filter((p) => !get(filter).trim() || p.name.toLowerCase().includes(get(filter).trim().toLowerCase())));
	const one = user_derived(() => get(chosen).length === 1 ? get(projects).find((p) => p.id === get(chosen)[0]) : null);
	const plans = user_derived(() => get(one) ? get(specs).find((s) => s.root === get(one).root || s.project_id === get(one).id)?.plans ?? [] : []);
	function toggle(id) {
		set(chosen, get(chosen).includes(id) ? get(chosen).filter((x) => x !== id) : [...get(chosen), id], true);
		if (get(chosen).length !== 1) set(spec, "");
	}
	const body = user_derived(() => ({
		projects: get(chosen),
		title: get(title).trim(),
		prompt: get(prompt).trim() || null,
		agent: get(agent) || null,
		spec: get(spec) || null,
		worktree: get(worktree)
	}));
	let fetchedTargets = state(null);
	const targets = user_derived(() => get(fetchedTargets) ?? planted());
	let checking = state(false);
	let checkError = state("");
	user_effect(() => {
		const b = get(body);
		if (b.projects.length === 0 || !b.title) {
			set(fetchedTargets, null);
			set(checkError, "");
			return;
		}
		set(checking, true);
		let live = true;
		const t = setTimeout(() => {
			api("/api/changes/preflight", {
				method: "POST",
				body: JSON.stringify(b)
			}).then((r) => {
				if (live) {
					set(fetchedTargets, r.targets ?? [], true);
					set(checkError, "");
				}
			}).catch((e) => {
				if (live) set(checkError, e instanceof Error ? e.message : String(e), true);
			}).finally(() => {
				if (live) set(checking, false);
			});
		}, 350);
		return () => {
			live = false;
			clearTimeout(t);
		};
	});
	const refused = user_derived(() => (get(targets) ?? []).filter((t) => t.refusal));
	const ready = user_derived(() => startable(get(targets), get(checking), get(checkError)));
	const starting = writer();
	let said = state("");
	async function start(e) {
		e.preventDefault();
		if (!get(ready) || starting.busy) return;
		await starting.run("start", async () => {
			try {
				const r = await api("/api/changes", {
					method: "POST",
					body: JSON.stringify(get(body))
				});
				const first = r.change_id ?? r.changes?.[0]?.change_id;
				if (r.error) set(said, r.error, true);
				if (first) go(`#${changeSurface()}/${encodeURIComponent(first)}`);
			} catch (err) {
				const f = failure(err, "devplane change list");
				set(said, `Nothing was started: ${f.says}. \`${f.tell}\` tells more.`);
			}
		});
	}
	const close = () => run("leave", "new");
	const gatesAt = surfaces().find((s) => s.holds === "gates")?.id ?? "";
	var fragment = comment();
	var node = first_child(fragment);
	var consequent = ($$anchor) => {
		Draft($$anchor, {
			get projects() {
				return get(projects);
			},
			get agents() {
				return get(agents);
			},
			get changeSurface() {
				return changeSurface();
			},
			implement: () => set(drafting, false),
			close
		});
	};
	var alternate_2 = ($$anchor) => {
		var form = root_15();
		var header = child(form);
		var node_1 = child(header);
		Icon(node_1, {
			name: "change",
			size: 16
		});
		var button = sibling(node_1, 4);
		var button_1 = sibling(button, 2);
		var node_2 = child(button_1);
		Icon(node_2, {
			name: "x",
			size: 14
		});
		reset(button_1);
		reset(header);
		var div = sibling(header, 2);
		var section = child(div);
		var node_3 = sibling(child(section), 2);
		var consequent_1 = ($$anchor) => {
			var input = root();
			remove_input_defaults(input);
			bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
			append($$anchor, input);
		};
		if_block(node_3, ($$render) => {
			if (get(projects).length > 6) $$render(consequent_1);
		});
		var div_1 = sibling(node_3, 2);
		each(div_1, 21, () => get(shownProjects), (p) => p.id, ($$anchor, p) => {
			var button_2 = root_2();
			let classes;
			var node_4 = child(button_2);
			{
				let $0 = user_derived(() => get(chosen).includes(get(p).id) ? "check" : "folder");
				Icon(node_4, {
					get name() {
						return get($0);
					},
					size: 13
				});
			}
			var text = sibling(node_4);
			var node_5 = sibling(text);
			var consequent_2 = ($$anchor) => {
				var span = root_1();
				append($$anchor, span);
			};
			if_block(node_5, ($$render) => {
				if (!get(p).trusted) $$render(consequent_2);
			});
			reset(button_2);
			template_effect(($0, $1) => {
				classes = set_class(button_2, 1, "proj svelte-wpxmn", null, classes, { on: $0 });
				set_attribute(button_2, "aria-pressed", $1);
				set_text(text, ` ${get(p).name ?? ""} `);
			}, [() => get(chosen).includes(get(p).id), () => get(chosen).includes(get(p).id)]);
			delegated("click", button_2, () => toggle(get(p).id));
			append($$anchor, button_2);
		}, ($$anchor) => {
			var fragment_2 = comment();
			var node_6 = first_child(fragment_2);
			var consequent_3 = ($$anchor) => {
				Failed($$anchor, {
					what: "the projects",
					get failure() {
						return projectsRead.failure;
					}
				});
			};
			var consequent_4 = ($$anchor) => {
				Skeleton($$anchor, { rows: 2 });
			};
			var alternate = ($$anchor) => {
				var p_1 = root_3();
				append($$anchor, p_1);
			};
			if_block(node_6, ($$render) => {
				if (projectsRead.phase === "failed" && projectsRead.failure) $$render(consequent_3);
				else if (projectsRead.phase === "loading") $$render(consequent_4, 1);
				else $$render(alternate, -1);
			});
			append($$anchor, fragment_2);
		});
		reset(div_1);
		reset(section);
		var node_7 = sibling(section, 2);
		var consequent_5 = ($$anchor) => {
			Failed($$anchor, {
				what: "the specifications, so none can be picked",
				get failure() {
					return specsRead.failure;
				}
			});
		};
		if_block(node_7, ($$render) => {
			if (get(one) && specsRead.failure) $$render(consequent_5);
		});
		var node_8 = sibling(node_7, 2);
		var consequent_6 = ($$anchor) => {
			var section_1 = root_5();
			var select = sibling(child(section_1), 2);
			var option = child(select);
			option.value = option.__value = "";
			var node_9 = sibling(option);
			each(node_9, 17, () => get(plans), (p) => p.plan.path, ($$anchor, p) => {
				var option_1 = root_4();
				var text_1 = only_child(option_1);
				var option_1_value = {};
				template_effect(() => {
					option_1.disabled = !!get(p).change_id;
					set_text(text_1, `${get(p).plan.path ?? ""}${get(p).plan.progress ? ` · ${get(p).plan.progress.done} ticked · ${get(p).plan.progress.total} tasks` : get(p).plan.no_progress ? " · no task file, no progress claimed" : ""}${get(p).change_id ? " · a change already works to it" : ""}`);
					if (option_1_value !== (option_1_value = get(p).plan.path)) option_1.value = (option_1.__value = option_1_value) ?? "";
				});
				append($$anchor, option_1);
			});
			reset(select);
			init_select(select);
			reset(section_1);
			bind_select_value(select, () => get(spec), ($$value) => set(spec, $$value));
			append($$anchor, section_1);
		};
		if_block(node_8, ($$render) => {
			if (get(one) && get(plans).length > 0) $$render(consequent_6);
		});
		var section_2 = sibling(node_8, 2);
		var input_1 = sibling(child(section_2), 2);
		remove_input_defaults(input_1);
		autofocus(input_1, true);
		reset(section_2);
		var section_3 = sibling(section_2, 2);
		var textarea = sibling(child(section_3), 2);
		remove_textarea_child(textarea);
		reset(section_3);
		var div_2 = sibling(section_3, 2);
		var section_4 = child(div_2);
		var select_1 = sibling(child(section_4), 2);
		var option_2 = child(select_1);
		var text_2 = only_child(option_2, true);
		option_2.value = option_2.__value = "";
		var node_10 = sibling(option_2);
		each(node_10, 17, () => get(agents), (a) => a.id, ($$anchor, a) => {
			var option_3 = root_4();
			var text_3 = only_child(option_3, true);
			var option_3_value = {};
			template_effect(() => {
				set_text(text_3, get(a).name ?? get(a).id);
				if (option_3_value !== (option_3_value = get(a).id)) option_3.value = (option_3.__value = option_3_value) ?? "";
			});
			append($$anchor, option_3);
		});
		reset(select_1);
		init_select(select_1);
		reset(section_4);
		var section_5 = sibling(section_4, 2);
		var label = sibling(child(section_5), 2);
		var input_2 = child(label);
		remove_input_defaults(input_2);
		var span_1 = sibling(input_2, 2);
		var text_4 = only_child(span_1, true);
		reset(label);
		reset(section_5);
		reset(div_2);
		var section_6 = sibling(div_2, 2);
		var node_11 = sibling(child(section_6), 2);
		var consequent_7 = ($$anchor) => {
			var p_2 = root_6();
			append($$anchor, p_2);
		};
		var d = user_derived(() => !get(targets) && (get(chosen).length === 0 || !get(title).trim()));
		var consequent_8 = ($$anchor) => {
			var p_3 = root_7();
			var node_12 = child(p_3);
			Icon(node_12, {
				name: "alert",
				size: 13
			});
			var text_5 = sibling(node_12);
			reset(p_3);
			template_effect(() => set_text(text_5, ` ${get(checkError) ?? ""}`));
			append($$anchor, p_3);
		};
		var consequent_9 = ($$anchor) => {
			var p_4 = root_8();
			append($$anchor, p_4);
		};
		var alternate_1 = ($$anchor) => {
			var ul = root_13();
			each(ul, 21, () => get(targets), (t) => t.asked, ($$anchor, t) => {
				var li = root_12();
				let classes_1;
				var node_13 = child(li);
				{
					let $0 = user_derived(() => get(t).refusal ? "x" : "check");
					Icon(node_13, {
						get name() {
							return get($0);
						},
						size: 14
					});
				}
				var b_1 = sibling(node_13, 2);
				var text_6 = only_child(b_1, true);
				var span_2 = sibling(b_1, 2);
				var text_7 = only_child(span_2, true);
				var node_14 = sibling(span_2, 2);
				each(node_14, 17, () => get(t).notes, index, ($$anchor, n) => {
					var span_3 = root_9();
					var text_8 = only_child(span_3, true);
					template_effect(() => set_text(text_8, get(n)));
					append($$anchor, span_3);
				});
				var node_15 = sibling(node_14, 2);
				var consequent_11 = ($$anchor) => {
					var span_4 = root_11();
					var text_9 = child(span_4, true);
					var node_16 = sibling(text_9);
					var consequent_10 = ($$anchor) => {
						var fragment_6 = root_10();
						var a_1 = sibling(first_child(fragment_6));
						template_effect(($0) => set_attribute(a_1, "href", $0), [() => `#${gatesAt}/declare/${encodeURIComponent(get(projects).find((p) => p.root === get(t).root)?.id ?? get(t).root)}`]);
						append($$anchor, fragment_6);
					};
					if_block(node_16, ($$render) => {
						if (get(t).root && gatesAt) $$render(consequent_10);
					});
					reset(span_4);
					template_effect(() => set_text(text_9, get(t).no_check));
					append($$anchor, span_4);
				};
				if_block(node_15, ($$render) => {
					if (get(t).no_check) $$render(consequent_11);
				});
				reset(li);
				template_effect(() => {
					classes_1 = set_class(li, 1, "svelte-wpxmn", null, classes_1, { bad: !!get(t).refusal });
					set_text(text_6, get(t).name);
					set_text(text_7, get(t).says);
				});
				append($$anchor, li);
			});
			reset(ul);
			append($$anchor, ul);
		};
		if_block(node_11, ($$render) => {
			if (get(d)) $$render(consequent_7);
			else if (get(checkError)) $$render(consequent_8, 1);
			else if (!get(targets)) $$render(consequent_9, 2);
			else $$render(alternate_1, -1);
		});
		reset(section_6);
		var node_17 = sibling(section_6, 2);
		var consequent_12 = ($$anchor) => {
			var p_5 = root_14();
			var text_10 = only_child(p_5, true);
			template_effect(() => set_text(text_10, get(said)));
			append($$anchor, p_5);
		};
		if_block(node_17, ($$render) => {
			if (get(said)) $$render(consequent_12);
		});
		reset(div);
		var footer = sibling(div, 2);
		var span_5 = child(footer);
		var text_11 = only_child(span_5, true);
		var button_3 = sibling(span_5, 2);
		var button_4 = sibling(button_3, 2);
		var node_18 = child(button_4);
		Icon(node_18, {
			name: "play",
			size: 13
		});
		var text_12 = sibling(node_18);
		reset(button_4);
		reset(footer);
		reset(form);
		template_effect(() => {
			set_text(text_2, agentsRead.failure ? "the project's default (the agents could not be read)" : "the project's default");
			set_text(text_4, get(worktree) ? "your checkout is never touched" : "in place — no parallel safety, and the diff is against your working tree");
			set_text(text_11, get(refused).length ? `${get(refused).length} refused — nothing will be created until every project can start` : get(chosen).length > 1 ? `${get(chosen).length} changes, one per project` : "");
			button_4.disabled = !get(ready) || !!starting.busy;
			set_text(text_12, ` ${starting.busy ? `Starting · ${starting.elapsed}` : get(chosen).length > 1 ? `Start ${get(chosen).length} changes` : "Start the change"}`);
		});
		event("submit", form, start);
		delegated("click", button, () => set(drafting, true));
		delegated("click", button_1, close);
		bind_value(input_1, () => get(title), ($$value) => set(title, $$value));
		bind_value(textarea, () => get(prompt), ($$value) => set(prompt, $$value));
		bind_select_value(select_1, () => get(agent), ($$value) => set(agent, $$value));
		bind_checked(input_2, () => get(worktree), ($$value) => set(worktree, $$value));
		delegated("click", button_3, close);
		append($$anchor, form);
	};
	if_block(node, ($$render) => {
		if (get(drafting)) $$render(consequent);
		else $$render(alternate_2, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
export { New as default, startable };

//# sourceMappingURL=New.js.map