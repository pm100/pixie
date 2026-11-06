// What Import shares between the tool and the Browse tree: the sources that are open, the drives that could be opened, the scan and
// the Keep in progress. Kept in one place so both show the same thing, and the events and the polling run once.
import { importAvailable, importSources, onImportDone, onImportProgress, onImportScan, onImportScanDone, type KeepDone, type KeepProgress, type OpenSource, type Removable, type ScanEvent } from './api';

export const imports = $state({
  /** The sources that are open (indexed where they are). */
  sources: [] as OpenSource[],
  /** The cards and drives that could be opened. */
  available: [] as Removable[],
  /** How far reading the source being read has got. */
  scan: null as ScanEvent | null,
  /** A Keep is running, for this source, and how far it has got. */
  keeping: null as number | null,
  progress: null as KeepProgress | null,
  /** How the last Keep ended (cleared when another starts). */
  done: null as KeepDone | null,
  /** The last thing that went wrong, in words. */
  error: '',
});

export async function refreshSources(): Promise<void> {
  try {
    imports.sources = await importSources();
  } catch (e) {
    imports.error = String(e);
  }
}

export async function refreshAvailable(): Promise<void> {
  try {
    imports.available = await importAvailable();
  } catch {
    /* the next refresh tries again */
  }
}

/** Starts listening for what Import reports, and looking at the open sources now and then (a card may be pulled out). Returns the stop function. */
export function startImportEvents(): () => void {
  void refreshSources();
  const subs = [
    onImportScan((e) => { imports.scan = e; }),
    onImportScanDone((e) => {
      imports.scan = null;
      if (e.error !== null) imports.error = e.error;
      void refreshSources();
    }),
    onImportProgress((p) => { imports.progress = p; }),
    onImportDone((d) => {
      imports.keeping = null;
      imports.progress = null;
      imports.done = d;
      void refreshSources();
    }),
  ];
  const timer = setInterval(() => {
    if (imports.sources.length > 0 && imports.keeping === null) void refreshSources();
  }, 3000);
  return () => {
    clearInterval(timer);
    subs.forEach((s) => s.then((un) => un()).catch(() => {}));
  };
}
