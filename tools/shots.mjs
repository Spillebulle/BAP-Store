#!/usr/bin/env node
// Screenshots for the README (STYLE-GUIDE.md §17.3): taken by the page itself
// against `vite dev` with the mock backend, in the dark theme, at the app's
// own accent, 1500 px wide, so a picture cannot go stale without the build
// noticing. Run from the repository root:
//
//   node tools/shots.mjs            every picture into docs/images/
//   node tools/shots.mjs window     one of them
//
// Needs a Chromium-based browser: BAP_SHOTS_BROWSER, else ~/.local/bin/vivaldi-snapshot,
// else chromium on PATH. Node's own fetch polls the dev server until it answers.
// The mock world's delays are a few hundred milliseconds, so each shot gives
// the page a virtual-time budget before the capture rather than a fixed sleep.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(import.meta.url), "..", "..");
const out = join(root, "docs", "images");
const port = 1420;
const base = `http://localhost:${port}/`;

// Name, query. The mock reads ?view= so a picture can be taken of any page
// without clicking through the shell.
const SHOTS = [
  ["window", "?view=search"],
  ["installed", "?view=installed"],
  ["updates", "?view=updates"],
  ["drivers", "?view=drivers"],
  ["settings", "?view=settings"],
  ["components", "?gallery"],
];

function browser() {
  const env = process.env.BAP_SHOTS_BROWSER;
  if (env) return env;
  const local = join(homedir(), ".local", "bin", "vivaldi-snapshot");
  if (existsSync(local)) return local;
  return "chromium";
}

function run(cmd, args, opts = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { stdio: "inherit", ...opts });
    child.on("error", reject);
    child.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exited with ${code}`))));
  });
}

async function waitFor(url, ms = 30000) {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`${url} did not answer within ${ms} ms. Is port ${port} free?`);
}

async function main() {
  const wanted = process.argv.slice(2);
  const shots = wanted.length ? SHOTS.filter(([name]) => wanted.includes(name)) : SHOTS;
  if (shots.length === 0) {
    console.error(`No such picture. Known: ${SHOTS.map(([n]) => n).join(", ")}.`);
    process.exit(2);
  }
  mkdirSync(out, { recursive: true });

  // A dev server that is already up is used as it is; otherwise one is started
  // for the run and stopped after, so the script works in a clean checkout.
  let dev = null;
  let already = false;
  try {
    await waitFor(base, 500);
    already = true;
  } catch {
    dev = spawn("npm", ["run", "dev"], { cwd: root, stdio: "ignore", detached: true });
    await waitFor(base);
  }

  // A throwaway profile, or a browser that is already open as the user's own
  // takes the URL, opens it in a tab, and exits 0 without writing a picture.
  const profile = mkdtempSync(join(tmpdir(), "bap-shots-"));
  try {
    for (const [name, query] of shots) {
      const file = join(out, `${name}.png`);
      await run(browser(), [
        "--headless=new",
        "--disable-gpu",
        "--hide-scrollbars",
        "--no-first-run",
        `--user-data-dir=${profile}`,
        "--force-dark-mode",
        "--window-size=1500,900",
        "--virtual-time-budget=4000",
        `--screenshot=${file}`,
        `${base}${query}`,
      ]);
      if (!existsSync(file)) throw new Error(`${browser()} exited without writing ${file}.`);
      console.log(`wrote ${file}`);
    }
  } finally {
    rmSync(profile, { recursive: true, force: true });
    if (dev && !already) process.kill(-dev.pid, "SIGTERM");
  }
}

main().catch((e) => {
  console.error(e.message);
  process.exit(1);
});
