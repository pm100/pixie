/** Selected photos by index in the current view. Cleared by the caller when the view changes. */
export interface Selection {
  picked: ReadonlySet<number>;
  anchor: number | null;
}

export const emptySelection: Selection = { picked: new Set(), anchor: null };

export function click(sel: Selection, index: number, mods: { ctrl: boolean; shift: boolean }): Selection {
  if (mods.shift && sel.anchor !== null) {
    const [a, b] = sel.anchor < index ? [sel.anchor, index] : [index, sel.anchor];
    const next = new Set<number>(mods.ctrl ? sel.picked : []);
    for (let i = a; i <= b; i++) next.add(i);
    return { picked: next, anchor: sel.anchor };
  }
  if (mods.ctrl) {
    const next = new Set(sel.picked);
    if (!next.delete(index)) next.add(index);
    return { picked: next, anchor: index };
  }
  return { picked: new Set([index]), anchor: index };
}

/** The picked index when exactly one photo is selected. */
export function single(sel: Selection): number | null {
  return sel.picked.size === 1 ? [...sel.picked][0] : null;
}
