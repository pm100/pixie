/** A file the scan has just stored, as the "now reading" strip sees it (mirrors the Rust `LatestItem`). */
export type StripItem = { id: number; mtime: number; size: number; name: string; kind: 'photo' | 'video' };

/**
 * Decides which picture the strip shows next. Pictures arrive faster than a slow drive can deliver their thumbnails, and
 * a picture whose address keeps changing never finishes loading, so this loads each one in the background and shows it
 * only once it is ready, always preferring the newest that is waiting (older ones are skipped, not queued). A video has
 * no thumbnail while it is scanned, so it is shown at once as a placeholder. `minGapMs` limits how often the strip changes.
 */
export function createStripFeed(o: {
  load: (item: StripItem) => Promise<boolean>;
  onShow: (item: StripItem) => void;
  minGapMs?: number;
  concurrency?: number;
}): { offer: (item: StripItem) => void; stop: () => void } {
  const minGap = o.minGapMs ?? 100;
  const concurrency = o.concurrency ?? 3;
  let waiting: { item: StripItem; seq: number } | null = null; // the newest offer not yet taken
  let ready: { item: StripItem; seq: number } | null = null; // the newest loaded picture not yet shown
  let seq = 0, shownSeq = -1, inflight = 0, lastShown = -Infinity;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let stopped = false;

  function show() {
    timer = null;
    if (stopped || !ready) return;
    const wait = lastShown + minGap - Date.now();
    if (wait > 0) {
      timer = setTimeout(show, wait);
      return;
    }
    const next = ready;
    ready = null;
    if (next.seq < shownSeq) return; // something newer has already been shown
    shownSeq = next.seq;
    lastShown = Date.now();
    o.onShow(next.item);
  }

  function becomeReady(item: StripItem, mySeq: number) {
    if (stopped || mySeq < shownSeq || (ready && ready.seq > mySeq)) return;
    ready = { item, seq: mySeq };
    if (!timer) show();
  }

  function pump() {
    while (!stopped && inflight < concurrency && waiting) {
      const { item, seq: mySeq } = waiting;
      waiting = null;
      if (item.kind === 'video') {
        becomeReady(item, mySeq);
        continue;
      }
      inflight++;
      o.load(item)
        .then((ok) => ok && becomeReady(item, mySeq), () => {})
        .finally(() => {
          inflight--;
          pump();
        });
    }
  }

  return {
    offer(item) {
      if (stopped) return;
      waiting = { item, seq: seq++ };
      pump();
    },
    stop() {
      stopped = true;
      waiting = null;
      ready = null;
      if (timer) clearTimeout(timer);
    },
  };
}
