#!/usr/bin/env node
// Screenshots for the README (STYLE-GUIDE.md §17.3): taken by the page itself
// against `vite dev` with the mock backend, in the dark theme, at the app's
// own accent, 1500 px wide, so a picture cannot go stale without the build
// noticing. Run from the repository root:
//
//   node tools/shots.mjs                     every picture into docs/images/
//   node tools/shots.mjs window search       some of them
//   node tools/shots.mjs --out target/shots  somewhere else, to try the script
//
// Firefox is driven headless over WebDriver BiDi (its --remote-debugging-port)
// with Node's own WebSocket, so nothing is installed: `firefox --screenshot`
// captures at the load event, before the mock has answered, and cannot wait
// or crop. Chromium hangs on the development machine. The mock's ?fast switch
// zeroes its delays; the script still waits until the page has no skeleton
// and no pending picture before it captures. Detail pictures are clipped to
// the module they show, at natural size (§17.3), by asking the page for the
// element's rectangle; the whole-window picture is the 1500 × 900 viewport.
//
// BAP_SHOTS_BASE=http://localhost:1420/ uses a dev server that is already up.

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(import.meta.url), "..", "..");
const VIEWPORT = { width: 1500, height: 900 };

/**
 * name: the file, docs/images/<name>.png. query: what the mock and the shell
 * read. clip: the module to crop to, as a selector or a list of them tried in
 * order (the module when the page draws one, the content column until then),
 * with an optional cap on its size (a long list is cut, not shrunk); no clip
 * is the whole viewport.
 * act: an expression the page runs before the capture (a click), then it
 * settles again. The README uses window, search, detail and updates; the
 * rest are for the docs and for looking at the pages.
 *
 * TODO(pages-search): the search page reads no query yet. Once it takes
 * ?q=<text> (the parameter the shell will pass on), window.png shows a search
 * for "steam" and search.png one for "gimp", as the README's captions say.
 */
export const SHOTS = [
  { name: "window", query: "?view=search&q=steam&fast&selfupdate=none" },
  // The list pages are the width of the window, so a module crop at 760 cut
  // their rows in half; they are taken whole, like the window itself.
  { name: "search", query: "?view=search&q=gimp&fast&selfupdate=none" },
  // The detail page finds its application in the search store, which the
  // seeded query fills; without q= it reports the application as gone.
  { name: "detail", query: "?view=app&app=com.valvesoftware.Steam&q=steam&fast&selfupdate=none" },
  { name: "installed", query: "?view=installed&fast&selfupdate=none" },
  { name: "updates", query: "?view=updates&fast" },
  { name: "drivers", query: "?view=drivers&fast&selfupdate=none" },
  { name: "settings", query: "?view=settings&fast&selfupdate=none" },
  {
    name: "activity",
    query: "?view=installed&fast&selfupdate=none&run=install:pacman:gimp,install:aur:spotify&hold&log",
    clip: { selector: ".bs-activity" },
  },
  {
    name: "confirm",
    query: "?view=installed&fast&selfupdate=none&confirm=install:pacman:gimp,install:aur:spotify",
    clip: { selector: ".bs-dialog" },
  },
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function freePort() {
  return new Promise((resolve, reject) => {
    const s = createServer();
    s.on("error", reject);
    s.listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
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
    await sleep(250);
  }
  throw new Error(`${url} did not answer within ${ms} ms.`);
}

/** A dev server for the run, on a port nobody else has, stopped after. */
export async function startDev() {
  const port = await freePort();
  const base = `http://localhost:${port}/`;
  const vite = join(root, "node_modules", ".bin", "vite");
  const child = spawn(vite, ["--port", String(port), "--strictPort"], { cwd: root, stdio: "ignore" });
  child.on("error", (e) => console.error(`vite could not start: ${e.message}`));
  await waitFor(base);
  return { base, stop: () => child.kill("SIGTERM") };
}

/** Headless Firefox with a throwaway profile, spoken to over BiDi. */
export async function startFirefox() {
  const browser = process.env.BAP_SHOTS_BROWSER ?? "firefox";
  const port = await freePort();
  const profile = mkdtempSync(join(tmpdir(), "bap-shots-"));
  const child = spawn(browser, ["--headless", "--no-remote", "--profile", profile, `--remote-debugging-port=${port}`, "about:blank"], { stdio: "ignore" });
  child.on("error", (e) => console.error(`${browser} could not start: ${e.message}`));

  let ws = null;
  for (let i = 0; i < 150 && !ws; i += 1) {
    await sleep(200);
    ws = await new Promise((resolve) => {
      const w = new WebSocket(`ws://127.0.0.1:${port}/session`);
      w.onopen = () => resolve(w);
      w.onerror = () => resolve(null);
    });
  }
  if (!ws) {
    child.kill();
    rmSync(profile, { recursive: true, force: true });
    throw new Error(`${browser} did not open its remote port within 30 s.`);
  }

  let nextId = 1;
  const pending = new Map();
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    const settle = msg.id !== undefined ? pending.get(msg.id) : undefined;
    if (settle) {
      pending.delete(msg.id);
      settle(msg);
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, (msg) => (msg.type === "error" ? reject(new Error(`${method}: ${msg.message}`)) : resolve(msg.result)));
      ws.send(JSON.stringify({ id, method, params }));
    });

  await send("session.new", { capabilities: {} });
  const tree = await send("browsingContext.getTree");
  const context = tree.contexts[0].context;
  await send("browsingContext.setViewport", { context, viewport: VIEWPORT });

  const evaluate = async (expression) => {
    const r = await send("script.evaluate", { expression, target: { context }, awaitPromise: true });
    if (r.type === "exception") throw new Error(r.exceptionDetails?.text ?? "the page threw");
    return r.result?.value;
  };

  return {
    /** Load a page and wait until it has drawn what it has: no skeleton, no busy region, every picture in. */
    async open(url, settleMs = 8000) {
      await send("browsingContext.navigate", { context, url, wait: "complete" });
      await this.settle(settleMs);
    },
    async settle(settleMs = 8000) {
      const until = Date.now() + settleMs;
      while (Date.now() < until) {
        const busy = await evaluate(
          `document.querySelector('.bs-skel, [aria-busy="true"]') !== null || [...document.images].some((i) => !i.complete)`,
        );
        if (busy === false) break;
        await sleep(100);
      }
      // The 160 ms fade of a dialog or a toast (§13), plus a frame.
      await sleep(400);
    },
    evaluate,
    /** The box of the first selector that matches, in the viewport; null when none does. */
    async rect(selector) {
      const list = Array.isArray(selector) ? selector : [selector];
      const json = await evaluate(
        `(() => { for (const s of ${JSON.stringify(list)}) { const e = document.querySelector(s); if (e) return JSON.stringify(e.getBoundingClientRect()); } return null; })()`,
      );
      return json ? JSON.parse(json) : null;
    },
    async screenshot(file, clip) {
      const params = { context, origin: "viewport", format: { type: "image/png" } };
      if (clip) {
        const x = Math.max(0, Math.floor(clip.x));
        const y = Math.max(0, Math.floor(clip.y));
        params.clip = {
          type: "box",
          x,
          y,
          width: Math.min(Math.ceil(clip.width), VIEWPORT.width - x),
          height: Math.min(Math.ceil(clip.height), VIEWPORT.height - y),
        };
      }
      const shot = await send("browsingContext.captureScreenshot", params);
      writeFileSync(file, Buffer.from(shot.data, "base64"));
    },
    async close() {
      await send("session.end").catch(() => undefined);
      ws.close();
      child.kill();
      rmSync(profile, { recursive: true, force: true });
    },
  };
}

