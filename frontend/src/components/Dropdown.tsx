import { Check, ChevronDown, Search } from "lucide-react";
import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { CheckMark } from "./Checkbox";
import { Figure } from "./Figure";
import { ICON, ICON_SM } from "./icons";

export interface DropdownOption<T extends string = string> {
  value: T;
  label: string;
  /** A count or a version, in mono at the right of the row. */
  figure?: string;
  /** A word or two in dim after the label ("not installed"): a state the row is in, not a reason it is off. */
  hint?: string;
  icon?: ReactNode;
  disabled?: boolean;
  /** Shown in the row's tooltip: why it cannot be chosen ("snapd is not installed."). */
  disabledReason?: string;
  /** Rows with the same group sit under one eyebrow. */
  group?: string;
}

interface Common<T extends string> {
  /** The control's own name: the trigger's label when nothing is chosen, and the accessible name. */
  name: string;
  options: DropdownOption<T>[];
  /** Stands alone on a line: takes a border so it reads as a control. Its list is at least its width, and wider when its rows need it. */
  alone?: boolean;
  /** In a form or settings row: 26 px tall at control size, with a list of exactly its width. */
  form?: boolean;
  /** Fill the container. */
  full?: boolean;
  /** Right-aligned in a strip: the list aligns its right edge to the trigger's. */
  align?: "left" | "right";
  icon?: ReactNode;
  /** A search field pinned at the top. Defaults to on past ten options. */
  searchable?: boolean;
  disabled?: boolean;
  disabledReason?: string;
  className?: string;
}

export type DropdownProps<T extends string> = Common<T> & {
  value: T | null;
  onChange: (value: T) => void;
};

export type MultiSelectProps<T extends string> = Common<T> & {
  values: T[];
  onChange: (values: T[]) => void;
};

// The list is at least as wide as the trigger and, for a bare in-row trigger,
// no wider than this, so long labels are not clipped and a word does not grow
// a menu across the page.
const BARE_MAX_WIDTH = 240;
const GAP = 4;
const EDGE = 8;

interface Placed {
  style: CSSProperties;
  above: boolean;
}

/**
 * Where the list goes: 4 px below the trigger with left edges aligned (right
 * edges for a right-aligned trigger), flipped above when there is no room
 * below, and never clipped by a container because it lives on the body.
 */
function usePlacement(
  open: boolean,
  trigger: React.RefObject<HTMLButtonElement | null>,
  menu: React.RefObject<HTMLDivElement | null>,
  exact: boolean,
  align: "left" | "right",
): Placed | null {
  const [placed, setPlaced] = useState<Placed | null>(null);

  useLayoutEffect(() => {
    if (!open) {
      setPlaced(null);
      return;
    }
    const t = trigger.current;
    const m = menu.current;
    if (!t || !m) return;
    const rect = t.getBoundingClientRect();
    const height = m.offsetHeight;
    const width = exact ? rect.width : Math.max(rect.width, Math.min(m.offsetWidth, BARE_MAX_WIDTH));
    const roomBelow = window.innerHeight - rect.bottom - GAP - EDGE;
    const roomAbove = rect.top - GAP - EDGE;
    const above = height > roomBelow && roomAbove > roomBelow;
    const style: CSSProperties = exact ? { width } : { minWidth: rect.width, maxWidth: Math.max(rect.width, BARE_MAX_WIDTH) };
    if (align === "right") {
      style.right = Math.max(EDGE, window.innerWidth - rect.right);
    } else {
      style.left = Math.max(EDGE, Math.min(rect.left, window.innerWidth - width - EDGE));
    }
    if (above) {
      style.bottom = window.innerHeight - rect.top + GAP;
      style.maxHeight = roomAbove;
    } else {
      style.top = rect.bottom + GAP;
      style.maxHeight = roomBelow;
    }
    setPlaced({ style, above });
  }, [open, trigger, menu, exact, align]);

  return placed;
}

function useDismiss(open: boolean, close: () => void, refs: React.RefObject<HTMLElement | null>[]) {
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node;
      if (refs.some((r) => r.current?.contains(target))) return;
      close();
    };
    // A scroll anywhere but inside the list moves the trigger away from the
    // list, so the list closes rather than floating where the trigger was.
    const onScroll = (e: Event) => {
      if (refs.some((r) => r.current?.contains(e.target as Node))) return;
      close();
    };
    document.addEventListener("pointerdown", onPointer, true);
    window.addEventListener("resize", close);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("resize", close);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [open, close, refs]);
}

