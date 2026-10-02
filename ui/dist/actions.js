import { h as spell, i as api } from "./api.js";
import { t as copyText } from "./resource.svelte.js";
import { t as safeHref } from "./href.js";
//#region src/surfaces/inbox/actions.ts
function outcome(r, ours) {
	const commands = [r?.push, r?.create].filter((c) => !!c);
	if (commands.length) return {
		said: r?.says ?? "Nothing was pushed — run these yourself:",
		undo: null,
		commands
	};
	return {
		said: r?.says ?? ours,
		undo: null
	};
}
var ROUTES = {
	retry: {
		of: "change",
		route: "/api/changes/{id}/retry",
		says: "retrying"
	},
	resume: {
		of: "change",
		route: "/api/changes/{id}/resume",
		says: "resumed"
	},
	tell_run: {
		of: "change",
		route: "/api/changes/{id}/drift/tell",
		says: "told — the run was handed the files that changed, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	},
	accept_drift: {
		of: "change",
		route: "/api/changes/{id}/drift/accept",
		says: "accepted — the change now works to what the run saw, and the decision is recorded as yours",
		body: (item) => ({ run: item.run_id })
	}
};
var failed = (e) => e instanceof Error ? e.message : String(e);
async function act(item, action) {
	const r = ROUTES[action];
	const id = r?.of === "change" ? item.change_id : item.run_id;
	if (!r || !id) return {
		said: `${action} cannot be done from here`,
		undo: null
	};
	try {
		return outcome(await api(r.route.replace("{id}", encodeURIComponent(id)), {
			method: "POST",
			...r.body ? { body: JSON.stringify(r.body(item)) } : {}
		}), r.says);
	} catch (e) {
		return {
			said: `${action} did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function answer(item, what) {
	const ask = item.ask ?? item.request_id;
	if (!ask) return {
		said: "this one cannot be answered from here",
		undo: null
	};
	try {
		const r = await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				...what,
				hold: true
			})
		});
		return {
			said: r?.held ? `${r.held.says} — held; z takes it back before it is sent` : "answered — on its way to the agent. That cannot be taken back.",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
function snoozeRoute(item) {
	return item.change_id ? `/api/changes/${encodeURIComponent(item.change_id)}/snooze` : item.run_id ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze` : item.project_id ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze` : null;
}
function untilActivity(item) {
	return !!(item.change_id || item.run_id);
}
async function snooze(item) {
	if (!untilActivity(item)) return snoozeHour(item);
	const where = snoozeRoute(item);
	if (!where) return {
		said: "there is nothing to snooze this against",
		undo: null
	};
	const kind = encodeURIComponent(item.kind);
	try {
		await api(`${where}?until=activity&kind=${kind}`, { method: "POST" });
		return {
			said: `snoozed until the ${item.change_id ? "change" : "session"} next reports activity — counted under snoozed`,
			undo: {
				says: "put it back",
				where: `${where}?minutes=0&kind=${kind}`
			}
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function snoozeHour(item) {
	const where = snoozeRoute(item);
	if (!where) return {
		said: "there is nothing to snooze this against",
		undo: null
	};
	const kind = encodeURIComponent(item.kind);
	try {
		await api(`${where}?minutes=60&kind=${kind}`, { method: "POST" });
		return {
			said: "hidden for an hour — counted under snoozed",
			undo: {
				says: "put it back",
				where: `${where}?minutes=0&kind=${kind}`
			}
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function bringBack(row) {
	try {
		await api(row.back, { method: "POST" });
		return {
			said: `${row.title} — back in the list`,
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
function rowActions(item) {
	const acts = item.actions ?? [];
	const has = (a) => acts.includes(a);
	const out = [];
	const form = (item.form ?? []).length > 0 ? item.form ?? [] : null;
	if (form && (has("choose") || has("reply"))) {
		form.forEach((q, qi) => {
			q.options.forEach((o, oi) => out.push({
				id: `option:${q.field}:${oi}`,
				label: form.length > 1 ? `${q.title}: ${o.label}` : o.label,
				key: qi === 0 && oi < 9 ? String(oi + 1) : null
			}));
			if (q.custom_field) out.push({
				id: `reply:${q.field}`,
				label: `answer in your own words: ${q.title}`,
				key: qi === 0 ? "t" : null
			});
		});
		if (form.length > 1) out.push({
			id: "send_form",
			label: `send the ${form.length} answers`,
			key: null
		});
	} else {
		if (has("choose")) {
			let n = 0;
			for (const o of item.options ?? []) {
				if (!o.id) continue;
				n += 1;
				out.push({
					id: `option:${n}`,
					label: o.label,
					key: n <= 9 ? String(n) : null
				});
			}
		}
		if (has("reply")) out.push({
			id: "reply",
			label: "answer in your own words",
			key: "t"
		});
	}
	if (item.offer) out.push({
		id: "copy_rule",
		label: "copy the rule",
		key: null
	});
	const sessionLink = safeHref(item.session_link);
	if (item.answer_in && (sessionLink || item.resume_command)) out.push({
		id: "session",
		label: sessionLink ? "open the session in VS Code" : "copy the resume command",
		key: "w"
	});
	if (has("allow")) out.push({
		id: "allow",
		label: "allow, once",
		key: "a"
	});
	if (has("deny")) out.push({
		id: "deny",
		label: "deny",
		key: "d"
	});
	if (has("retry")) out.push({
		id: "retry",
		label: "retry",
		key: null
	});
	if (has("resume")) out.push({
		id: "resume",
		label: "resume",
		key: null
	});
	if (has("review") && item.change_id) out.push({
		id: "review",
		label: "open the review",
		key: "r"
	});
	if (has("tell_run")) out.push({
		id: "tell_run",
		label: "tell the run",
		key: null
	});
	if (has("accept_drift")) out.push({
		id: "accept_drift",
		label: "accept the drift",
		key: null
	});
	if (has("open") && (item.run_id || item.change_id)) out.push({
		id: "open",
		label: item.change_id ? "open the change" : "open the session",
		key: "o"
	});
	if ((has("open_pr") || has("open_issue")) && safeHref(item.url)) out.push({
		id: has("open_pr") ? "open_pr" : "open_issue",
		label: has("open_pr") ? "open pull request" : "open issue",
		key: null
	});
	if (safeHref(item.launch)) out.push({
		id: "launch",
		label: "open an agent with this typed",
		key: null
	});
	if (has("snooze")) {
		const activity = untilActivity(item);
		out.push({
			id: "snooze",
			label: activity ? "snooze until there is new activity" : "snooze for an hour",
			key: "s"
		});
		if (activity) out.push({
			id: "snooze_hour",
			label: "snooze for an hour",
			key: null
		});
	}
	if (has("attach") && item.run_id) out.push({
		id: "attach",
		label: "copy the attach command",
		key: null
	});
	return out;
}
function rowCommands(item, run) {
	return rowActions(item).map((a) => ({
		label: a.label,
		icon: a.key ? "keyboard" : "right",
		hint: a.key ? spell(a.key) : void 0,
		run: () => run(a)
	}));
}
function pressAction(root, id) {
	const el = root?.querySelector(`[data-action="${CSS.escape(id)}"]`);
	if (!el) return false;
	if (el instanceof HTMLInputElement) el.focus();
	else el.click();
	return true;
}
async function takeBack(undo) {
	try {
		return {
			said: (await api(undo.where, { method: "POST" }))?.says ?? "back in the list",
			undo: null
		};
	} catch (e) {
		return {
			said: `that did not land: ${failed(e)}`,
			undo: null
		};
	}
}
async function copyCommand(cmd, then) {
	return await copyText(cmd) ? `copied: ${cmd}${then}` : `nothing was copied: this page has no clipboard — run ${cmd}`;
}
async function copyRule(rule) {
	return await copyText(rule) ? {
		said: `copied: ${rule}`,
		undo: null
	} : {
		said: "nothing was copied: this page has no clipboard — select the rule above",
		undo: null
	};
}
//#endregion
export { copyRule as a, snooze as c, untilActivity as d, copyCommand as i, snoozeHour as l, answer as n, pressAction as o, bringBack as r, rowCommands as s, act as t, takeBack as u };

//# sourceMappingURL=actions.js.map