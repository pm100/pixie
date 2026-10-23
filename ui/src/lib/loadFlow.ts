/** The first-run / add-a-folder flow: which full-screen step is showing ('off': Browse). 'background' is the note shown
 *  after "Carry on in the background": what is still running, how to tell when it ends, what to do meanwhile. */
export type Phase = 'off' | 'welcome' | 'scanning' | 'analysing' | 'allset' | 'background';

const n = (x: number) => x.toLocaleString();
const plural = (count: number, one: string) => `${n(count)} ${one}${count === 1 ? '' : 's'}`;

/** Where the flow goes when the scan ends. Stopping keeps what was found and shows Browse; finding nothing, or only
 *  videos (which are not analysed), has nothing to analyse and goes straight to the end screen. */
export function phaseAfterScan(o: { stopped: boolean; found: number; photos: number }): Phase {
  if (o.stopped) return 'off';
  return o.found === 0 || o.photos === 0 ? 'allset' : 'analysing';
}

/** "3 minutes", "1 second": rounded to whole minutes from a minute up. */
function duration(seconds: number): string {
  return seconds < 60 ? plural(Math.round(seconds), 'second') : plural(Math.round(seconds / 60), 'minute');
}

/** What the end screen says. `problems`: folders that could not be read, so success is never claimed silently. */
export function allSetSummary(o: {
  folders: { name: string; count: number }[]; photos: number; videos: number; seconds: number; facesFound: boolean;
  problems?: string[];
}): { title: string; detail: string; rows: { name: string; text: string }[]; showPeopleBox: boolean; problems: string[] } {
  const items = o.photos + o.videos;
  const problems = o.problems ?? [];
  if (items === 0) {
    return { title: 'Nothing was found', detail: 'Pixie did not find any photos or videos in the folders you chose.', rows: [], showPeopleBox: false, problems };
  }
  return {
    problems,
    title: `All set — ${n(items)} items found`,
    detail: `${plural(o.photos, 'photo')} and ${plural(o.videos, 'video')}, in ${plural(o.folders.length, 'folder')}. That took ${duration(o.seconds)}.`,
    rows: o.folders.map((f) => ({ name: f.name, text: n(f.count) })),
    showPeopleBox: o.facesFound,
  };
}

/** The Start button on the welcome screen: just "Start", off while nothing is ticked. */
export function startLabel(ticked: number): { label: string; disabled: boolean } {
  return { label: 'Start', disabled: ticked === 0 };
}
