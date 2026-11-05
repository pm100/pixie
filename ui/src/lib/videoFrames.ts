/** Stepping through a video frame by frame in the viewer, and saving the frame on screen. The webview gives a video's time but not its frame
 *  rate, so the rate is worked out from the gaps between the frames as they are shown. */

const COMMON_RATES = [23.976, 24, 25, 29.97, 30, 47.952, 48, 50, 59.94, 60];

/** The video's frame rate from the gaps (in seconds) between frames shown one after another: the middle gap, set to a usual rate when it is within 1% of one.
 *  30 when there is not enough to go on. */
export function estimateFps(gaps: readonly number[]): number {
  const good = gaps.filter((g) => g > 0 && g < 0.2).sort((a, b) => a - b);
  if (good.length < 3) return 30;
  const fps = 1 / good[Math.floor(good.length / 2)];
  const near = COMMON_RATES.reduce((a, b) => (Math.abs(b - fps) < Math.abs(a - fps) ? b : a));
  return Math.abs(near - fps) / near < 0.01 ? near : Math.round(fps * 100) / 100;
}

/** Which frame (counting from 0) is shown at this time. */
export const frameNumber = (time: number, fps: number): number => Math.max(0, Math.floor(time * fps + 1e-3));

/** The time to seek to so that the frame `delta` frames from the one at `time` is shown: the middle of that frame, because seeking to the very start of a
 *  frame can show the one before it. Never before the start or past the end (`duration`, when known). */
export function stepTo(time: number, fps: number, delta: number, duration: number = Infinity): number {
  const n = Math.max(0, frameNumber(time, fps) + delta);
  const last = Number.isFinite(duration) ? Math.max(0, Math.ceil(duration * fps) - 1) : n;
  return (Math.min(n, last) + 0.5) / fps;
}

/** A time to seek to `seconds` from now, within the video. */
export const jumpTo = (time: number, seconds: number, duration: number = Infinity): number => Math.min(Math.max(0, time + seconds), Number.isFinite(duration) ? duration : Infinity);

/** "0:32.480", "1:02:03.250": minutes and seconds with the thousandths, and hours when there are any. */
export function timecode(time: number): string {
  const ms = Math.max(0, Math.round(time * 1000));
  const [h, m, s, milli] = [Math.floor(ms / 3_600_000), Math.floor(ms / 60_000) % 60, Math.floor(ms / 1000) % 60, ms % 1000];
  const tail = `${String(s).padStart(2, '0')}.${String(milli).padStart(3, '0')}`;
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${tail}` : `${m}:${tail}`;
}

/** How far into the video, in whole milliseconds, a frame at this time is (what is sent with the picture to be saved). */
export const frameMs = (time: number): number => Math.max(0, Math.round(time * 1000));
