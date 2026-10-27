export type TagCount = { tag: string; count: number; category: string; category_photos: number };
export type TagGroup = { category: string; photos: number; tags: TagCount[] };

/** "city street" -> "City street". */
export const tagLabel = (tag: string): string => (tag ? tag[0].toUpperCase() + tag.slice(1) : tag);

/** The Auto tags tree: tags grouped by category, categories in the order the list gives them (the backend sorts them),
 *  tags inside a category in list order. */
export function tagTree(tags: TagCount[]): TagGroup[] {
  const groups: TagGroup[] = [];
  for (const t of tags) {
    let g = groups.find((x) => x.category === t.category);
    if (!g) groups.push((g = { category: t.category, photos: t.category_photos, tags: [] }));
    g.tags.push(t);
  }
  return groups;
}

/** The category a tag is listed under, if it is in the list. */
export const categoryOf = (tags: TagCount[], tag: string | null): string | null => (tag === null ? null : (tags.find((t) => t.tag === tag)?.category ?? null));

/** The listing is restricted to hits when a tag, a person or a camera is applied. */
export const hitsActive = (tag: string | null, person: number | null = null, camera: string | null = null): boolean => tag !== null || person !== null || camera !== null;
