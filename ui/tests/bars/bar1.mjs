// Bar 1 — first verified change. From an empty Devplane home, a sample
// repository that declares nothing, and the echo agent, to a change reading
// *verified*: the seconds, the inputs, whether any step had no path in the
// window (it is then done from a command line so the rest is still measured,
// and named *terminal*), and whether any step wrote configuration before a
// person confirmed it.
//
// The window's first-run steps are found by `data-first-run-step="<step>"`
// on the control that takes the step; a step whose control is absent has no
// path in the window. A step the window shows as done by itself (marked
// `data-first-run-done`) took no input. A control may appear once the previous
// step's request has landed, so each is waited for briefly — and the wait is
// in the measured seconds. New change, its fields and Start are found as the
// form labels them.
//
// The page stands in for the app's window: the host is handed a window key
// (a debug build reads `DEVPLANE_WINDOW_KEY`) and the page the same key, as
// the app hands its webview, so trusting from the page carries the window's
// proof as it does in the app.
//
// `DEVPLANE_BIN` names the host binary (default `debug/devplane` under
// `CARGO_TARGET_DIR`, else `../target`),
// `ECHO_AGENT` the echo agent (default beside it under `examples/`).

import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { here, sleep } from "./driver.mjs";

const root = resolve(here, "..");

/**
 * The walk, in order. `confirms`: the step is the person confirming a shown
 * change, so configuration may change across it. `window` takes the step in
 * the page and returns the inputs it took, or `null` when the window has no
 * path for it; `terminal` does it from a command line.
 * @typedef {{ name: string, confirms: boolean, window: (c: Ctx) => Promise<number | null>, terminal: (c: Ctx) => string }} Step
 * @typedef {{ page: any, bin: string, env: Record<string, string>, repo: string, home: string, api: (path: string, init?: any) => Promise<any> }} Ctx
 */

/** A first-run control the window offers for `step`, if any, waited for up to 5 s. */
async function firstRun(/** @type {any} */ page, /** @type {string} */ step) {
  const el = page.locator(`[data-first-run-step="${step}"]`).first();
  for (let i = 0; i < 50; i++) {
    if ((await el.count()) > 0 && (await el.isVisible())) return el;
    await sleep(100);
  }
  return null;
}

/** A first-run step taken with one input on its control, or none when the window shows it done by itself. */
const oneClick = (/** @type {string} */ step) => async (/** @type {Ctx} */ c) => {
  const el = await firstRun(c.page, step);
  if (!el) return null;
  if ((await el.getAttribute("data-first-run-done")) === "true") return 0;
  await el.click();
  await sleep(300);
  return 1;
};