interface PickerProps<T extends string> extends Common<T> {
  multi: boolean;
  chosen: T[];
  /** The trigger's text. */
  summary: ReactNode;
  empty: boolean;
  pick: (value: T) => void;
  pickAll?: (values: T[]) => void;
  clearAll?: (values: T[]) => void;
}

function Picker<T extends string>(props: PickerProps<T>) {
  const { name, options, alone, form, full, align = "left", icon, disabled, disabledReason, className, multi, chosen, summary, empty, pick, pickAll, clearAll } = props;
  const searchable = props.searchable ?? options.length > 10;
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [focus, setFocus] = useState(0);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const typed = useRef({ text: "", at: 0 });
  const id = useId();

  // A form or full-width control's list matches it exactly; any other trigger,
  // bordered or not, can be a short word ("All") over rows that are not.
  const exact = Boolean(full || form);
  const placed = usePlacement(open, triggerRef, menuRef, exact, align);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => o.label.toLowerCase().includes(q)) : options;
  }, [options, query]);

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
  }, []);
  const closeAndFocus = useCallback(() => {
    close();
    triggerRef.current?.focus();
  }, [close]);

  useDismiss(open, close, useMemo(() => [triggerRef, menuRef], []));

  const openAt = (index: number) => {
    setFocus(Math.max(0, index));
    setOpen(true);
  };

  useEffect(() => {
    if (!open) return;
    if (searchable) searchRef.current?.focus();
    else listRef.current?.focus();
  }, [open, searchable]);

  useEffect(() => {
    if (!open) return;
    const el = listRef.current?.querySelector<HTMLElement>(`[data-index="${focus}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [focus, open]);

  const move = (delta: number) => {
    if (visible.length === 0) return;
    let next = focus;
    for (let n = 0; n < visible.length; n += 1) {
      next = (next + delta + visible.length) % visible.length;
      if (!visible[next].disabled) break;
    }
    setFocus(next);
  };

  const choose = (index: number) => {
    const o = visible[index];
    if (!o || o.disabled) return;
    pick(o.value);
    if (!multi) closeAndFocus();
  };

  const onTriggerKey = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      const current = options.findIndex((o) => chosen.includes(o.value));
      openAt(current);
    }
  };

  const onMenuKey = (e: KeyboardEvent<HTMLDivElement>) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Home":
        e.preventDefault();
        setFocus(0);
        break;
      case "End":
        e.preventDefault();
        setFocus(visible.length - 1);
        break;
      case "Enter":
        e.preventDefault();
        choose(focus);
        break;
      case " ":
        if (searchable && e.target === searchRef.current) return;
        e.preventDefault();
        choose(focus);
        break;
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        closeAndFocus();
        break;
      case "Tab":
        close();
        break;
      default: {
        if (searchable || e.key.length !== 1 || e.ctrlKey || e.metaKey || e.altKey) return;
        // Typing jumps: letters typed within half a second run together.
        const now = Date.now();
        const text = now - typed.current.at < 500 ? typed.current.text + e.key : e.key;
        typed.current = { text: text.toLowerCase(), at: now };
        const hit = visible.findIndex((o) => !o.disabled && o.label.toLowerCase().startsWith(typed.current.text));
        if (hit >= 0) setFocus(hit);
      }
    }
  };

  useEffect(() => setFocus(0), [query]);

  const triggerClasses = [
    "bs-dd",
    alone ? "bs-dd--alone" : "",
    form ? "bs-dd--form" : "",
    full ? "bs-dd--full" : "",
    open ? "open" : "",
    className ?? "",
  ]
    .filter(Boolean)
    .join(" ");

  const allChecked = multi && visible.length > 0 && visible.every((o) => chosen.includes(o.value) || o.disabled);
  const someChecked = multi && visible.some((o) => chosen.includes(o.value));
  const allState = allChecked ? true : someChecked ? "mixed" : false;

  let lastGroup: string | undefined;

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className={triggerClasses}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={name}
        title={disabled ? disabledReason : undefined}
        disabled={disabled}
        onClick={() => (open ? close() : openAt(options.findIndex((o) => chosen.includes(o.value))))}
        onKeyDown={onTriggerKey}
      >
        {icon ? <span className="bs-dd-icon bs-inline">{icon}</span> : null}
        <span className={empty ? "bs-dd-value empty" : "bs-dd-value"}>{summary}</span>
        <ChevronDown className="bs-dd-chev" {...ICON_SM} aria-hidden="true" />
      </button>
      {open
        ? createPortal(
            <div
              ref={menuRef}
              id={id}
              className={placed?.above ? "bs-menu bs-menu--above" : "bs-menu"}
              style={placed ? placed.style : { visibility: "hidden", left: 0, top: 0 }}
              onKeyDown={onMenuKey}
            >
              {searchable ? (
                <div className="bs-menu-head">
                  <div className="bs-field">
                    <Search {...ICON} aria-hidden="true" />
                    <input
                      ref={searchRef}
                      value={query}
                      placeholder={`Search ${name.toLowerCase()}`}
                      aria-label={`Search ${name.toLowerCase()}`}
                      spellCheck={false}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </div>
                </div>
              ) : null}
              <div
                ref={listRef}
                className="bs-menu-list"
                role="listbox"
                aria-label={name}
                aria-multiselectable={multi || undefined}
                aria-activedescendant={visible[focus] ? `${id}-${focus}` : undefined}
                tabIndex={-1}
              >
                {multi && visible.length > 0 ? (
                  <>
                    <div
                      role="option"
                      aria-selected={allChecked}
                      className="bs-menu-item"
                      onClick={() => (allChecked ? clearAll?.(visible.map((o) => o.value)) : pickAll?.(visible.filter((o) => !o.disabled).map((o) => o.value)))}
                    >
                      <CheckMark checked={allState} />
                      <span className="bs-menu-item-label">All</span>
                    </div>
                    <div className="bs-menu-sep" />
                  </>
                ) : null}
                {visible.length === 0 ? <div className="bs-menu-empty">Nothing matches.</div> : null}
                {visible.map((o, i) => {
                  const group = o.group !== lastGroup && o.group ? o.group : null;
                  lastGroup = o.group;
                  const current = chosen.includes(o.value);
                  const cls = [
                    "bs-menu-item",
                    !multi && current ? "cur" : "",
                    i === focus ? "focus" : "",
                    o.disabled ? "off" : "",
                  ]
                    .filter(Boolean)
                    .join(" ");
                  return (
                    <div key={o.value}>
                      {group ? (
                        <>
                          {i > 0 ? <div className="bs-menu-sep" /> : null}
                          <div className="bs-eyebrow bs-menu-eyebrow">{group}</div>
                        </>
                      ) : null}
                      <div
                        id={`${id}-${i}`}
                        role="option"
                        aria-selected={current}
                        aria-disabled={o.disabled || undefined}
                        data-index={i}
                        className={cls}
                        title={o.disabled ? o.disabledReason : undefined}
                        onMouseEnter={() => setFocus(i)}
                        onClick={() => choose(i)}
                      >
                        {multi ? <CheckMark checked={current} /> : null}
                        {o.icon ? <span className="bs-menu-item-icon bs-inline">{o.icon}</span> : null}
                        <span className="bs-menu-item-label">{o.label}</span>
                        {o.hint ? <span className="bs-menu-item-hint">{o.hint}</span> : null}
                        {o.figure ? <Figure className="bs-menu-item-figure">{o.figure}</Figure> : null}
                        {!multi && current ? <Check className="bs-menu-item-check" {...ICON_SM} aria-hidden="true" /> : null}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>,
            document.body,
          )
        : null}
    </>
  );
}

/** One choice (§7.7). The list matches the box; the current item wears the selected-row look. */
export function Dropdown<T extends string>(props: DropdownProps<T>) {
  const { value, onChange, ...rest } = props;
  const current = props.options.find((o) => o.value === value);
  return (
    <Picker<T>
      {...rest}
      multi={false}
      chosen={current ? [current.value] : []}
      summary={current ? current.label : props.name}
      empty={!current}
      pick={onChange}
    />
  );
}

/** Several choices: a checkbox on every row, an "All" row above, and the list stays open. Changes apply live. */
export function MultiSelect<T extends string>(props: MultiSelectProps<T>) {
  const { values, onChange, ...rest } = props;
  const chosen = props.options.filter((o) => values.includes(o.value));
  const selectable = props.options.filter((o) => !o.disabled).length;
  let summary: ReactNode;
  if (chosen.length === 0) summary = props.name;
  else if (chosen.length === selectable && selectable > 0) summary = "All";
  else if (chosen.length === 1) summary = chosen[0].label;
  else if (chosen.length === 2) summary = `${chosen[0].label}, ${chosen[1].label}`;
  else
    summary = (
      <>
        <Figure>{chosen.length}</Figure> selected
      </>
    );
  const toggle = (value: T) => onChange(values.includes(value) ? values.filter((v) => v !== value) : [...values, value]);
  return (
    <Picker<T>
      {...rest}
      multi
      chosen={values}
      summary={summary}
      empty={chosen.length === 0}
      pick={toggle}
      pickAll={(vs) => onChange([...new Set([...values, ...vs])])}
      clearAll={(vs) => onChange(values.filter((v) => !vs.includes(v)))}
    />
  );
}
