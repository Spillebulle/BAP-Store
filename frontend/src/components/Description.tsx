import { createElement, useMemo, type ReactNode } from "react";

// AppStream description markup is exactly these tags (p, ul, ol, li, em,
// code); strong and br are accepted because Flathub and Snap descriptions use
// them. Everything else is unwrapped to its text, so a stray <a>, <img> or
// <script> from a source's page contributes words and nothing more.
const ALLOWED = new Set(["p", "ul", "ol", "li", "em", "code", "strong", "br"]);
const RENAMED: Record<string, string> = { b: "strong", i: "em", tt: "code" };
const DROPPED = new Set(["script", "style", "template", "iframe", "object", "svg"]);

function rebuild(node: Node, key: number): ReactNode {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent;
  if (node.nodeType !== Node.ELEMENT_NODE) return null;
  const el = node as Element;
  const tag = RENAMED[el.localName] ?? el.localName;
  if (DROPPED.has(tag)) return null;
  const children = Array.from(el.childNodes).map(rebuild);
  if (tag === "br") return createElement("br", { key });
  if (!ALLOWED.has(tag)) return createElement("span", { key }, ...children);
  return createElement(tag, { key }, ...children);
}

function paragraphs(text: string): ReactNode[] {
  return text
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p, i) => createElement("p", { key: i }, p));
}

/** Parse with the browser, keep the allowed tags, wrap stray text in paragraphs. */
export function sanitise(markup: string): ReactNode[] {
  if (!/<[a-zA-Z]/.test(markup)) return paragraphs(markup);
  const doc = new DOMParser().parseFromString(`<div>${markup}</div>`, "text/html");
  const root = doc.body.firstElementChild;
  if (!root) return [];
  const out: ReactNode[] = [];
  let stray: ReactNode[] = [];
  let key = 0;
  const flush = () => {
    if (stray.length > 0) {
      out.push(createElement("p", { key: key++ }, ...stray));
      stray = [];
    }
  };
  for (const child of Array.from(root.childNodes)) {
    const isBlock = child.nodeType === Node.ELEMENT_NODE && ["p", "ul", "ol"].includes((child as Element).localName);
    if (isBlock) {
      flush();
      out.push(rebuild(child, key++));
    } else {
      const built = rebuild(child, key++);
      if (typeof built === "string" ? built.trim() : built) stray.push(built);
    }
  }
  flush();
  return out;
}

/** A package's description, sanitised to p, ul, ol, li, em, code, strong and br. Never raw HTML. */
export function Description({ markup, className }: { markup: string | null | undefined; className?: string }) {
  const nodes = useMemo(() => (markup ? sanitise(markup) : []), [markup]);
  if (nodes.length === 0) return null;
  return <div className={className ? `bs-desc ${className}` : "bs-desc"}>{nodes}</div>;
}