/** @type {Step[]} */
const STEPS = [
  { name: "answer the update question", confirms: false, window: oneClick("update"), terminal: () => "nothing to run: no window path and no command" },
  { name: "skip connect", confirms: false, window: oneClick("skip-connect"), terminal: () => "nothing to run: no window path and no command" },
  {
    name: "add the repository",
    confirms: false,
    window: async (c) => {
      const el = await firstRun(c.page, "add-repository");
      if (!el) return null;
      await el.fill(c.repo);
      await el.press("Enter");
      await sleep(500);
      return 1;
    },
    terminal: () => "registered by `devplane project trust` at the next step",
  },
  {
    name: "trust the repository",
    confirms: true,
    window: oneClick("trust"),
    terminal: (c) => {
      execFileSync(c.bin, ["project", "trust", c.repo, "--yes"], { env: c.env, stdio: "pipe" });
      return `devplane project trust ${"<repo>"} --yes`;
    },
  },
  { name: "pick a gate candidate", confirms: false, window: oneClick("pick-check"), terminal: () => "written by hand at the confirm step" },
  { name: "continue to the diff", confirms: false, window: oneClick("show-diff"), terminal: () => "written by hand at the confirm step" },
  {
    name: "confirm the diff",
    confirms: true,
    window: oneClick("confirm-diff"),
    terminal: (c) => {
      writeFileSync(join(c.repo, "devplane.toml"), '[gates]\ncheck = ["npm test"]\n');
      git(c.repo, ["add", "devplane.toml"]);
      git(c.repo, ["commit", "-qm", "declare a check"]);
      return 'devplane.toml `[gates] check = ["npm test"]` written and committed by hand';
    },
  },
  {
    name: "New change",
    confirms: false,
    window: async (c) => {
      const b = c.page.getByRole("button", { name: /New change/ }).first();
      if (!(await b.count())) return null;
      await b.click();
      await c.page.locator("form.new").waitFor({ timeout: 5000 });
      await sleep(800);
      // The project, when the form did not choose the only one itself.
      const projects = c.page.locator("form.new .proj");
      let n = 1;
      if ((await projects.count()) > 0 && (await c.page.locator("form.new .proj[aria-pressed=true]").count()) === 0) {
        await projects.first().click();
        n += 1;
      }
      return n;
    },
    terminal: () => "the change started from the command line at Start",
  },
  {
    name: "type the intent",
    confirms: false,
    window: async (c) => {
      const t = c.page.locator("form.new #title");
      if (!(await t.count())) return null;
      await t.fill("Say hello");
      return 1;
    },
    terminal: () => "the intent given on the command line at Start",
  },
  {
    name: "choose the agent",
    confirms: false,
    window: async (c) => {
      const s = c.page.locator("form.new select#agent");
      if (!(await s.count())) return null;
      const ids = await s.locator("option").evaluateAll((os) => os.map((o) => /** @type {HTMLOptionElement} */ (o).value));
      if (!ids.includes("echo")) return null;
      await s.selectOption("echo");
      return 1;
    },
    terminal: () => "--agent echo on the command line at Start",
  },
  {
    name: "start",
    confirms: true,
    window: async (c) => {
      const go = c.page.locator("form.new button[type=submit]");
      if (!(await go.count())) return null;
      for (let i = 0; i < 20 && (await go.isDisabled()); i++) await sleep(250);
      if (await go.isDisabled()) return null;
      await go.click();
      return 1;
    },
    terminal: (c) => {
      execFileSync(c.bin, ["change", "start", "--project", c.repo, "--agent", "echo", "Say hello"], { env: c.env, stdio: "pipe", cwd: c.repo });
      return 'devplane change start --agent echo "Say hello"';
    },
  },
];

function git(/** @type {string} */ cwd, /** @type {string[]} */ args) {
  return execFileSync("git", args, {
    cwd,
    stdio: "pipe",
    env: { ...process.env, GIT_AUTHOR_NAME: "Bar", GIT_AUTHOR_EMAIL: "bar@example.com", GIT_COMMITTER_NAME: "Bar", GIT_COMMITTER_EMAIL: "bar@example.com" },
  }).toString();
}

/** Every file under `dir` (but `.git`), hashed. */
function files(/** @type {string} */ dir, /** @type {string} */ prefix = "") {
  /** @type {Record<string, string>} */
  const out = {};
  if (!existsSync(dir)) return out;
  for (const f of readdirSync(dir)) {
    if (f === ".git") continue;
    const p = join(dir, f);
    const st = statSync(p);
    if (st.isDirectory()) Object.assign(out, files(p, `${prefix}${f}/`));
    else out[`${prefix}${f}`] = createHash("sha256").update(readFileSync(p)).digest("hex");
  }
  return out;
}

/**
 * The configuration a step may not change unconfirmed: the repository's files,
 * the agent's settings, Devplane's own configuration files, and each project's
 * trust as the host reports it. The store and the host's own files are not
 * compared.
 */
async function snapshot(/** @type {Ctx} */ c, /** @type {string} */ claude) {
  /** @type {Record<string, string>} */
  const s = {};
  for (const [k, v] of Object.entries(files(c.repo))) s[`repository/${k}`] = v;
  for (const [k, v] of Object.entries(files(claude))) s[`agent settings/${k}`] = v;
  for (const f of ["policy.toml", "agents.toml", "app.toml"]) {
    const p = join(c.home, f);
    if (existsSync(p)) s[`~/.devplane/${f}`] = createHash("sha256").update(readFileSync(p)).digest("hex");
  }
  const projects = await c.api("/api/projects").catch(() => []);
  for (const p of Array.isArray(projects) ? projects : []) s[`trust of ${p.name}`] = String(p.trusted);
  return s;
}

