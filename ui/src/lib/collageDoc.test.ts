import { describe, expect, it } from 'vitest';
import { docDpi, MIN_FRAME, newDoc, pageMm, paperLabel, parseDoc, serializeDoc, toSaveArgs, type CollageDoc } from './collageDoc';

describe('parseDoc', () => {
  it('gives a fresh page for nothing, damage, a newer version or the wrong shape', () => {
    for (const text of [null, '', '{', '{"v":2}', '{"v":1', '[]', '7', 'null', '"x"']) expect(parseDoc(text), String(text)).toEqual(newDoc());
  });

  it('keeps a good document as it was', () => {
    const d: CollageDoc = { ...newDoc('en-US'), landscape: true, dpi: 200, gap: 2.5, shadow: true, template: 'grid-2x2',
      frames: [{ id: 'a', photo: 7, x: 0.1, y: 0.2, w: 0.3, h: 0.4, fx: 0.25, fy: 0.75, zoom: 2 }] };
    expect(parseDoc(serializeDoc(d), 'en-US')).toEqual(d);
  });

  it('turns filling the space on for a collage kept before there was such a setting, and keeps it off when it was turned off', () => {
    expect(parseDoc('{"v":1,"snapPage":false}').snapFill).toBe(true);
    expect(parseDoc('{"v":1,"snapFill":false}').snapFill).toBe(false);
    expect(parseDoc('{"v":1,"snapFill":"no"}').snapFill).toBe(true);
  });

  it('puts every number back in range and every frame on the page', () => {
    const d = parseDoc('{"v":1,"gap":-5,"corner":1e9,"customW":0,"frames":[{"id":"a","photo":1,"x":0.9,"y":0.9,"w":0.5,"h":0.0,"zoom":9,"fx":-3}]}');
    expect([d.gap, d.corner, d.customW]).toEqual([0, 100, 1]);
    const f = d.frames[0];
    expect(f.w).toBe(0.5);
    expect(f.h).toBe(MIN_FRAME);
    expect(f.x + f.w).toBeLessThanOrEqual(1);
    expect(f.y + f.h).toBeLessThanOrEqual(1);
    expect([f.zoom, f.fx]).toEqual([4, 0]);
  });

  it('drops frames that are not frames and names those without an id', () => {
    const d = parseDoc('{"v":1,"frames":["x",null,7,{"photo":"3"},{"photo":4,"x":0,"y":0,"w":0.5,"h":0.5}]}');
    expect(d.frames).toHaveLength(1);
    expect(d.frames[0].photo).toBe(4);
    expect(d.frames[0].id).toBeTruthy();
  });

  it('replaces an unknown paper, a bad colour and a bad resolution', () => {
    const d = parseDoc('{"v":1,"paper":"nope","background":"red","borderColour":"#12","dpi":"300"}', 'en-US');
    expect([d.paper, d.background, d.borderColour, d.dpi]).toEqual(['8x10in', '#ffffff', '#ffffff', null]);
    expect(parseDoc('{"v":1,"paper":"custom","dpi":200}').paper).toBe('custom');
    expect(parseDoc('{"v":1,"dpi":200}').dpi).toBe(200);
  });
});

describe('page size', () => {
  it('turns the paper to suit the orientation', () => {
    const d = { ...newDoc(), paper: '20x30cm' };
    expect(pageMm(d)).toEqual({ w: 200, h: 300 });
    expect(pageMm({ ...d, landscape: true })).toEqual({ w: 300, h: 200 });
    expect(pageMm({ ...d, paper: 'custom', customW: 60, customH: 40 })).toEqual({ w: 400, h: 600 });
  });

  it('uses 300 dpi for a print and 150 for a poster unless one was chosen', () => {
    expect(docDpi(newDoc())).toBe(300);
    expect(docDpi({ ...newDoc(), paper: '70x100cm' })).toBe(150);
    expect(docDpi({ ...newDoc(), paper: '70x100cm', dpi: 300 })).toBe(300);
  });
});

describe('toSaveArgs', () => {
  it('sends the frames in order with the page and look', () => {
    const d: CollageDoc = { ...newDoc(), paper: '70x100cm', shadow: true, border: 3, borderColour: '#112233',
      frames: [{ id: 'a', photo: 1, x: 0, y: 0, w: 0.5, h: 0.5, fx: 0.5, fy: 0.5, zoom: 1 }, { id: 'b', photo: 2, x: 0.5, y: 0.5, w: 0.5, h: 0.5, fx: 0.1, fy: 0.9, zoom: 2 }] };
    const a = toSaveArgs(d);
    expect([a.paper_w_mm, a.paper_h_mm, a.dpi, a.shadow, a.border_mm, a.border_colour]).toEqual([700, 1000, 150, true, 3, '#112233']);
    expect(a.frames.map((f) => f.photo_id)).toEqual([1, 2]);
    expect(a.frames[1]).toEqual({ photo_id: 2, x: 0.5, y: 0.5, w: 0.5, h: 0.5, focus_x: 0.1, focus_y: 0.9, zoom: 2 });
  });
});

describe('paperLabel', () => {
  it('names the paper, or the custom size', () => {
    expect(paperLabel({ ...newDoc(), paper: '8x10in' })).toBe('8 × 10 in');
    expect(paperLabel({ ...newDoc(), paper: 'a4' })).toBe('A4');
    expect(paperLabel({ ...newDoc(), paper: 'custom', customW: 40, customH: 60 })).toBe('40 × 60 cm');
  });
});
