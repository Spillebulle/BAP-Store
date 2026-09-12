// The three sizes an interface icon comes in (§11): 16 in rows and buttons at
// stroke 1.5, 20 in panel headers at 1.75, 24 in empty states. Spread one of
// these onto a lucide-react component; never a fourth size.

export const ICON = { size: 16, strokeWidth: 1.5 } as const;
export const ICON_LG = { size: 20, strokeWidth: 1.75 } as const;
export const ICON_EMPTY = { size: 24, strokeWidth: 1.5 } as const;
/** The mark inside a badge or a chip: smaller than a row icon, heavier so it survives. */
export const ICON_MARK = { size: 10, strokeWidth: 2.5 } as const;
/** The chevron and the check inside a dropdown. */
export const ICON_SM = { size: 12, strokeWidth: 1.5 } as const;
