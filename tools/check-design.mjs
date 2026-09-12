#!/usr/bin/env node
// The design rules that a grep can enforce, run as `npm run check:design`:
//
//   - no raw hex colour anywhere in frontend/src: a colour is a token;
//   - no em dash (U+2014) anywhere: the copy rule is a full stop and a new
//     sentence, and it applies to comments too so one cannot leak into a string;
//   - no class="web" (or className="web"): this is a desktop application;
//   - every px size in app.css is a token value or on the fine grid.
//
// tokens.css is exempt from all of it: it is a verbatim copy of the house file,
// the one place a colour is a value, and its comments describe the web scale
// this application never stamps.
//
// Exits non-zero with file:line for each finding. Run from the repository root.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(import.meta.url), "..", "..");
const src = join(root, "frontend", "src");

// tokens.css's own values, the application-icon ladder declared in app.css,
// the dialog widths, and the 4 px grid. A size that is not here wants a token,
// not an entry here.
const ALLOWED_PX = new Set([
  0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 13, 15, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 40, 44, 48, 64, 96,
  100, 150, 240, 260, 264, 280, 430, 760, 1000,
]);

const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const EM_DASH = "—";
const WEB_CLASS = /class(?:Name)?\s*=\s*["'`]web["'`]/g;
const PX = /(-?\d*\.?\d+)px\b/g;

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else out.push(path);
  }
  return out;
}

function stripCssComments(text) {
  // Replace comment bodies with spaces of the same length so line numbers hold.
  return text.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
}

const findings = [];

function report(file, lineNo, what) {
  findings.push(`${relative(root, file)}:${lineNo}: ${what}`);
}

for (const file of walk(src)) {
  const text = readFileSync(file, "utf8");
  const lines = text.split("\n");
  if (file.endsWith("tokens.css")) continue;

  lines.forEach((line, i) => {
    const lineNo = i + 1;
    if (line.includes(EM_DASH)) report(file, lineNo, "em dash (U+2014); use a full stop and a new sentence");
    for (const m of line.matchAll(HEX)) report(file, lineNo, `raw hex colour ${m[0]}; use a token`);
    for (const m of line.matchAll(WEB_CLASS)) report(file, lineNo, `${m[0]}: a desktop application never stamps the web scale`);
  });

  if (file.endsWith("app.css")) {
    const bare = stripCssComments(text).split("\n");
    bare.forEach((line, i) => {
      for (const m of line.matchAll(PX)) {
        const value = Number(m[1]);
        if (!ALLOWED_PX.has(Math.abs(value))) {
          report(file, i + 1, `${m[0]} is not a token value; use a token or the 4 px grid`);
        }
      }
    });
  }
}

if (findings.length > 0) {
  for (const f of findings) console.error(f);
  console.error(`\n${findings.length} design ${findings.length === 1 ? "finding" : "findings"}.`);
  process.exit(1);
}

console.log("check-design: frontend/src is clean.");
