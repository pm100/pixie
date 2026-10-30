/** The top-level sections of the Browse list. */
export type SectionId = 'library' | 'people' | 'trips' | 'mytags' | 'tags' | 'cameras' | 'when' | 'folders' | 'missing';
export const SECTION_IDS: SectionId[] = ['library', 'people', 'trips', 'mytags', 'tags', 'cameras', 'when', 'folders', 'missing'];

/** The collapsed sections after the user clicks the heading of `id`. */
export function toggleSection(collapsed: SectionId[], id: SectionId): SectionId[] {
  return collapsed.includes(id) ? collapsed.filter((s) => s !== id) : [...collapsed, id];
}

/** Whatever was saved, reduced to the sections that exist (each once). Nothing saved means everything is open. */
export function cleanCollapsed(raw: unknown): SectionId[] {
  if (!Array.isArray(raw)) return [];
  return SECTION_IDS.filter((id) => raw.includes(id));
}
