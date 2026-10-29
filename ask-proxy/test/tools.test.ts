import { describe, expect, it } from 'vitest';
import { interpret, TOOL_SPECS } from '../src/tools';

const propose = (changes: unknown) => ({ name: 'propose_settings_changes', input: { changes } });
const feedback = (category: unknown, summary: unknown) => ({ name: 'submit_feedback', input: { category, summary } });

describe('tool specs', () => {
  it('declares the tools with required fields, as plain JSON Schema', () => {
    expect(TOOL_SPECS.map((t) => t.name)).toEqual(['propose_settings_changes', 'go_to', 'find_photos', 'open_tool', 'lookup', 'submit_feedback']);
    expect(TOOL_SPECS[0]?.schema.required).toEqual(['changes']);
    expect(TOOL_SPECS[1]?.schema.required).toEqual(['target', 'say']);
    expect(TOOL_SPECS[5]?.schema.required).toEqual(['category', 'summary']);
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
    expect(interpret([''], [goTo('nowhere'), goTo('tools-email'), goTo('browse')]).goTo).toBe('tools-email');
    expect(interpret(['x'], [goTo('rm -rf')]).goTo).toBeNull();
  });

  it('uses the sentence given with the button when the model wrote no text of its own', () => {
    expect(interpret([], [goTo('tools-email', 'Choose the video, then press Email.')]).reply).toBe('Choose the video, then press Email.');
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

describe('interpret', () => {
  it('returns plain text with no proposal', () => {
    expect(interpret(['Try a slower tempo.'], [])).toEqual({ reply: 'Try a slower tempo.', proposal: null, goTo: null, find: null, openTool: null, lookup: [], feedback: [] });
  });

  it('joins several text blocks and ignores unknown tools', () => {
    expect(interpret(['One.', 'Two.'], [{ name: 'rm_rf', input: { path: '/' } }])).toEqual({
      reply: 'One.\nTwo.',
      proposal: null,
      goTo: null,
      find: null,
      openTool: null,
      lookup: [],
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

describe('find_photos', () => {
  const find = (input: unknown) => ({ name: 'find_photos', input });

  it('is declared with the sentence as a required field', () => {
    const spec = TOOL_SPECS.find((t) => t.name === 'find_photos');
    expect(spec?.schema.required).toEqual(['say']);
    expect(Object.keys((spec?.schema.properties as object) ?? {}).sort()).toEqual(['also', 'from', 'kind', 'limit', 'people', 'picked_only', 'say', 'tag', 'tags', 'text', 'then', 'to', 'with_people']);
  });

  it('keeps well-formed arguments and the sentence as the reply when the model wrote nothing else', () => {
    const r = interpret([], [find({ text: 'pink flowers', then: ['tag'], tag: 'pink flowers', say: 'I will look for pink flowers and offer to tag them.' })]);
    expect(r.find).toEqual({ text: 'pink flowers', also: [], withPeople: false, tags: [], people: [], from: null, to: null, kind: null, limit: null, pickedOnly: false, then: ['tag'], tag: 'pink flowers' });
    expect(r.reply).toBe('I will look for pink flowers and offer to tag them.');
  });

  it('reads dates, kinds, people and tags, and drops what is not valid', () => {
    const r = interpret(['ok'], [find({ tags: ['holiday', 5, ''], people: ['Joan'], from: '2019-06-01', to: 'yesterday', kind: 'video', picked_only: true, then: ['pick', 'delete', 'email'], say: 'x' })]);
    expect(r.find).toEqual({ text: null, also: [], withPeople: false, tags: ['holiday'], people: ['Joan'], from: '2019-06-01', to: null, kind: 'video', limit: null, pickedOnly: true, then: ['pick', 'email'], tag: null });
  });

  it('cuts long text and long lists', () => {
    const r = interpret(['ok'], [find({ text: 'x'.repeat(500), tags: Array.from({ length: 30 }, (_, i) => `t${i}`), say: 'x' })]);
    expect(r.find?.text).toHaveLength(200);
    expect(r.find?.tags).toHaveLength(10);
  });

  it('reads how many the user asked for, as a whole number from 1 to 500', () => {
    expect(interpret(['ok'], [find({ text: 'a', limit: 20, say: 'x' })]).find?.limit).toBe(20);
    expect(interpret(['ok'], [find({ text: 'a', limit: 20.7, say: 'x' })]).find?.limit).toBe(20);
    for (const bad of [0, -3, 'twenty', 501, null, Number.NaN]) expect(interpret(['ok'], [find({ text: 'a', limit: bad, say: 'x' })]).find?.limit).toBeNull();
  });

  it('reads more things that must also be in the picture, and people being in it', () => {
    const r = interpret(['ok'], [find({ text: 'yellow flowers', also: ['a birthday cake', '', 5], with_people: true, say: 'x' })]);
    expect(r.find).toMatchObject({ text: 'yellow flowers', also: ['a birthday cake'], withPeople: true });
  });

  it('a search for photos with people in them is a search, even with nothing else', () => {
    expect(interpret(['ok'], [find({ with_people: true, say: 'x' })]).find?.withPeople).toBe(true);
  });

  it('cuts a long list of things to look for', () => {
    expect(interpret(['ok'], [find({ text: 'a', also: Array.from({ length: 9 }, (_, i) => `t${i}`), say: 'x' })]).find?.also).toHaveLength(3);
  });

  it('ignores a call with nothing to look for', () => {
    expect(interpret(['ok'], [find({ say: 'x' })]).find).toBeNull();
  });

  it('takes the first find only', () => {
    expect(interpret(['ok'], [find({ text: 'a', say: 'x' }), find({ text: 'b', say: 'x' })]).find?.text).toBe('a');
  });

  it('a tag action without a tag name is dropped, so nothing is tagged with a blank', () => {
    expect(interpret(['ok'], [find({ text: 'a', then: ['tag'], say: 'x' })]).find?.then).toEqual([]);
  });
});

describe('lookup', () => {
  const lookup = (input: unknown, id = 'toolu_1') => ({ name: 'lookup', input, id });

  it('is declared, asking only for what to look up', () => {
    const spec = TOOL_SPECS.find((t) => t.name === 'lookup');
    expect(spec?.schema.required).toEqual(['what']);
    expect((spec?.schema.properties as any).what.enum).toEqual(['count', 'tags', 'people', 'trips']);
  });

  it('asks the app for the answer, keeping the call so it can be sent back, and reads the filter of a count', () => {
    const input = { what: 'count', text: 'dog', from: '2019-01-01' };
    const r = interpret(['One moment.'], [lookup(input)]);
    expect(r.lookup).toEqual([{ id: 'toolu_1', input, what: 'count', query: null, filter: expect.objectContaining({ text: 'dog', from: '2019-01-01' }) }]);
  });

  it('reads a name lookup with its text to look for', () => {
    const r = interpret([], [lookup({ what: 'people', query: ' rose ' })]);
    expect(r.lookup[0]).toMatchObject({ what: 'people', query: 'rose', filter: null });
  });

  it('a count of everything has no filter', () => {
    expect(interpret([], [lookup({ what: 'count' })]).lookup[0]).toMatchObject({ what: 'count', filter: null });
  });

  it('drops a lookup with no id (it could not be answered), an unknown kind, and more than three', () => {
    expect(interpret([], [lookup({ what: 'people' }, '')]).lookup).toEqual([]);
    expect(interpret([], [lookup({ what: 'delete_everything' })]).lookup).toEqual([]);
    const many = Array.from({ length: 5 }, (_, i) => lookup({ what: 'tags' }, `toolu_${i}`));
    expect(interpret([], many).lookup).toHaveLength(3);
  });

  it('says nothing else while it waits for the answer, so nothing is done on a half-formed plan', () => {
    const r = interpret(['Let me check.'], [lookup({ what: 'people', query: 'rose' }), { name: 'find_photos', input: { text: 'dog', say: 'x' } }, { name: 'go_to', input: { target: 'browse', say: 'x' } }]);
    expect(r.lookup).toHaveLength(1);
    expect(r.find).toBeNull();
    expect(r.goTo).toBeNull();
    expect(r.proposal).toBeNull();
    expect(r.reply).toBe('Let me check.');
  });

  it('is empty when the model asked for nothing', () => {
    expect(interpret(['Hi.'], []).lookup).toEqual([]);
  });
});

describe('open_tool', () => {
  const open = (input: unknown) => ({ name: 'open_tool', input });

  it('is declared, needing the tool, where the photos come from, and the sentence', () => {
    const spec = TOOL_SPECS.find((t) => t.name === 'open_tool');
    expect(spec?.schema.required).toEqual(['tool', 'source', 'say']);
    expect((spec?.schema.properties as any).source.enum).toEqual(['results', 'picked', 'tag']);
    expect((spec?.schema.properties as any).tool.enum).toEqual(['details', 'email', 'print', 'tags']);
  });

  it('reads a tool to open on the results, the picked photos, or a tag, and uses the sentence when the model wrote none', () => {
    const r = interpret([], [open({ tool: 'print', source: 'results', say: 'Opening Print on the photos we found.' })]);
    expect(r.openTool).toEqual({ tool: 'print', source: 'results', tag: null });
    expect(r.reply).toBe('Opening Print on the photos we found.');
    expect(interpret([], [open({ tool: 'email', source: 'tag', tag: ' rose1 ', say: 'x' })]).openTool).toEqual({ tool: 'email', source: 'tag', tag: 'rose1' });
  });

  it('needs a tag name for a tag, and drops what it does not know', () => {
    expect(interpret(['ok'], [open({ tool: 'print', source: 'tag', say: 'x' })]).openTool).toBeNull();
    expect(interpret(['ok'], [open({ tool: 'delete', source: 'results', say: 'x' })]).openTool).toBeNull();
    expect(interpret(['ok'], [open({ tool: 'print', source: 'everything', say: 'x' })]).openTool).toBeNull();
  });

  it('forgets a tag name it was not asked for, and takes only the first', () => {
    expect(interpret(['ok'], [open({ tool: 'tags', source: 'picked', tag: 'x', say: 'x' })]).openTool).toEqual({ tool: 'tags', source: 'picked', tag: null });
    expect(interpret(['ok'], [open({ tool: 'print', source: 'results', say: 'a' }), open({ tool: 'email', source: 'results', say: 'b' })]).openTool?.tool).toBe('print');
  });
});