/** One picture: open, settle, act if asked, clip to the module, write. */
export async function capture(firefox, base, shot, out) {
  const file = join(out, `${shot.name}.png`);
  await firefox.open(`${base}${shot.query}`);
  if (shot.act) {
    await firefox.evaluate(shot.act);
    await firefox.settle(2000);
  }
  let clip = null;
  if (shot.clip) {
    const r = await firefox.rect(shot.clip.selector);
    if (r) {
      clip = {
        x: r.x,
        y: r.y,
        width: shot.clip.width ? Math.min(r.width, shot.clip.width) : r.width,
        height: shot.clip.height ? Math.min(r.height, shot.clip.height) : r.height,
      };
    } else {
      console.warn(`${shot.name}: nothing matches ${[].concat(shot.clip.selector).join(" or ")}; the whole window is taken instead.`);
    }
  }
  await firefox.screenshot(file, clip);
  if (!existsSync(file)) throw new Error(`Firefox answered without a picture for ${file}.`);
  console.log(`wrote ${file}`);
}

async function main() {
  const args = process.argv.slice(2);
  let out = join(root, "docs", "images");
  const wanted = [];
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === "--out") {
      out = join(process.cwd(), args[i + 1] ?? "");
      i += 1;
    } else wanted.push(args[i]);
  }
  const shots = wanted.length ? SHOTS.filter(({ name }) => wanted.includes(name)) : SHOTS;
  if (shots.length === 0) {
    console.error(`No such picture. Known: ${SHOTS.map((s) => s.name).join(", ")}.`);
    process.exit(2);
  }
  mkdirSync(out, { recursive: true });

  const given = process.env.BAP_SHOTS_BASE;
  const dev = given ? { base: given, stop: () => undefined } : await startDev();
  const firefox = await startFirefox();
  try {
    for (const shot of shots) await capture(firefox, dev.base, shot, out);
  } finally {
    await firefox.close();
    dev.stop();
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
