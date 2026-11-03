import { describe, expect, it } from 'vitest';
import { holdEstimate, label, view, type ActivitySnapshot, type ActivityTask } from './activity';

const task = (p: Partial<ActivityTask> = {}): ActivityTask => ({
  kind: 'auto_tags', state: 'running', done: 0, total: null, bytes_done: 0, bytes_total: null, per_sec: null, bytes_per_sec: null,
  eta_secs: null, started_ms: 0, finished_ms: null, waiting_secs: null, note: null, current: null, items: [], place: null, ...p,
});
const snap = (tasks: ActivityTask[], eta_secs: number | null = null, finished: ActivityTask[] = []): ActivitySnapshot => ({ tasks, finished, eta_secs, now_ms: 0 });

describe('the progress screen view', () => {
  it('has nothing to show and no pill when nothing is running', () => {
    const v = view(null);
    expect([v.pill, v.overall, v.running.length, v.finished.length]).toEqual([null, 'Nothing is running.', 0, 0]);
    expect(view(snap([])).pill).toBeNull();
  });

  it('shows no bar and no time for work whose total is not known yet', () => {
    const v = view(snap([task({ kind: 'scan', done: 120 })]));
    expect(v.running[0].fraction).toBeNull();
    expect(v.running[0].left).toBe('Estimating…');
    expect(v.overall).not.toMatch(/left/);
  });

  it('shows a bar, counts, speed and time left for work that has them', () => {
    const r = view(snap([task({ done: 40, total: 100, per_sec: 10, eta_secs: 6 })], 6)).running[0];
    expect(r.fraction).toBeCloseTo(0.4);
    expect(r.counts).toBe('40 of 100 photos');
    expect(r.speed).toBe('10 photos/s');
    expect(r.left).toBe('under a minute');
  });

  it('judges a copy by its bytes when they are known', () => {
    const r = view(snap([task({ kind: 'copy_to_master', done: 0, total: 5, bytes_done: 4_400_000_000, bytes_total: 5_800_000_000, bytes_per_sec: 23_000_000, eta_secs: 60 })])).running[0];
    expect(r.fraction).toBeCloseTo(4.4 / 5.8, 2);
    expect(r.counts).toBe('0 of 5 items, 4.1 GB of 5.4 GB');
    expect(r.speed).toBe('21.9 MB/s');
  });

  it('counts with the right plural', () => {
    expect(view(snap([task({ kind: 'read', done: 0, total: 1 })])).running[0].counts).toBe('0 of 1 file');
    expect(view(snap([task({ kind: 'read', done: 1, total: 2 })])).running[0].counts).toBe('1 of 2 files');
    expect(view(snap([task({ kind: 'scan', done: 1 })])).running[0].counts).toBe('1 file found');
    expect(view(snap([task({ kind: 'scan', done: 2 })])).running[0].counts).toBe('2 files found');
  });

  it('shows a paused row with no time left and no line of explanation (the bar and the Resume button say so)', () => {
    const r = view(snap([task({ state: 'paused', done: 5, total: 10 })])).running[0];
    expect(r.left).toBe('');
    expect(r.explain).toBe('');
    expect(r.state).toBe('paused');
  });

  it('shows how long a cloud drive has been waited for, and no time left', () => {
    const r = view(snap([task({ kind: 'cloud_wait', state: 'waiting', done: 2, total: 10, waiting_secs: 90, eta_secs: null })])).running[0];
    expect(r.left).toBe('');
    expect(r.explain).toMatch(/cloud drive/);
    expect(r.explain).toMatch(/1 min 30 s|90 s/);
  });

  it('shows no speed and says there is no progress when a job has not moved for a while, instead of 0 a minute and Estimating', () => {
    const r = view(snap([task({ kind: 'analysing', done: 2773, total: 14273, per_sec: 0, eta_secs: null })])).running[0];
    expect([r.speed, r.left]).toEqual(['', 'No progress']);
    const young = view(snap([task({ kind: 'analysing', done: 5, total: 100, per_sec: null })])).running[0];
    expect(young.left).toBe('Estimating…');
  });

  it('presents a cloud wait as a list of the biggest files, with no count and no progress bar', () => {
    const items = [
      { name: 'a.mp4', bytes: 2_900_000_000, secs: 130, source: 'icloud' },
      { name: 'b.mov', bytes: 500_000_000, secs: 20, source: 'onedrive' },
      { name: 'c.mp4', bytes: 300_000_000, secs: 5, source: 'onedrive' },
      { name: 'd.mp4', bytes: 100_000_000, secs: 1, source: 'onedrive' },
    ];
    for (const state of ['waiting', 'running'] as const) {
      const r = view(snap([task({ kind: 'cloud_wait', state, done: 3, total: 455, waiting_secs: 130, items })])).running[0];
      expect([r.counts, r.fraction, r.speed, r.left]).toEqual(['', null, '', '']);
      expect(r.files.map((f) => f.name)).toEqual(['a.mp4', 'b.mov', 'c.mp4']);
      expect(r.files[0].detail).toBe('2.7 GB · waited 2 min 10 s');
      expect(r.moreFiles).toBe(1);
      expect(r.source, 'the picture is for the biggest file').toBe('icloud');
    }
    const odd = view(snap([task({ kind: 'cloud_wait', state: 'waiting', items: [{ name: 'x', bytes: 1, secs: 0, source: 'dropbox' }] })])).running[0];
    expect(odd.source, 'a drive with no picture gets the plain cloud').toBe('other');
    expect(view(snap([task({ kind: 'cloud_wait', state: 'waiting' })])).running[0].source).toBe('other');
    expect(view(snap([task({ kind: 'analysing', done: 1, total: 9 })])).running[0].files).toEqual([]);
    expect(view(snap([task({ kind: 'analysing', done: 1, total: 9 })])).running[0].source).toBeNull();
  });

  it('names the folder the scan is in, cutting a long path in the middle but keeping it whole in the tooltip', () => {
    const r = view(snap([task({ kind: 'scan', done: 120, note: 'Looking in 2019/Summer trip' })])).running[0];
    expect([r.explain, r.explainFull]).toEqual(['Looking in 2019/Summer trip', 'Looking in 2019/Summer trip']);
    const deep = `Looking in ${'a'.repeat(40)}/${'b'.repeat(40)}/Raw`;
    const long = view(snap([task({ kind: 'scan', done: 120, note: deep })])).running[0];
    expect([long.explain.length, long.explain.includes('…'), long.explainFull]).toEqual([72, true, deep]);
  });

  it('shows the watched folder under the loading and analysis rows, keeping the line when it is not known yet', () => {
    const r = view(snap([task({ kind: 'analysing', done: 1, total: 9, place: 'P:\\' })])).running[0];
    expect([r.place, r.placeFull]).toEqual(['P:\\', 'P:\\']);
    expect(view(snap([task({ kind: 'analysing', done: 1, total: 9 })])).running[0].place).toBe('');
    const long = 'C:\\' + 'a'.repeat(100);
    const cut = view(snap([task({ kind: 'read', done: 1, total: 9, place: long })])).running[0];
    expect([cut.place?.length, cut.placeFull]).toEqual([64, long]);
    expect(view(snap([task({ kind: 'copy_to_master', done: 1, total: 9 })])).running[0].place).toBeNull();
  });

  it('says what a waiting job is waiting for and how long it has waited', () => {
    const r = view(snap([task({ kind: 'duplicates', state: 'waiting', done: 5, total: 20, waiting_secs: 150, note: 'Waiting for the folder scan and loading to finish' })])).running[0];
    expect(r.explain).toBe('Waiting for the folder scan and loading to finish · 2 min 30 s');
    expect(r.left).toBe('');
  });

  it('names the file a running job has been on for a while, with its size and how long', () => {
    const r = view(snap([task({ kind: 'duplicates', done: 5, total: 20, current: { name: 'big.mp4', bytes: 5_800_000_000, secs: 130 } })])).running[0];
    expect(r.explain).toBe('Working on big.mp4 (5.4 GB) for 2 min 10 s');
    const quick = view(snap([task({ kind: 'read', done: 5, total: 20, note: 'Checking what was synced', current: null })])).running[0];
    expect(quick.explain).toBe('Checking what was synced');
  });

  it('does not show a model download, but shows an error with its reason', () => {
    expect(view(snap([task({ kind: 'models', bytes_done: 30_000_000, bytes_total: 90_000_000 })])).running).toEqual([]);
    expect(view(snap([task({ kind: 'face_models', bytes_done: 5, bytes_total: 50 })])).running).toEqual([]);
    const bad = view(snap([task({ kind: 'models', state: 'error', note: 'no network' })])).running[0];
    expect(bad.explain).toContain('no network');
    expect(bad.left).toBe('');
  });

  it('counts the one analysis row in photos, and says what it is doing', () => {
    const r = view(snap([task({ kind: 'analysing', done: 821, total: 14274, note: 'Auto tags and faces' })])).running[0];
    expect([r.label, r.counts, r.explain]).toEqual(['Analysing photos', '821 of 14,274 photos', 'Auto tags and faces']);
  });

  it('keeps the rows in a fixed order whichever started first, so the analysis and the cloud wait never swap places', () => {
    const order = (tasks: ActivityTask[]) => view(snap(tasks)).running.map((r) => r.kind);
    const analysing = task({ kind: 'analysing', done: 1, total: 9 });
    const cloud = task({ kind: 'cloud_wait', state: 'waiting', done: 1, total: 3 });
    expect(order([analysing, cloud])).toEqual(['analysing', 'cloud_wait']);
    expect(order([cloud, analysing])).toEqual(['analysing', 'cloud_wait']);
    expect(order([cloud, { ...analysing, state: 'paused' }])).toEqual(['analysing', 'cloud_wait']);
  });

  it('sorts errors first, then by kind (pausing does not move a row), and puts finished work in its own list', () => {
    const v = view(snap([
      task({ kind: 'faces', state: 'paused', done: 1, total: 9 }),
      task({ kind: 'scan', state: 'done', done: 9, total: 9, started_ms: 0, finished_ms: 125_000 }),
      task({ kind: 'read', done: 1, total: 9 }),
      task({ kind: 'models', state: 'error', note: 'x' }),
    ]));
    expect(v.running.map((r) => r.kind)).toEqual(['models', 'read', 'faces']);
    expect(v.finished.map((r) => r.kind)).toEqual(['scan']);
    expect(v.finished[0].explain).toBe('Took about 2 min.');
  });

  it('keeps earlier finished runs in the finished list, newest first, while the same kind runs again', () => {
    const old = (kind: ActivityTask['kind'], finished_ms: number) => task({ kind, state: 'done', done: 9, total: 9, started_ms: 0, finished_ms });
    const v = view(snap([task({ kind: 'duplicates', done: 1, total: 9, started_ms: 200_000 })], null, [old('duplicates', 125_000), old('faces', 400_000)]));
    expect(v.running.map((r) => r.kind)).toEqual(['duplicates']);
    expect(v.finished.map((r) => r.kind)).toEqual(['faces', 'duplicates']);
    expect(new Set(v.finished.map((r) => r.key)).size).toBe(2);
  });

  it('names the pill by what the unfinished work is doing', () => {
    expect(view(snap([task({ done: 1, total: 9 })])).pill).toBe('working');
    expect(view(snap([task({ state: 'paused' }), task({ kind: 'faces', state: 'paused' })])).pill).toBe('paused');
    expect(view(snap([task({ state: 'paused' }), task({ kind: 'faces' })])).pill).toBe('working');
    expect(view(snap([task({ state: 'paused' }), task({ kind: 'models', state: 'error' })])).pill).toBe('error');
    expect(view(snap([task({ state: 'waiting', kind: 'cloud_wait' })])).pill).toBe('working');
    // Pause cannot stop the cloud drive's download: the rest paused is still "paused" while it goes on.
    expect(view(snap([task({ state: 'paused' }), task({ state: 'waiting', kind: 'cloud_wait' })])).pill).toBe('paused');
    expect(view(snap([task({ state: 'done', done: 3, total: 3 })])).pill).toBeNull();
  });

  it('gives the overall time left and how many things are going on', () => {
    const v = view(snap([task({ done: 40, total: 100, per_sec: 10, eta_secs: 600 }), task({ kind: 'faces', done: 1, total: 5, per_sec: 1, eta_secs: 300 })], 600));
    expect(v.overall).toBe('About 10 min left · 10 photos/s');
    expect(view(snap([task({ done: 1, total: 100, eta_secs: 30 })], 30)).overall).toBe('Under a minute left');
    expect(view(snap([task({ done: 1, total: 100 })], null)).overall).toBe('Estimating time…');
    expect(view(snap([task({ state: 'paused' })])).overall).toBe('Paused.');
    expect(view(snap([task({ state: 'done', done: 3, total: 3 })])).overall).toBe('Everything is done.');
  });

  it('puts the speed of the slowest job next to the time left, and the bytes speed when that is what it has', () => {
    const slow = task({ kind: 'read', done: 1, total: 100, per_sec: 2, eta_secs: 900 });
    const quick = task({ kind: 'faces', done: 1, total: 100, per_sec: 50, eta_secs: 60 });
    expect(view(snap([quick, slow], 900)).overall).toBe('About 15 min left · 2.0 files/s');
    const copy = task({ kind: 'copy_to_master', done: 0, total: 5, bytes_done: 1, bytes_total: 9, bytes_per_sec: 23_000_000, eta_secs: 120 });
    expect(view(snap([copy], 120)).overall).toBe('About 2 min left · 21.9 MB/s');
  });

  it('says estimating only until there has been an estimate: after that it keeps showing the last time', () => {
    const lost = snap([task({ done: 5, total: 100 })], null);
    expect(view(lost).overall).toBe('Estimating time…');
    expect(view(lost, 600).overall).toBe('About 10 min left');
    expect(view(snap([task({ done: 5, total: 100, per_sec: 1, eta_secs: 300 })], 300), 600).overall).toBe('About 5 min left · 1.0 photos/s');
  });

  it('holds the last estimate while work runs and forgets it when nothing does', () => {
    expect(holdEstimate(null, 600, true)).toBe(600);
    expect(holdEstimate(600, null, true)).toBe(600);
    expect(holdEstimate(600, 300, true)).toBe(300);
    expect(holdEstimate(600, null, false)).toBeNull();
    expect(holdEstimate(null, null, true)).toBeNull();
  });

  it('names every kind of work in plain words', () => {
    expect(label('scan')).toBe('Finding your files');
    expect(label('read')).toBe('Reading and fingerprinting files');
    expect(label('copy_to_master')).toBe('Syncing to your sync folder');
    expect(label('cloud_wait')).toBe('Waiting for your cloud drive');
    expect(label('models')).toBe('Getting models');
    expect(label('face_models')).toBe('Getting face models');
    expect(label('analysing')).toBe('Analysing photos');
  });
});