/** @param {Record<string, string>} a @param {Record<string, string>} b */
const changed = (a, b) => [...new Set([...Object.keys(a), ...Object.keys(b)])].filter((k) => a[k] !== b[k]);

/** @param {any} driver @param {any} budget */
export default async function bar1(driver, budget) {
  /** @type {string[]} */
  const findings = [];
  /** @type {string[]} */
  const measured = [];
  const bin = process.env.DEVPLANE_BIN ?? resolve(process.env.CARGO_TARGET_DIR ? resolve(root, process.env.CARGO_TARGET_DIR) : resolve(root, "target"), "debug/devplane");
  const echo = process.env.ECHO_AGENT ?? resolve(bin, "../examples/echo_agent");
  for (const [what, p] of [["the host binary", bin], ["the echo agent", echo]]) {
    if (!existsSync(p)) {
      return { measured: [], findings: [`host-failed · no ${what} at ${p} — \`cargo build --locked --example echo_agent && cargo build --locked\`, or set ${what === "the host binary" ? "DEVPLANE_BIN" : "ECHO_AGENT"}`] };
    }
  }

  // The sandbox: an empty home with the echo agent installed (before the
  // clock, as installing an agent is), and the sample repository.
  const sb = mkdtempSync(join(tmpdir(), "devplane-bar1-"));
  const home = join(sb, "home");
  const claude = join(sb, "claude");
  const repo = join(sb, "sample");
  mkdirSync(home, { recursive: true });
  mkdirSync(join(claude, "projects"), { recursive: true });
  mkdirSync(repo, { recursive: true });
  writeFileSync(join(home, "agents.toml"), `[agents.echo]\nname = "Echo"\ncommand = ${JSON.stringify(echo)}\n`);
  git(repo, ["init", "-q", "-b", "main"]);
  writeFileSync(join(repo, "package.json"), JSON.stringify({ name: "sample", private: true, scripts: { test: "node -e \"process.exit(1 + 1 === 2 ? 0 : 1)\"" } }, null, 2) + "\n");
  writeFileSync(join(repo, ".gitignore"), ".devplane/\n");
  git(repo, ["add", "-A"]);
  git(repo, ["commit", "-qm", "a sample repository"]);

  /** @type {Record<string, string>} */
  const env = {};
  for (const [k, v] of Object.entries(process.env)) {
    // An agent session's variables would make the host think it runs inside one.
    if (v !== undefined && !/^(CLAUDECODE|CLAUDE_CODE_|DEVPLANE_RUN|CODEX_|COPILOT_CLI)/.test(k)) env[k] = v;
  }
  // The window's key, as the app mints one for its webview.
  const windowKey = createHash("sha256").update(`${sb}${Date.now()}`).digest("hex");
  Object.assign(env, { DEVPLANE_HOME: home, CLAUDE_CONFIG_DIR: claude, DEVPLANE_WINDOW_KEY: windowKey });

  const host = spawn(bin, ["up", "--no-browser"], { cwd: sb, env, stdio: ["ignore", "pipe", "pipe"] });
  let log = "";
  host.stdout.on("data", (d) => (log += d));
  host.stderr.on("data", (d) => (log += d));
  /** @type {any} */
  let vite = null;
  /** @type {any} */
  let page = null;
  try {
    for (let i = 0; i < 100 && !existsSync(join(home, "host.json")); i++) {
      if (host.exitCode !== null) break;
      await sleep(100);
    }
    if (!existsSync(join(home, "host.json"))) {
      return { measured: [], findings: [`host-failed · the host did not start: ${log.trim().split("\n").slice(-2).join(" ").slice(0, 200) || `exit ${host.exitCode}`}`] };
    }
    await sleep(200);
    const port = JSON.parse(readFileSync(join(home, "host.json"), "utf8")).port;
    const token = readFileSync(join(home, "token"), "utf8").trim();
    const api = async (/** @type {string} */ path, /** @type {any} */ init = {}) => {
      const r = await fetch(`http://127.0.0.1:${port}${path}`, { ...init, headers: { authorization: `Bearer ${token}`, "content-type": "application/json", ...(init.headers ?? {}) } });
      if (!r.ok) throw new Error(`${path}: ${r.status}`);
      return r.json();
    };

    // The interface from this tree's sources, its API proxied to this host.
    const fixtures = process.env.DEVPLANE_FIXTURES;
    delete process.env.DEVPLANE_FIXTURES;
    process.env.DEVPLANE_PORT = String(port);
    const { createServer } = await import("vite");
    vite = await createServer({ root: here, configFile: resolve(here, "vite.config.ts"), logLevel: "error", server: { port: 0, host: "127.0.0.1", hmr: false } });
    await vite.listen();
    if (fixtures) process.env.DEVPLANE_FIXTURES = fixtures;
    const base = vite.resolvedUrls?.local[0] ?? "";

    page = await driver.page();
    await page.addInitScript((/** @type {string} */ key) => {
      Object.defineProperty(window, "__devplane_window_key", { value: key, writable: false, configurable: false, enumerable: false });
    }, windowKey);
    /** @type {Ctx} */
    const c = { page, bin, env, repo, home, api };
    const started = Date.now();
    await page.goto(`${base}?token=${token}`);
    await page.locator("main#surface").waitFor({ timeout: 10_000 });
    await sleep(1000);

    let inputs = 0;
    /** @type {string[]} */
    const steps = [];
    /** @type {string[]} */
    const terminal = [];
    for (const step of STEPS) {
      const before = await snapshot(c, claude);
      let n = null;
      try {
        n = await step.window(c);
      } catch {
        n = null;
      }
      let path = "window";
      if (n === null) {
        path = "terminal";
        let did = "";
        try {
          did = step.terminal(c);
        } catch (e) {
          did = `failed: ${e instanceof Error ? e.message.split("\n")[0] : String(e)}`;
        }
        terminal.push(step.name);
        steps.push(`step     ${step.name.padEnd(30)} terminal — ${did}`);
        findings.push(`terminal · ${step.name} · no path in the window`);
      } else {
        inputs += n;
        steps.push(`step     ${step.name.padEnd(30)} window — ${n} input${n === 1 ? "" : "s"}`);
      }
      if (!step.confirms) {
        const after = await snapshot(c, claude);
        const wrote = changed(before, after);
        for (const f of wrote) findings.push(`unconfirmed-write · ${step.name} (${path}) · ${f}`);
      }
    }

    // Until the change reads *verified*, in the host and in the window.
    let last = "no change started";
    let verified = false;
    while (Date.now() - started < budget.seconds * 1000 * 2) {
      const changes = await api("/api/changes").catch(() => null);
      const list = Array.isArray(changes) ? changes : (changes?.changes ?? []);
      const mine = list.find((/** @type {any} */ x) => x.title === "Say hello") ?? list[0];
      if (mine) last = `${mine.title}: ${mine.state}`;
      if (mine?.state === "verified") {
        verified = true;
        break;
      }
      if (mine && /failed|stopped|abandoned/.test(String(mine.state))) break;
      await sleep(500);
    }
    const seconds = (Date.now() - started) / 1000;
    let shown = false;
    if (verified) {
      for (let i = 0; i < 20 && !shown; i++) {
        shown = /\bverified\b/.test(await page.locator("body").innerText().catch(() => ""));
        if (!shown) await sleep(250);
      }
    }
    measured.unshift(`${inputs} inputs · ${seconds.toFixed(1)} s · ${terminal.length} terminal step${terminal.length === 1 ? "" : "s"}`);
    if (!verified) findings.push(`never-verified · last state: ${last}`);
    else if (!shown) findings.push(`never-verified · the host says verified; the window never showed it`);
    if (inputs > budget.inputs) findings.push(`over-inputs · ${inputs} inputs, budget ${budget.inputs}`);
    if (seconds > budget.seconds) findings.push(`over-seconds · ${seconds.toFixed(1)} s, budget ${budget.seconds} s`);
    return { measured, findings, notes: steps };
  } finally {
    await page?.context().close().catch(() => {});
    await vite?.close().catch(() => {});
    host.kill();
    await sleep(300);
    rmSync(sb, { recursive: true, force: true });
  }
}
