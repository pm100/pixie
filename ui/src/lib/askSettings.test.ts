import { writeFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { DEFAULT_TRIP_PREFS } from './trips';
import { ALL_ON } from './analysisOptions';
import { applyChanges, checkProposal, settingsSchema, type SettingValues } from './askSettings';

const values: SettingValues = {
  viewerMode: 'window',
  emailLimitMb: 20,
  recentDays: 30,
  networkRescan: { on: true, minutes: 30 },
  analysis: { ...ALL_ON },
  trips: { ...DEFAULT_TRIP_PREFS },
};

const find = (key: string) => settingsSchema(values).find((s) => s.key === key)!;

describe('settingsSchema', () => {
  it('describes every setting with a unique key, a label and a description', () => {
    const schema = settingsSchema(values);
    expect(new Set(schema.map((s) => s.key)).size).toBe(schema.length);
    for (const s of schema) {
      expect(s.label.length).toBeGreaterThan(0);
      expect(s.description.length).toBeGreaterThan(0);
    }
    expect(schema.length).toBeLessThanOrEqual(40);
  });

  it('carries the current values, as numbers or booleans with a readable label', () => {
    expect(find('analysis_faces')).toMatchObject({ type: 'bool', current: true, currentLabel: 'On' });
    expect(find('email_limit_mb')).toMatchObject({ type: 'int', current: 20, currentLabel: '20 MB', min: 1, max: 500 });
    expect(find('viewer_mode')).toMatchObject({ type: 'choice', current: 0, currentLabel: 'Its own window' });
    expect(find('network_rescan_minutes')).toMatchObject({ type: 'choice', current: 30, currentLabel: '30 minutes' });
  });

  it('leaves out where home is: that is the user\'s to confirm on the map, not something to say in words', () => {
    expect(settingsSchema(values).some((s) => s.key.includes('home'))).toBe(false);
  });

  it('writes the contract fixture the proxy tests read, when asked', () => {
    if (process.env.WRITE_ASK_FIXTURE !== '1') return;
    const context = { platform: 'windows', language: 'en-US', tab: 'ask', today: '2026-10-28', settings: settingsSchema(values) };
    writeFileSync(new URL('../../../ask-proxy/test/fixtures/context.json', import.meta.url), `${JSON.stringify(context, null, 2)}\n`);
  });
});

describe('checkProposal', () => {
  it('turns a valid change into a card line with before and after', () => {
    const r = checkProposal([{ setting: 'analysis_faces', value: false }], values);
    expect(r.changes).toEqual([{ key: 'analysis_faces', label: 'Find faces', value: false, from: 'On', to: 'Off' }]);
    expect(r.dropped).toBe(0);
  });

  it('drops unknown settings, wrong types, out-of-range values and options that do not exist', () => {
    const r = checkProposal([
      { setting: 'delete_everything', value: true },
      { setting: 'analysis_faces', value: 'yes' },
      { setting: 'email_limit_mb', value: 0 },
      { setting: 'email_limit_mb', value: 501 },
      { setting: 'email_limit_mb', value: 12.5 },
      { setting: 'viewer_mode', value: 7 },
      { setting: 'trip_min_nights', value: -1 },
      { setting: 'recent_days', value: Number.NaN },
    ], values);
    expect(r.changes).toEqual([]);
    expect(r.dropped).toBe(8);
  });

  it('drops a change to the value the setting already has', () => {
    const r = checkProposal([{ setting: 'email_limit_mb', value: 20 }, { setting: 'analysis_tags', value: true }], values);
    expect(r.changes).toEqual([]);
    expect(r.dropped).toBe(2);
  });

  it('keeps the last of two proposals for one setting', () => {
    const r = checkProposal([{ setting: 'recent_days', value: 7 }, { setting: 'recent_days', value: 14 }], values);
    expect(r.changes.map((c) => c.value)).toEqual([14]);
  });

  it('accepts choices by their numeric value and names them in the card', () => {
    const r = checkProposal([{ setting: 'viewer_mode', value: 1 }, { setting: 'network_rescan_minutes', value: 5 }], values);
    expect(r.changes.map((c) => [c.to])).toEqual([['A pop-out'], ['5 minutes']]);
  });
});

describe('applyChanges', () => {
  function recorder() {
    const log: unknown[] = [];
    return {
      log,
      handlers: {
        viewer: (m: unknown) => log.push(['viewer', m]),
        emailLimit: (mb: number) => log.push(['email', mb]),
        recentDays: (n: number) => log.push(['recent', n]),
        networkRescan: (r: unknown) => log.push(['rescan', r]),
        analysis: (o: unknown) => log.push(['analysis', o]),
        trips: (p: unknown) => log.push(['trips', p]),
      },
    };
  }

  it('sends each group of settings once, with the other settings in that group left as they were', () => {
    const { log, handlers } = recorder();
    const { changes } = checkProposal([
      { setting: 'analysis_faces', value: false },
      { setting: 'analysis_dupes', value: false },
      { setting: 'network_rescan_minutes', value: 60 },
      { setting: 'email_limit_mb', value: 25 },
      { setting: 'trip_min_nights', value: 2 },
      { setting: 'viewer_mode', value: 1 },
      { setting: 'recent_days', value: 7 },
    ], values);
    applyChanges(changes, values, handlers);
    expect(log).toEqual([
      ['viewer', 'popout'],
      ['email', 25],
      ['recent', 7],
      ['rescan', { on: true, minutes: 60 }],
      ['analysis', { tags: true, faces: false, dupes: false }],
      ['trips', { ...DEFAULT_TRIP_PREFS, minNights: 2 }],
    ]);
  });

  it('does nothing for no changes', () => {
    const { log, handlers } = recorder();
    applyChanges([], values, handlers);
    expect(log).toEqual([]);
  });
});
