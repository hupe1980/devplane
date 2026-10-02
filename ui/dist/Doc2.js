import { $ as user_effect, B as append, F as index, G as delegate, H as from_html, J as get, K as delegated, L as if_block, O as set_style, P as each, Q as template_effect, V as comment, _t as to_array, ct as user_derived, dt as pop, f as run, ft as push, g as prop, ht as reset, it as sibling, k as set_class, mt as next, nt as first_child, rt as only_child, tt as child, w as set_attribute, z as set_text } from "./api.js";
import { a as Inline, i as Skeleton, n as Icon } from "./app.js";
import { t as Empty } from "./Empty.js";
import { t as ChangeState } from "./ChangeState.js";
import { n as specs, r as watch, t as key } from "./store.svelte.js";
//#region src/surfaces/plan/Doc.svelte
var root = from_html(`<div class="pad svelte-khab3i"><!></div>`);
var root_1 = from_html(`<span class="svelte-khab3i"><b> </b> boxes · <b> </b> ticked</span>`);
var root_2 = from_html(`<span class="wait svelte-khab3i"><!> </span>`);
var root_3 = from_html(`<span class="wait svelte-khab3i"><!> changed under a run</span>`);
var root_4 = from_html(`<span class="quiet svelte-khab3i"><!></span>`);
var root_5 = from_html(`<a class="btn svelte-khab3i"><!> </a> <!> <!>`, 1);
var root_6 = from_html(`<button class="btn primary svelte-khab3i"><!> Start a change…</button> <span class="quiet svelte-khab3i">No change is working to this specification; pick it in the form.</span>`, 1);
var root_7 = from_html(`<p class="warn svelte-khab3i"><!> Its change is finished while boxes are still open.</p>`);
var root_8 = from_html(`<li> </li>`);
var root_9 = from_html(`<section class="card wait svelte-khab3i"><h2 class="svelte-khab3i"><!> Waiting on a person</h2> <ul class="svelte-khab3i"></ul></section>`);
var root_10 = from_html(`<p class="quiet svelte-khab3i">Not traced.</p>`);
var root_11 = from_html(`<p class="quiet svelte-khab3i">No requirement notation was recognised in this folder. A project declares its own with <code class="svelte-khab3i">[spec] tokens</code> in devplane.toml; nothing is guessed.</p>`);
var root_12 = from_html(`<tr><td class="svelte-khab3i"><code class="svelte-khab3i"> </code></td><td class="svelte-khab3i"> </td><td class="svelte-khab3i"> </td></tr>`);
var root_13 = from_html(`<p class="warn svelte-khab3i"> </p>`);
var root_14 = from_html(`<p class="quiet svelte-khab3i"> </p>`);
var root_15 = from_html(`<table class="svelte-khab3i"><thead><tr><th class="svelte-khab3i">requirement</th><th class="svelte-khab3i">tasks</th><th class="svelte-khab3i">ticked</th></tr></thead><tbody></tbody></table> <!> <!>`, 1);
var root_16 = from_html(`<article class="doc svelte-khab3i"><header class="svelte-khab3i"><p class="where svelte-khab3i"><!> <span>/</span> <code class="svelte-khab3i"> </code></p> <h1 class="svelte-khab3i"> </h1> <div class="facts svelte-khab3i"><!> <span class="svelte-khab3i"> </span> <!> <!></div> <div class="acts svelte-khab3i"><!></div> <!></header> <div class="cols svelte-khab3i"><nav class="outline svelte-khab3i" aria-label="outline"><h2 class="svelte-khab3i">Outline</h2> <ol class="svelte-khab3i"></ol></nav> <div class="main svelte-khab3i"><!> <section class="card svelte-khab3i"><h2 class="svelte-khab3i">Requirements and the tasks that cite them</h2> <!></section></div></div></article>`);
function Doc($$anchor, $$props) {
	push($$props, true);
	let focus = prop($$props, "focus", 3, "");
	user_effect(watch);
	const found = user_derived(() => {
		for (const p of specs.projects ?? []) for (const r of p.plans) if (key(p, r) === focus()) return {
			p,
			r
		};
		return null;
	});
	const total = user_derived(() => (specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
	var fragment = comment();
	var node = first_child(fragment);
	var consequent_1 = ($$anchor) => {
		var fragment_1 = comment();
		var node_1 = first_child(fragment_1);
		var consequent = ($$anchor) => {
			var div = root();
			var node_2 = child(div);
			{
				let $0 = user_derived(() => !!specs.error);
				Skeleton(node_2, {
					rows: 4,
					get failed() {
						return get($0);
					}
				});
			}
			reset(div);
			append($$anchor, div);
		};
		var alternate = ($$anchor) => {
			{
				let $0 = user_derived(() => get(total) ? "Pick a specification" : "No specification on this machine");
				let $1 = user_derived(() => get(total) ? `${get(total)} specifications across ${specs.projects.length} projects. A specification is read as it is on disk: its outline, its boxes, and the requirements its tasks cite.` : "Devplane reads Spec Kit, OpenSpec and Kiro folders where they already are, and a plain folder a project names in devplane.toml.");
				Empty($$anchor, {
					icon: "spec",
					get title() {
						return get($0);
					},
					get body() {
						return get($1);
					}
				});
			}
		};
		if_block(node_1, ($$render) => {
			if (specs.projects === null) $$render(consequent);
			else $$render(alternate, -1);
		});
		append($$anchor, fragment_1);
	};
	var alternate_3 = ($$anchor) => {
		const r = user_derived(() => get(found).r);
		var article = root_16();
		var header = child(article);
		var p_1 = child(header);
		var node_3 = child(p_1);
		Icon(node_3, {
			name: "folder",
			size: 12
		});
		var text = sibling(node_3);
		var code = sibling(text, 3);
		var text_1 = only_child(code, true);
		reset(p_1);
		var h1 = sibling(p_1, 2);
		var text_2 = only_child(h1, true);
		var div_1 = sibling(h1, 2);
		var node_4 = child(div_1);
		var consequent_2 = ($$anchor) => {
			var span = root_1();
			var b = child(span);
			var text_3 = only_child(b, true);
			var b_1 = sibling(b, 2);
			var text_4 = only_child(b_1, true);
			next();
			reset(span);
			template_effect(() => {
				set_text(text_3, get(r).plan.progress.total);
				set_text(text_4, get(r).plan.progress.done);
			});
			append($$anchor, span);
		};
		if_block(node_4, ($$render) => {
			if (get(r).plan.progress) $$render(consequent_2);
		});
		var span_1 = sibling(node_4, 2);
		var text_5 = only_child(span_1);
		var node_5 = sibling(span_1, 2);
		var consequent_3 = ($$anchor) => {
			var span_2 = root_2();
			var node_6 = child(span_2);
			Icon(node_6, {
				name: "question",
				size: 13
			});
			var text_6 = sibling(node_6);
			reset(span_2);
			template_effect(() => set_text(text_6, ` ${get(r).plan.open_questions ?? ""} open questions`));
			append($$anchor, span_2);
		};
		if_block(node_5, ($$render) => {
			if (get(r).plan.open_questions > 0) $$render(consequent_3);
		});
		var node_7 = sibling(node_5, 2);
		var consequent_4 = ($$anchor) => {
			var span_3 = root_3();
			var node_8 = child(span_3);
			Icon(node_8, {
				name: "alert",
				size: 13
			});
			next();
			reset(span_3);
			append($$anchor, span_3);
		};
		if_block(node_7, ($$render) => {
			if (get(r).drifted) $$render(consequent_4);
		});
		reset(div_1);
		var div_2 = sibling(div_1, 2);
		var node_9 = child(div_2);
		var consequent_7 = ($$anchor) => {
			var fragment_3 = root_5();
			var a = first_child(fragment_3);
			var node_10 = child(a);
			Icon(node_10, {
				name: "change",
				size: 14
			});
			var text_7 = sibling(node_10);
			reset(a);
			var node_11 = sibling(a, 2);
			var consequent_5 = ($$anchor) => {
				ChangeState($$anchor, {
					get state() {
						return get(r).state;
					},
					get q() {
						return get(r).qualifier;
					}
				});
			};
			if_block(node_11, ($$render) => {
				if (get(r).state) $$render(consequent_5);
			});
			var node_12 = sibling(node_11, 2);
			var consequent_6 = ($$anchor) => {
				var span_4 = root_4();
				var node_13 = child(span_4);
				Inline(node_13, { get text() {
					return get(r).counts_says;
				} });
				reset(span_4);
				append($$anchor, span_4);
			};
			if_block(node_12, ($$render) => {
				if (get(r).counts_says) $$render(consequent_6);
			});
			template_effect(($0) => {
				set_attribute(a, "href", $0);
				set_text(text_7, ` ${get(r).title ?? ""}`);
			}, [() => `#change/${encodeURIComponent(get(r).change_id)}`]);
			append($$anchor, fragment_3);
		};
		var alternate_1 = ($$anchor) => {
			var fragment_5 = root_6();
			var button = first_child(fragment_5);
			var node_14 = child(button);
			Icon(node_14, {
				name: "play",
				size: 14
			});
			next();
			reset(button);
			next(2);
			delegated("click", button, () => run("new-change", "plan"));
			append($$anchor, fragment_5);
		};
		if_block(node_9, ($$render) => {
			if (get(r).change_id) $$render(consequent_7);
			else $$render(alternate_1, -1);
		});
		reset(div_2);
		var node_15 = sibling(div_2, 2);
		var consequent_8 = ($$anchor) => {
			var p_2 = root_7();
			var node_16 = child(p_2);
			Icon(node_16, {
				name: "alert",
				size: 13
			});
			next();
			reset(p_2);
			append($$anchor, p_2);
		};
		if_block(node_15, ($$render) => {
			if (get(r).contradicts_done) $$render(consequent_8);
		});
		reset(header);
		var div_3 = sibling(header, 2);
		var nav = child(div_3);
		var ol = sibling(child(nav), 2);
		each(ol, 21, () => get(r).plan.outline, index, ($$anchor, h) => {
			var li = root_8();
			let styles;
			var text_8 = only_child(li, true);
			template_effect(($0, $1) => {
				set_class(li, 1, `h${$0 ?? ""}`, "svelte-khab3i");
				styles = set_style(li, "", styles, { "padding-left": $1 });
				set_text(text_8, get(h).text);
			}, [() => Math.min(get(h).level, 4), () => `${Math.max(0, get(h).level - 1) * .8}rem`]);
			append($$anchor, li);
		});
		reset(ol);
		reset(nav);
		var div_4 = sibling(nav, 2);
		var node_17 = child(div_4);
		var consequent_9 = ($$anchor) => {
			var section = root_9();
			var h2 = child(section);
			var node_18 = child(h2);
			Icon(node_18, {
				name: "question",
				size: 13
			});
			next();
			reset(h2);
			var ul = sibling(h2, 2);
			each(ul, 21, () => get(r).plan.questions, index, ($$anchor, q) => {
				var li_1 = root_8();
				var text_9 = only_child(li_1, true);
				template_effect(($0) => set_text(text_9, $0), [() => typeof get(q) === "string" ? get(q) : JSON.stringify(get(q))]);
				append($$anchor, li_1);
			});
			reset(ul);
			reset(section);
			append($$anchor, section);
		};
		if_block(node_17, ($$render) => {
			if (get(r).plan.questions?.length) $$render(consequent_9);
		});
		var section_1 = sibling(node_17, 2);
		var node_19 = sibling(child(section_1), 2);
		var consequent_10 = ($$anchor) => {
			var p_3 = root_10();
			append($$anchor, p_3);
		};
		var consequent_11 = ($$anchor) => {
			var p_4 = root_11();
			append($$anchor, p_4);
		};
		var alternate_2 = ($$anchor) => {
			var fragment_6 = root_15();
			var table = first_child(fragment_6);
			var tbody = sibling(child(table));
			each(tbody, 21, () => get(r).trace.edges, ([token, tasks]) => token, ($$anchor, $$item) => {
				var $$array = user_derived(() => to_array(get($$item), 2));
				let token = () => get($$array)[0];
				let tasks = () => get($$array)[1];
				var tr = root_12();
				var td = child(tr);
				var code_1 = child(td);
				var text_10 = only_child(code_1, true);
				reset(td);
				var td_1 = sibling(td);
				var text_11 = only_child(td_1, true);
				var td_2 = sibling(td_1);
				var text_12 = only_child(td_2, true);
				reset(tr);
				template_effect(($0) => {
					set_text(text_10, token());
					set_text(text_11, tasks().length);
					set_text(text_12, $0);
				}, [() => tasks().filter((t) => t.done).length]);
				append($$anchor, tr);
			});
			reset(tbody);
			reset(table);
			var node_20 = sibling(table, 2);
			var consequent_12 = ($$anchor) => {
				var p_5 = root_13();
				var text_13 = only_child(p_5);
				template_effect(($0) => set_text(text_13, `No task cites: ${$0 ?? ""}`), [() => get(r).trace.orphan_requirements.join(", ")]);
				append($$anchor, p_5);
			};
			if_block(node_20, ($$render) => {
				if (get(r).trace.orphan_requirements.length) $$render(consequent_12);
			});
			var node_21 = sibling(node_20, 2);
			var consequent_13 = ($$anchor) => {
				var p_6 = root_14();
				var text_14 = only_child(p_6);
				template_effect(() => set_text(text_14, `${get(r).trace.tasks_citing_nothing.length ?? ""} tasks cite no requirement.`));
				append($$anchor, p_6);
			};
			if_block(node_21, ($$render) => {
				if (get(r).trace.tasks_citing_nothing.length) $$render(consequent_13);
			});
			append($$anchor, fragment_6);
		};
		if_block(node_19, ($$render) => {
			if (!get(r).trace) $$render(consequent_10);
			else if (get(r).trace.unrecognised) $$render(consequent_11, 1);
			else $$render(alternate_2, -1);
		});
		reset(section_1);
		reset(div_4);
		reset(div_3);
		reset(article);
		template_effect(() => {
			set_text(text, ` ${get(found).p.project ?? ""} `);
			set_text(text_1, get(r).plan.path);
			set_text(text_2, get(r).plan.outline[0]?.text ?? get(r).plan.path);
			set_text(text_5, `${get(r).plan.files ?? ""} files`);
		});
		append($$anchor, article);
	};
	if_block(node, ($$render) => {
		if (!focus() || !get(found)) $$render(consequent_1);
		else $$render(alternate_3, -1);
	});
	append($$anchor, fragment);
	pop();
}
delegate(["click"]);
//#endregion
export { Doc as default };

//# sourceMappingURL=Doc2.js.map