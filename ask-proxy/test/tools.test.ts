import { describe, expect, it } from 'vitest';
import { interpret as interpretFor, toolSpecs } from '../src/tools';
import type { ToolCall } from '../src/types';

const PLACES = ['browse', 'send-email', 'send-print'];
const TOOL_SPECS = toolSpecs(PLACES);
const interpret = (texts: string[], calls: ToolCall[]) => interpretFor(texts, calls, PLACES);

const propose = (changes: unknown) => ({ name: 'propose_settings_changes', input: { changes } });
const feedback = (category: unknown, summary: unknown) => ({ name: 'submit_feedback', input: { category, summary } });

describe('tool specs', () => {
  it('declares the tools with required fields, as plain JSON Schema', () => {
    expect(TOOL_SPECS.map((t) => t.name)).toEqual(['propose_settings_changes', 'go_to', 'submit_feedback']);
    expect(TOOL_SPECS[0]?.schema.required).toEqual(['changes', 'say']);
    expect(TOOL_SPECS[1]?.schema.required).toEqual(['target', 'say']);
    expect(TOOL_SPECS[2]?.schema.required).toEqual(['category', 'summary', 'say']);
  });

  it('offers the places the app says it has, and no others', () => {
    const target = (places: string[]) => (toolSpecs(places)[1]?.schema.properties as any).target.enum;
    expect(target(PLACES)).toEqual(PLACES);
    expect(target(['browse', 'create-collage'])).toEqual(['browse', 'create-collage']);
  });

  it('leaves the change value untyped, because providers disagree on unions', () => {
    const items = ((TOOL_SPECS[0]?.schema.properties as any).changes.items.properties) as Record<string, any>;
    expect(items.value.type).toBeUndefined();
    expect(items.value.description).toContain('number');
  });
});

describe('interpret, go_to', () => {
  const goTo = (target: unknown, say?: unknown) => ({ name: 'go_to', input: { target, say } });

  it('takes the first valid target and ignores one that is not on the list', () => {
    expect(interpret([''], [goTo('nowhere'), goTo('send-email'), goTo('browse')]).goTo).toBe('send-email');
    expect(interpret(['x'], [goTo('rm -rf')]).goTo).toBeNull();
  });

  it('takes a place only if this version of the app listed it', () => {
    const call = [{ name: 'go_to', input: { target: 'create-collage', say: 'Here.' } }];
    expect(interpretFor([''], call, PLACES).goTo).toBeNull();
    expect(interpretFor([''], call, [...PLACES, 'create-collage']).goTo).toBe('create-collage');
  });

  it('uses the sentence given with the button when the model wrote no text of its own', () => {
    expect(interpret([], [goTo('send-email', 'Choose the video, then press Email.')]).reply).toBe('Choose the video, then press Email.');
  });

  it('uses the sentence in the call when it says more than the lead-in the model wrote beside it', () => {
    expect(interpret(['There are two ways:'], [goTo('browse', 'There are two ways: pick them, or tag them and work on the tag.')]).reply).toBe('There are two ways: pick them, or tag them and work on the tag.');
  });

  it('keeps the model own text when it wrote some', () => {
    expect(interpret(['Here you go.'], [goTo('browse', 'Other words.')]).reply).toBe('Here you go.');
  });

  it('asks for the sentence in the tool, so the model cannot leave it out', () => {
    expect(TOOL_SPECS.find((t) => t.name === 'go_to')?.schema.required).toEqual(['target', 'say']);
  });
});

describe('interpret, propose_settings_changes with its own words', () => {
  const proposeSaying = (say: unknown) => ({ name: 'propose_settings_changes', input: { changes: [{ setting: 'analysis_faces', value: false }], say } });

  it('shows the words that came with the proposal when the model wrote no text of its own', () => {
    const result = interpret([], [proposeSaying('The People checkbox in Settings, Picture analysis, stops face finding.')]);
    expect(result.reply).toBe('The People checkbox in Settings, Picture analysis, stops face finding.');
    expect(result.proposal).toEqual([{ setting: 'analysis_faces', value: false }]);
  });

  it('shows the fuller of the two when the model also wrote a lead-in', () => {
    expect(interpret(['Here:'], [proposeSaying('Turn off the People checkbox in Settings, Picture analysis.')]).reply).toBe('Turn off the People checkbox in Settings, Picture analysis.');
  });

  it('ignores words that come with nothing to propose', () => {
    expect(interpret([], [{ name: 'propose_settings_changes', input: { changes: [], say: 'Done.' } }]).reply).toBe('');
  });
});

