import { savePoster, type Photo } from './api';
import { videoUrl } from './url';

const TIMEOUT_MS = 8000;
const MAX_SIDE = 360;

/** Decodes one frame of the video with the webview's own player and saves it as the poster. */
export function makePoster(p: Photo): Promise<boolean> {
  return new Promise((resolve) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    v.crossOrigin = 'anonymous';
    let done = false;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      v.removeAttribute('src'); // always tear the decoder down
      v.load();
      resolve(ok);
    };
    const timer = setTimeout(() => finish(false), TIMEOUT_MS);
    v.onerror = () => finish(false);
    v.onloadedmetadata = () => {
      const d = Number.isFinite(v.duration) ? v.duration : 0;
      v.currentTime = Math.min(1, d / 2);
    };
    v.onseeked = () => {
      try {
        const scale = Math.min(1, MAX_SIDE / Math.max(v.videoWidth, v.videoHeight, 1));
        const c = document.createElement('canvas');
        c.width = Math.max(1, Math.round(v.videoWidth * scale));
        c.height = Math.max(1, Math.round(v.videoHeight * scale));
        c.getContext('2d')!.drawImage(v, 0, 0, c.width, c.height);
        c.toBlob(
          async (blob) => {
            if (!blob) return finish(false);
            try {
              await savePoster(p.id, new Uint8Array(await blob.arrayBuffer()));
              finish(true);
            } catch {
              finish(false);
            }
          },
          'image/jpeg',
          0.8,
        );
      } catch {
        finish(false);
      }
    };
    v.src = videoUrl(p.id, p.mtime, p.size);
  });
}
