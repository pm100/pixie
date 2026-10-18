/** The text of the selection bar; empty when nothing is selected. */
export const selectionLabel = (n: number): string => (n > 0 ? `${n.toLocaleString('en-US')} selected` : '');