describe('interpret, submit_feedback with its own words', () => {
  const draft = (say: unknown) => ({ name: 'submit_feedback', input: { category: 'feature_request', summary: 'Wants dark mode.', say } });

  it('shows what Pixie cannot do when the model wrote no text beside the call', () => {
    const result = interpret([], [draft('Pixie has no dark mode yet.')]);
    expect(result.reply).toBe('Pixie has no dark mode yet.');
    expect(result.feedback).toEqual([{ category: 'feature_request', summary: 'Wants dark mode.' }]);
  });

  it('keeps the words the model wrote itself when they say more', () => {
    expect(interpret(['Pixie has no dark mode yet, but you can change your Windows theme.'], [draft('No dark mode.')]).reply).toBe('Pixie has no dark mode yet, but you can change your Windows theme.');
  });

  it('shows nothing of its own when the call has no words, so the app can say something plain', () => {
    expect(interpret([], [{ name: 'submit_feedback', input: { category: 'bug', summary: 'x' } }]).reply).toBe('');
  });

  it('does not use the words of a note that was turned away', () => {
    expect(interpret([], [{ name: 'submit_feedback', input: { category: 'praise', summary: 'x', say: 'Hello.' } }]).reply).toBe('');
  });
});

describe('interpret', () => {
  it('returns plain text with no proposal', () => {
    expect(interpret(['Try a slower tempo.'], [])).toEqual({ reply: 'Try a slower tempo.', proposal: null, goTo: null, feedback: [] });
  });

  it('joins several text blocks and ignores unknown tools', () => {
    expect(interpret(['One.', 'Two.'], [{ name: 'rm_rf', input: { path: '/' } }])).toEqual({
      reply: 'One.\nTwo.',
      proposal: null,
      goTo: null,
      feedback: [],
    });
  });

  it('reads a settings proposal from the tool call, alongside text', () => {
    const result = interpret(
      ['Give yourself more tries.'],
      [propose([{ setting: 'noteRetries', value: 5 }, { setting: 'showTestNotes', value: true }])],
    );
    expect(result.reply).toBe('Give yourself more tries.');
    expect(result.proposal).toEqual([
      { setting: 'noteRetries', value: 5 },
      { setting: 'showTestNotes', value: true },
    ]);
  });

  it('merges proposals from several calls in order', () => {
    const result = interpret([], [propose([{ setting: 'a', value: 1 }]), propose([{ setting: 'b', value: 2 }])]);
    expect(result.proposal?.map((p) => p.setting)).toEqual(['a', 'b']);
  });

  it('accepts numbers and booleans sent as text, because some models do that', () => {
    const result = interpret([], [propose([
      { setting: 'noteRetries', value: '5' },
      { setting: 'tempoBpm', value: ' 120 ' },
      { setting: 'showTestNotes', value: 'TRUE' },
      { setting: 'octaveCorrection', value: 'false' },
      { setting: 'yinThreshold', value: '0.2' },
    ])]);
    expect(result.proposal).toEqual([
      { setting: 'noteRetries', value: 5 },
      { setting: 'tempoBpm', value: 120 },
      { setting: 'showTestNotes', value: true },
      { setting: 'octaveCorrection', value: false },
      { setting: 'yinThreshold', value: 0.2 },
    ]);
  });

  it('drops malformed changes but keeps the good ones', () => {
    const result = interpret([], [propose([
      { setting: 5, value: 1 },
      { setting: 'x', value: 'loud' },
      { setting: 'y', value: null },
      { setting: 'z' },
      { setting: 'w', value: '' },
      null,
      { setting: 'noteRetries', value: 5 },
    ])]);
    expect(result.proposal).toEqual([{ setting: 'noteRetries', value: 5 }]);
    expect(interpret([], [propose('nope')]).proposal).toBeNull();
    expect(interpret([], [{ name: 'propose_settings_changes', input: null }]).proposal).toBeNull();
  });

  it('reads feedback and validates its category and summary', () => {
    const result = interpret(['Noted, thanks.'], [
      feedback('feature_request', '  Wants a dark theme.  '),
      feedback('nonsense', 'bad category'),
      feedback('bug', '   '),
      feedback('bug', 42),
    ]);
    expect(result.feedback).toEqual([{ category: 'feature_request', summary: 'Wants a dark theme.' }]);
  });

  it('caps feedback items per turn and their length', () => {
    const many = Array.from({ length: 5 }, () => feedback('other', 'y'.repeat(900)));
    const result = interpret([], many);
    expect(result.feedback).toHaveLength(3);
    expect(result.feedback[0]?.summary).toHaveLength(500);
  });
});
