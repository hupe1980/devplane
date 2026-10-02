import { $ as remove_textarea_child, A as comment, C as each, E as if_block, F as delegated, G as first_child, H as user_effect, I as event, J as proxy, K as only_child, L as get, O as set_text, P as delegate, Q as autofocus, V as template_effect, W as child, X as state, Y as set, Z as user_derived, d as bind_checked, et as pop, f as bind_value, g as init_select, h as bind_select_value, i as api, it as reset, j as from_html, k as append, m as set_attribute, o as prop, p as remove_input_defaults, q as sibling, tt as push, v as set_class, w as index } from "./api.js";
import { h as resource, i as Skeleton, k as run, m as failure, n as Icon, w as go } from "./app.js";
import { t as writer } from "./write.svelte.js";
import { t as Failed } from "./Failed.js";
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
var root_10 = from_html(`<li><!> <b class="svelte-wpxmn"> </b> <span class="svelte-wpxmn"> </span> <!></li>`);
var root_11 = from_html(`<ul class="svelte-wpxmn"></ul>`);
var root_12 = from_html(`<p class="fail svelte-wpxmn"> </p>`);
var root_13 = from_html(`<form class="new svelte-wpxmn" aria-label="start a new change"><header class="svelte-wpxmn"><!> <h1 class="svelte-wpxmn">New change</h1> <button type="button" class="x svelte-wpxmn" aria-label="close"><!></button></header> <div class="body svelte-wpxmn"><section class="svelte-wpxmn"><span class="label svelte-wpxmn">Where <span class="hint svelte-wpxmn">one project, or the same change in several</span></span> <!> <div class="projects svelte-wpxmn" role="group" aria-label="projects"></div></section> <!> <!> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="title">Title <span class="hint svelte-wpxmn">also the branch name</span></label> <input id="title" placeholder="Rate-limit the login route" class="svelte-wpxmn"/></section> <section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="prompt">What to do <span class="hint svelte-wpxmn">the agent's first prompt; the title is used when empty</span></label> <textarea id="prompt" rows="5" placeholder="Five failed logins a minute per account; the sixth is rejected with Retry-After." class="svelte-wpxmn"></textarea></section> <div class="row svelte-wpxmn"><section class="svelte-wpxmn"><label class="label svelte-wpxmn" for="agent">Agent</label> <select id="agent" class="svelte-wpxmn"><option> </option><!></select></section> <section class="svelte-wpxmn"><span class="label svelte-wpxmn">Isolation</span> <label class="check svelte-wpxmn"><input type="checkbox" class="svelte-wpxmn"/> Its own worktree <span class="hint svelte-wpxmn"> </span></label></section></div> <section class="preflight svelte-wpxmn" aria-live="polite"><span class="label svelte-wpxmn">Before anything is created</span> <!></section> <!></div> <footer class="svelte-wpxmn"><span class="quiet svelte-wpxmn"> </span> <button type="button" class="svelte-wpxmn">Cancel</button> <button type="submit" class="primary svelte-wpxmn"><!> </button></footer></form>`);
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
	let title = state("");
	let prompt = state("");
	let agent = state("");
	let spec = state("");
	let worktree = state(true);
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
	var form = root_13();
	var header = child(form);
	var node = child(header);
	Icon(node, {
		name: "change",
		size: 16
	});
	var button = sibling(node, 4);
	var node_1 = child(button);
	Icon(node_1, {
		name: "x",
		size: 14
	});
	reset(button);
	reset(header);
	var div = sibling(header, 2);
	var section = child(div);
	var node_2 = sibling(child(section), 2);
	var consequent = ($$anchor) => {
		var input = root();
		remove_input_defaults(input);
		bind_value(input, () => get(filter), ($$value) => set(filter, $$value));
		append($$anchor, input);
	};
	if_block(node_2, ($$render) => {
		if (get(projects).length > 6) $$render(consequent);
	});
	var div_1 = sibling(node_2, 2);
	each(div_1, 21, () => get(shownProjects), (p) => p.id, ($$anchor, p) => {
		var button_1 = root_2();
		let classes;
		var node_3 = child(button_1);
		{
			let $0 = user_derived(() => get(chosen).includes(get(p).id) ? "check" : "folder");
			Icon(node_3, {
				get name() {
					return get($0);
				},
				size: 13
			});
		}
		var text = sibling(node_3);
		var node_4 = sibling(text);
		var consequent_1 = ($$anchor) => {
			var span = root_1();
			append($$anchor, span);
		};
		if_block(node_4, ($$render) => {
			if (!get(p).trusted) $$render(consequent_1);
		});
		reset(button_1);
		template_effect(($0, $1) => {
			classes = set_class(button_1, 1, "proj svelte-wpxmn", null, classes, { on: $0 });
			set_attribute(button_1, "aria-pressed", $1);
			set_text(text, ` ${get(p).name ?? ""} `);
		}, [() => get(chosen).includes(get(p).id), () => get(chosen).includes(get(p).id)]);
		delegated("click", button_1, () => toggle(get(p).id));
		append($$anchor, button_1);
	}, ($$anchor) => {
		var fragment = comment();
		var node_5 = first_child(fragment);
		var consequent_2 = ($$anchor) => {
			Failed($$anchor, {
				what: "the projects",
				get failure() {
					return projectsRead.failure;
				}
			});
		};
		var consequent_3 = ($$anchor) => {
			Skeleton($$anchor, { rows: 2 });
		};
		var alternate = ($$anchor) => {
			var p_1 = root_3();
			append($$anchor, p_1);
		};
		if_block(node_5, ($$render) => {
			if (projectsRead.phase === "failed" && projectsRead.failure) $$render(consequent_2);
			else if (projectsRead.phase === "loading") $$render(consequent_3, 1);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment);
	});
	reset(div_1);
	reset(section);
	var node_6 = sibling(section, 2);
	var consequent_4 = ($$anchor) => {
		Failed($$anchor, {
			what: "the specifications, so none can be picked",
			get failure() {
				return specsRead.failure;
			}
		});
	};
	if_block(node_6, ($$render) => {
		if (get(one) && specsRead.failure) $$render(consequent_4);
	});
	var node_7 = sibling(node_6, 2);
	var consequent_5 = ($$anchor) => {
		var section_1 = root_5();
		var select = sibling(child(section_1), 2);
		var option = child(select);
		option.value = option.__value = "";
		var node_8 = sibling(option);
		each(node_8, 17, () => get(plans), (p) => p.plan.path, ($$anchor, p) => {
			var option_1 = root_4();
			var text_1 = only_child(option_1);
			var option_1_value = {};
			template_effect(() => {
				option_1.disabled = !!get(p).change_id;
				set_text(text_1, `${get(p).plan.path ?? ""}${get(p).plan.progress ? ` · ${get(p).plan.progress.done} ticked · ${get(p).plan.progress.total} tasks` : ""}${get(p).change_id ? " · a change already works to it" : ""}`);
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
	if_block(node_7, ($$render) => {
		if (get(one) && get(plans).length > 0) $$render(consequent_5);
	});
	var section_2 = sibling(node_7, 2);
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
	var node_9 = sibling(option_2);
	each(node_9, 17, () => get(agents), (a) => a.id, ($$anchor, a) => {
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
	var node_10 = sibling(child(section_6), 2);
	var consequent_6 = ($$anchor) => {
		var p_2 = root_6();
		append($$anchor, p_2);
	};
	var d = user_derived(() => !get(targets) && (get(chosen).length === 0 || !get(title).trim()));
	var consequent_7 = ($$anchor) => {
		var p_3 = root_7();
		var node_11 = child(p_3);
		Icon(node_11, {
			name: "alert",
			size: 13
		});
		var text_5 = sibling(node_11);
		reset(p_3);
		template_effect(() => set_text(text_5, ` ${get(checkError) ?? ""}`));
		append($$anchor, p_3);
	};
	var consequent_8 = ($$anchor) => {
		var p_4 = root_8();
		append($$anchor, p_4);
	};
	var alternate_1 = ($$anchor) => {
		var ul = root_11();
		each(ul, 21, () => get(targets), (t) => t.asked, ($$anchor, t) => {
			var li = root_10();
			let classes_1;
			var node_12 = child(li);
			{
				let $0 = user_derived(() => get(t).refusal ? "x" : "check");
				Icon(node_12, {
					get name() {
						return get($0);
					},
					size: 14
				});
			}
			var b_1 = sibling(node_12, 2);
			var text_6 = only_child(b_1, true);
			var span_2 = sibling(b_1, 2);
			var text_7 = only_child(span_2, true);
			var node_13 = sibling(span_2, 2);
			each(node_13, 17, () => get(t).notes, index, ($$anchor, n) => {
				var span_3 = root_9();
				var text_8 = only_child(span_3, true);
				template_effect(() => set_text(text_8, get(n)));
				append($$anchor, span_3);
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
	if_block(node_10, ($$render) => {
		if (get(d)) $$render(consequent_6);
		else if (get(checkError)) $$render(consequent_7, 1);
		else if (!get(targets)) $$render(consequent_8, 2);
		else $$render(alternate_1, -1);
	});
	reset(section_6);
	var node_14 = sibling(section_6, 2);
	var consequent_9 = ($$anchor) => {
		var p_5 = root_12();
		var text_9 = only_child(p_5, true);
		template_effect(() => set_text(text_9, get(said)));
		append($$anchor, p_5);
	};
	if_block(node_14, ($$render) => {
		if (get(said)) $$render(consequent_9);
	});
	reset(div);
	var footer = sibling(div, 2);
	var span_4 = child(footer);
	var text_10 = only_child(span_4, true);
	var button_2 = sibling(span_4, 2);
	var button_3 = sibling(button_2, 2);
	var node_15 = child(button_3);
	Icon(node_15, {
		name: "play",
		size: 13
	});
	var text_11 = sibling(node_15);
	reset(button_3);
	reset(footer);
	reset(form);
	template_effect(() => {
		set_text(text_2, agentsRead.failure ? "the project's default (the agents could not be read)" : "the project's default");
		set_text(text_4, get(worktree) ? "your checkout is never touched" : "in place — no parallel safety, and the diff is against your working tree");
		set_text(text_10, get(refused).length ? `${get(refused).length} refused — nothing will be created until every project can start` : get(chosen).length > 1 ? `${get(chosen).length} changes, one per project` : "");
		button_3.disabled = !get(ready) || !!starting.busy;
		set_text(text_11, ` ${starting.busy ? `Starting · ${starting.elapsed}` : get(chosen).length > 1 ? `Start ${get(chosen).length} changes` : "Start the change"}`);
	});
	event("submit", form, start);
	delegated("click", button, close);
	bind_value(input_1, () => get(title), ($$value) => set(title, $$value));
	bind_value(textarea, () => get(prompt), ($$value) => set(prompt, $$value));
	bind_select_value(select_1, () => get(agent), ($$value) => set(agent, $$value));
	bind_checked(input_2, () => get(worktree), ($$value) => set(worktree, $$value));
	delegated("click", button_2, close);
	append($$anchor, form);
	pop();
}
delegate(["click"]);
//#endregion
export { New as default, startable };

//# sourceMappingURL=New.js.map