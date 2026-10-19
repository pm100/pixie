export type TagCount = { tag: string; count: number };

/** "city street" -> "City street". */
export const tagLabel = (tag: string): string => (tag ? tag[0].toUpperCase() + tag.slice(1) : tag);

/** The rows the Tags list shows: the most common `limit` (or all when expanded), plus the chosen tag even when it is rare. */
export function tagRows(tags: TagCount[], chosen: string | null, expanded: boolean, limit: number): { rows: TagCount[]; hidden: number } {
  if (expanded || tags.length <= limit) return { rows: tags, hidden: 0 };
  let rows = tags.slice(0, limit);
  const pick = chosen === null ? undefined : tags.find((t) => t.tag === chosen);
  if (pick && !rows.includes(pick)) rows = [...rows.slice(0, limit - 1), pick];
  return { rows, hidden: tags.length - limit };
}

/** The listing is restricted to hits when a text search or a tag is applied. */
export const searchActive = (searchTerm: string | null, tag: string | null): boolean => searchTerm !== null || tag !== null;
