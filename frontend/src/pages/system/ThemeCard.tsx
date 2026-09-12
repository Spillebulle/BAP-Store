// A theme card (§7.15, §9): a preview of the theme it names, whatever theme
// is on, and a caption. Selected is the 2 px accent border and "in use".

import type { Theme } from "../../types";

interface Props {
  theme: Theme;
  name: string;
  on: boolean;
  onPick: () => void;
}

function Half({ kind }: { kind: "dark" | "light" }) {
  return (
    <div className={`bs-theme-prev-half bs-theme-prev--${kind}`}>
      <div className="bs-theme-prev-side" />
      <div className="bs-theme-prev-rows">
        <i className="mark" />
        <i style={{ width: "70%" }} />
        <i style={{ width: "45%" }} />
        <i style={{ width: "60%" }} />
      </div>
    </div>
  );
}

export function ThemeCard({ theme, name, on, onPick }: Props) {
  const title = on ? `${name} is in use.` : `Switch to the ${name.toLowerCase()} theme.`;
  return (
    <button type="button" className={on ? "bs-card on" : "bs-card"} onClick={onPick} aria-pressed={on} title={title}>
      <div className="bs-card-prev">
        <div className="bs-theme-prev">
          {theme === "system" ? (
            <>
              <Half kind="dark" />
              <Half kind="light" />
            </>
          ) : (
            <Half kind={theme} />
          )}
        </div>
      </div>
      <div className="bs-card-cap">
        <span>{name}</span>
        {on ? <small>in use</small> : null}
      </div>
    </button>
  );
}
