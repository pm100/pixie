import { describe, expect, it } from 'vitest';
import { estimateFps, frameMs, frameNumber, jumpTo, stepTo, timecode } from './videoFrames';

describe('estimateFps', () => {
  it('reads a usual rate from the gaps between frames, allowing for jitter', () => {
    expect(estimateFps([0.0334, 0.0333, 0.0332, 0.0335, 0.0333])).toBe(30);
    expect(estimateFps([0.041667, 0.041666, 0.041668, 0.041667])).toBe(24);
    expect(estimateFps([0.041708, 0.041709, 0.041707])).toBe(23.976);
    expect(estimateFps([0.01667, 0.01666, 0.01668])).toBe(60);
    expect(estimateFps([0.03337, 0.03336, 0.03338])).toBe(29.97);
  });
  it('ignores gaps that are a stall or a seek, and falls back to 30 without enough to go on', () => {
    expect(estimateFps([0.04, 0.04, 0.04, 2.5, 0])).toBe(25);
    expect(estimateFps([0.04, 0.04])).toBe(30);
    expect(estimateFps([])).toBe(30);
  });
  it('keeps an unusual rate as it is', () => {
    expect(estimateFps([0.0667, 0.0667, 0.0667])).toBe(14.99);
  });
});

describe('stepping', () => {
  it('counts frames from 0', () => {
    expect(frameNumber(0, 30)).toBe(0);
    expect(frameNumber(1, 30)).toBe(30);
    expect(frameNumber(0.5 / 30, 30)).toBe(0);
    expect(frameNumber(-1, 30)).toBe(0);
  });
  it('seeks to the middle of the next or previous frame', () => {
    const at = stepTo(1, 30, 1); // from frame 30 to frame 31
    expect(frameNumber(at, 30)).toBe(31);
    expect(at).toBeCloseTo(31.5 / 30, 9);
    expect(frameNumber(stepTo(at, 30, -1), 30)).toBe(30);
  });
  it('stops at the first and the last frame', () => {
    expect(frameNumber(stepTo(0, 30, -1), 30)).toBe(0);
    expect(frameNumber(stepTo(9.9, 30, 5, 10), 30)).toBe(299); // 10 s at 30 fps has frames 0 to 299
  });
  it('jumps whole seconds within the video', () => {
    expect(jumpTo(5, 1, 10)).toBe(6);
    expect(jumpTo(0.4, -1, 10)).toBe(0);
    expect(jumpTo(9.5, 1, 10)).toBe(10);
    expect(jumpTo(3, 1)).toBe(4);
  });
});

describe('timecode and frameMs', () => {
  it('shows minutes, seconds and thousandths, and hours when there are any', () => {
    expect(timecode(0)).toBe('0:00.000');
    expect(timecode(32.48)).toBe('0:32.480');
    expect(timecode(62.5)).toBe('1:02.500');
    expect(timecode(3723.25)).toBe('1:02:03.250');
  });
  it('rounds to whole milliseconds', () => {
    expect(frameMs(32.4804)).toBe(32_480);
    expect(frameMs(-1)).toBe(0);
  });
});
