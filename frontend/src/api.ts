// The page's one door to the Rust side: one function per Tauri command, named
// exactly as crates/bap-store/src/commands.rs registers them. Inside the
// window every call is `invoke`; in a plain browser (`vite dev`, screenshots)
// the mock world in ./mock.ts answers instead, so the page renders without a
// backend. Nothing else in frontend/src imports @tauri-apps.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl as openerOpenUrl } from "@tauri-apps/plugin-opener";
import type {
  DriversReport,
  Event,
  Op,
  Package,
  PackageRef,
  PlanPreview,
  PlanStatus,
  Picture,
  Query,
  SearchResult,
  SelfUpdate,
  Settings,
  SourceStatus,
  SystemInfo,
  UpdateList,
} from "./types";

/** Tauri stamps this on the window before any script runs; its absence means a browser. */
export const inTauri: boolean =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// The mock world carries 60 kB of fixtures, so it is loaded on demand and Vite
// splits it into a chunk the window never fetches. A static import would ship
// every Flathub description inside the application.
function mock() {
  return import("./mock");
}

/** What a command failed with, as the sentence bap_core::Error carries. */
export class ApiError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ApiError";
  }
}

function sentence(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") {
    return e.message;
  }
  return "The application did not answer. Restart it and try again.";
}

async function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (e) {
    throw new ApiError(sentence(e));
  }
}

export function system_info(): Promise<SystemInfo> {
  return inTauri ? call("system_info") : mock().then((m) => m.system_info());
}

export function sources(): Promise<SourceStatus[]> {
  return inTauri ? call("sources") : mock().then((m) => m.sources());
}

export function search(query: Query): Promise<SearchResult> {
  return inTauri ? call("search", { query }) : mock().then((m) => m.search(query));
}

export function installed(): Promise<SearchResult> {
  return inTauri ? call("installed") : mock().then((m) => m.installed());
}

export function app_details(refs: PackageRef[]): Promise<Package[]> {
  return inTauri ? call("app_details", { refs }) : mock().then((m) => m.app_details(refs));
}

export function updates(force: boolean): Promise<UpdateList> {
  return inTauri ? call("updates", { force }) : mock().then((m) => m.updates(force));
}

export function plan(ops: Op[]): Promise<PlanPreview> {
  return inTauri ? call("plan", { ops }) : mock().then((m) => m.plan(ops));
}

export function run_plan(ops: Op[]): Promise<PlanStatus> {
  return inTauri ? call("run_plan", { ops }) : mock().then((m) => m.run_plan(ops));
}

export function cancel_plan(id: string): Promise<void> {
  return inTauri ? call("cancel_plan", { id }) : mock().then((m) => m.cancel_plan(id));
}

export function active_plans(): Promise<PlanStatus[]> {
  return inTauri ? call("active_plans") : mock().then((m) => m.active_plans());
}

export function drivers(): Promise<DriversReport> {
  return inTauri ? call("drivers") : mock().then((m) => m.drivers());
}

export function settings_get(): Promise<Settings> {
  return inTauri ? call("settings_get") : mock().then((m) => m.settings_get());
}

export function settings_set(settings: Settings): Promise<Settings> {
  return inTauri ? call("settings_set", { settings }) : mock().then((m) => m.settings_set(settings));
}

export function self_update_check(force: boolean): Promise<SelfUpdate> {
  return inTauri ? call("self_update_check", { force }) : mock().then((m) => m.self_update_check(force));
}

export function self_update_apply(): Promise<PlanStatus> {
  return inTauri ? call("self_update_apply") : mock().then((m) => m.self_update_apply());
}

export function group_split(pkg: PackageRef): Promise<Settings> {
  return inTauri ? call("group_split", { package: pkg }) : mock().then((m) => m.group_split(pkg));
}

/**
 * Events from a running transaction, forwarded by the window as
 * `transaction://event`. Returns the unsubscribe. The Tauri listener is
 * asynchronous to set up, so the returned function waits for it before
 * unlistening rather than leaking a listener that was still being attached.
 */
export function onTransactionEvent(handler: (event: Event) => void): () => void {
  if (!inTauri) {
    const pending = mock().then((m) => m.onTransactionEvent(handler));
    return () => {
      void pending.then((unsubscribe) => unsubscribe());
    };
  }
  const pending = listen<Event>("transaction://event", (e) => handler(e.payload));
  return () => {
    void pending.then((unlisten) => unlisten());
  };
}

/** Something an <img> can load: the URL as-is, or a file through the asset protocol. */
export function pictureSrc(picture: Picture | null | undefined): string | null {
  if (!picture) return null;
  if (picture.kind === "url") return picture.value;
  return inTauri ? convertFileSrc(picture.value) : picture.value;
}

/** A link leaves the window through the opener plugin; a browser opens a tab. */
export async function openUrl(url: string): Promise<void> {
  if (inTauri) {
    await openerOpenUrl(url);
    return;
  }
  window.open(url, "_blank", "noopener");
}
