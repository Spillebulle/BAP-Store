// What both pages say about an edition: its label in a dropdown, which one
// is primary, and whether a transaction is already running for it.

import { useMemo } from "react";
import { useActivity } from "../../activity/store";
import type { DropdownOption } from "../../components";
import { sourceLabel, type App, type Edition, type Op, type PackageRef } from "../../types";

export function refOf(e: Edition): PackageRef {
  return { source: e.package.source, id: e.package.id };
}

export function refKey(ref: PackageRef): string {
  return `${ref.source}:${ref.id}`;
}

export function editionKey(e: Edition): string {
  return refKey(refOf(e));
}

/** The edition a row speaks for: the installed one, else the first (sources are in interface order). */
export function primaryEdition(app: App): Edition | undefined {
  return app.editions.find((e) => e.package.installed) ?? app.editions[0];
}

/** "pacman · 1.0.0.85-1 · multilib", "AUR · 1.2.95.453-1", "Flatpak · 3.2.6 · flathub · installed". */
export function editionLabel(e: Edition): string {
  const p = e.package;
  const source = sourceLabel(p.source);
  const parts = [source];
  if (p.version) parts.push(p.version);
  if (p.repo && p.repo.toLowerCase() !== source.toLowerCase()) parts.push(p.repo);
  if (p.installed) parts.push("installed");
  return parts.join(" · ");
}

/** The editions as dropdown rows; an installed one is listed but cannot be chosen for an install. */
export function installOptions(app: App): DropdownOption[] {
  return app.editions.map((e) => ({
    value: editionKey(e),
    label: editionLabel(e),
    disabled: e.package.installed,
    disabledReason: e.package.installed ? "Already installed." : undefined,
  }));
}

const LIVE = new Set(["pending", "authorising", "running"]);

function opRef(op: Op): PackageRef | null {
  return "package" in op ? op.package : null;
}

/** Package refs with a plan still running, mapped to what that plan does to them. */
export function useBusyRefs(): Map<string, Op["op"]> {
  const stamp = useActivity((s) =>
    s.order
      .map((id) => s.plans[id])
      .filter((p) => p && LIVE.has(p.state))
      .flatMap((p) => p.plan.ops.map((op) => (opRef(op) ? `${op.op} ${refKey(opRef(op) as PackageRef)}` : "")))
      .filter(Boolean)
      .join("\n"),
  );
  return useMemo(() => {
    const map = new Map<string, Op["op"]>();
    for (const line of stamp.split("\n")) {
      if (!line) continue;
      const at = line.indexOf(" ");
      map.set(line.slice(at + 1), line.slice(0, at) as Op["op"]);
    }
    return map;
  }, [stamp]);
}

/** "pacman, AUR and Flatpak": plain English, no Oxford comma. */
export function joinAnd(words: string[]): string {
  if (words.length === 0) return "";
  if (words.length === 1) return words[0];
  return `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
}
