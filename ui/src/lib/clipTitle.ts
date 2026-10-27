/** The tooltip for text that may be cut off with "…": its full text when it is cut off, nothing when all of it shows. */
export const titleFor = (scrollWidth: number, clientWidth: number, text: string): string => (scrollWidth > clientWidth + 1 ? text.trim() : '');

/** A Svelte action for an element whose text is cut off with an ellipsis: when the mouse comes over it, it gets a tooltip with
 *  the full text if (and only if) some of it is hidden, so the answer is right however wide the pane has been dragged. */
export function clipTitle(node: HTMLElement) {
  const update = () => {
    const t = titleFor(node.scrollWidth, node.clientWidth, node.textContent ?? '');
    if (t) node.title = t;
    else node.removeAttribute('title');
  };
  node.addEventListener('mouseenter', update);
  return { destroy: () => node.removeEventListener('mouseenter', update) };
}
