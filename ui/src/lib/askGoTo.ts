import type { SettingsPage } from './Settings.svelte';
import type { TabId } from './tabs';
import { GROUPS, TOOLS } from './tools';

/** The places in the app Ask Pixie may offer a button for. They go to the proxy with every question (the `go_to` tool offers exactly these),
 *  and askGuide.test.ts checks the guide against them. A tool is named by its tab and its id: `send-email`, `cleanup-missing-dates`. */
export const GO_TO_TARGETS = [
  'browse', 'people',
  'send-email', 'send-print', 'create-collage', 'create-mosaic', 'organize-tags', 'cleanup-duplicates', 'cleanup-missing-dates', 'transfer-import', 'transfer-sync',
  'settings-folders', 'settings-viewing', 'settings-browsing', 'settings-trips', 'settings-analysis', 'settings-email', 'settings-ask', 'settings-about',
] as const;
export type GoToTarget = (typeof GO_TO_TARGETS)[number];

export interface Place { tab: TabId; settings?: SettingsPage; tool?: string }

const isTarget = (t: string): t is GoToTarget => (GO_TO_TARGETS as readonly string[]).includes(t);

/** The tool a `<tab>-<tool>` target names, if there is one. */
function toolOf(target: string) {
  const dash = target.indexOf('-');
  const group = target.slice(0, dash), id = target.slice(dash + 1);
  return TOOLS.find((t) => t.group === group && t.id === id);
}

/** Where a target leads, or null for anything that is not on the list (the model may say anything). */
export function placeFor(target: string): Place | null {
  if (!isTarget(target)) return null;
  if (target === 'browse') return { tab: 'browse' };
  if (target === 'people') return { tab: 'faces' };
  if (target.startsWith('settings-')) return { tab: 'settings', settings: target.slice(9) as SettingsPage };
  const tool = toolOf(target);
  return tool ? { tab: tool.group, tool: tool.id } : null;
}

const SETTINGS_NAMES: Record<string, string> = {
  folders: 'Photo folders', viewing: 'Viewing', browsing: 'Browsing', trips: 'Trips', analysis: 'Picture analysis', email: 'Email', ask: 'Ask Pixie', about: 'About',
};

/** The button's words, for example "Open Email in Send". */
export function goToLabel(target: string): string {
  if (target === 'browse') return 'Open Browse';
  if (target === 'people') return 'Open People';
  if (target.startsWith('settings-')) return `Open ${SETTINGS_NAMES[target.slice(9)] ?? 'Settings'} in Settings`;
  const tool = toolOf(target);
  return tool ? `Open ${tool.label} in ${GROUPS.find((g) => g.id === tool.group)?.label ?? 'Pixie'}` : 'Open';
}
