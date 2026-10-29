import type { SettingsPage } from './Settings.svelte';
import type { TabId } from './tabs';

/** The places in the app Ask Pixie may offer a button for (the same list as the proxy's `go_to` tool; a test compares them). */
export const GO_TO_TARGETS = [
  'browse', 'people', 'duplicates', 'tools-details', 'tools-email', 'tools-print', 'tools-tags',
  'settings-folders', 'settings-viewing', 'settings-browsing', 'settings-trips', 'settings-analysis', 'settings-email', 'settings-about',
] as const;
export type GoToTarget = (typeof GO_TO_TARGETS)[number];

export type ToolsPage = 'details' | 'email' | 'print' | 'tags';
export interface Place { tab: TabId; settings?: SettingsPage; tools?: ToolsPage }

const isTarget = (t: string): t is GoToTarget => (GO_TO_TARGETS as readonly string[]).includes(t);

/** Where a target leads, or null for anything that is not on the list (the model may say anything). */
export function placeFor(target: string): Place | null {
  if (!isTarget(target)) return null;
  if (target === 'browse') return { tab: 'browse' };
  if (target === 'people') return { tab: 'faces' };
  if (target === 'duplicates') return { tab: 'dupes' };
  if (target.startsWith('tools-')) return { tab: 'tools', tools: target.slice(6) as ToolsPage };
  return { tab: 'settings', settings: target.slice(9) as SettingsPage };
}

const SETTINGS_NAMES: Record<string, string> = {
  folders: 'Photo folders', viewing: 'Viewing', browsing: 'Browsing', trips: 'Trips', analysis: 'Picture analysis', email: 'Email', about: 'About',
};
const TOOLS_NAMES: Record<string, string> = { details: 'Photo details', email: 'Email', print: 'Print', tags: 'Tags' };

/** The button's words, for example "Open Email in Tools". */
export function goToLabel(target: string): string {
  if (target === 'browse') return 'Open Browse';
  if (target === 'people') return 'Open People';
  if (target === 'duplicates') return 'Open Duplicates';
  if (target.startsWith('tools-')) return `Open ${TOOLS_NAMES[target.slice(6)] ?? 'Tools'} in Tools`;
  if (target.startsWith('settings-')) return `Open ${SETTINGS_NAMES[target.slice(9)] ?? 'Settings'} in Settings`;
  return 'Open';
}
