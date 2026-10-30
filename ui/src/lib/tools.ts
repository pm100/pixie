/** The tools behind the Send, Create, Organize and Clean up tabs. A tool that is not built yet is listed greyed out and opens a placeholder showing what it will look like. */
export type ToolGroup = 'send' | 'create' | 'organize' | 'cleanup';

export const GROUPS: { id: ToolGroup; label: string }[] = [
  { id: 'send', label: 'Send' },
  { id: 'create', label: 'Create' },
  { id: 'organize', label: 'Organize' },
  { id: 'cleanup', label: 'Clean up' },
];

export interface ToolDef {
  id: string;
  group: ToolGroup;
  label: string;
  /** False while the tool is only a placeholder. */
  built: boolean;
  /** One sentence for the placeholder. */
  blurb: string;
  /** The options the placeholder shows (disabled), as label and example value. */
  mock?: { label: string; value: string }[];
  /** The words on the placeholder's main button. */
  action?: string;
}

export const TOOLS: ToolDef[] = [
  { id: 'email', group: 'send', label: 'Email', built: true, blurb: '' },
  { id: 'print', group: 'send', label: 'Print', built: true, blurb: '' },
  { id: 'share', group: 'send', label: 'Share by cloud drive', built: false, blurb: 'Copy photos, videos or folders into OneDrive, iCloud Drive or Google Drive so someone can get them by link.',
    mock: [{ label: 'Cloud drive', value: 'OneDrive' }, { label: 'Folder name', value: 'Rome, June 2019' }, { label: 'Size', value: 'As they are' }], action: 'Copy to the drive' },
  { id: 'clipboard', group: 'send', label: 'Copy to clipboard', built: false, blurb: 'Copy the photos so they can be pasted into another program.',
    mock: [{ label: 'Copy as', value: 'Files' }], action: 'Copy' },
  { id: 'open-with', group: 'send', label: 'Open with…', built: false, blurb: 'Open the photos or videos in another program, such as an editor.',
    mock: [{ label: 'Program', value: 'Paint' }], action: 'Open' },
  { id: 'slideshow', group: 'send', label: 'Slideshow', built: false, blurb: 'Show the photos full screen, one after another.',
    mock: [{ label: 'Seconds per photo', value: '5' }, { label: 'Order', value: 'By date' }], action: 'Start' },

  { id: 'collage', group: 'create', label: 'Collage', built: false, blurb: 'Put several photos on one page from a template and save it as a picture.',
    mock: [{ label: 'Template', value: 'Four across' }, { label: 'Page size', value: 'A4, portrait' }, { label: 'Gap', value: 'Small' }], action: 'Save collage…' },
  { id: 'gif', group: 'create', label: 'Animated GIF', built: false, blurb: 'Turn photos into a short looping animation.',
    mock: [{ label: 'Seconds per photo', value: '0.5' }, { label: 'Longest side', value: '720 pixels · about 2.4 MB' }, { label: 'Order', value: 'By date, then back (bounce)' }], action: 'Save animation…' },
  { id: 'frames', group: 'create', label: 'Frames from a video', built: false, blurb: 'Save still pictures from a video as photos.',
    mock: [{ label: 'Frames', value: 'One every 10 seconds' }, { label: 'Save in', value: 'Next to the video' }], action: 'Save frames' },
  { id: 'edit', group: 'create', label: 'Edit', built: false, blurb: 'Crop, rotate and adjust a photo. Your original is never changed.',
    mock: [{ label: 'Crop', value: 'Free' }, { label: 'Rotate', value: 'None' }, { label: 'Brightness', value: '0' }], action: 'Save a copy' },

  { id: 'tags', group: 'organize', label: 'Tags', built: true, blurb: '' },
  { id: 'dates', group: 'organize', label: 'Fix dates', built: false, blurb: 'Give photos with a missing or wrong date a better one. The file itself is not changed.',
    mock: [{ label: 'Date', value: '14 June 2019' }, { label: 'Time', value: 'Keep the time' }], action: 'Set the date' },
  { id: 'hide', group: 'organize', label: 'Hide', built: false, blurb: 'Take photos out of Browse without deleting their files. Hidden photos can be shown again.',
    mock: [{ label: 'Hide', value: 'These photos' }], action: 'Hide' },
  { id: 'move', group: 'organize', label: 'Copy or move to folders', built: false, blurb: 'Copy or move the files into a folder layout, for example by year and month.',
    mock: [{ label: 'Into', value: 'D:\\Photos' }, { label: 'Layout', value: 'Year, then month' }, { label: 'Copy or move', value: 'Copy' }], action: 'Copy the files' },

  { id: 'duplicates', group: 'cleanup', label: 'Duplicates', built: true, blurb: '' },
  { id: 'bursts', group: 'cleanup', label: 'Bursts', built: false, blurb: 'Find quick runs of shots of the same moment, and keep the best frame of each.',
    mock: [{ label: 'Found', value: '71 bursts, 1,096 photos' }, { label: 'Keep', value: 'The sharpest frame' }], action: 'Hide the others…' },
  { id: 'thumbnails', group: 'cleanup', label: 'Thumbnails and cache', built: false, blurb: 'Remove thumbnails left behind by folders you took out, or clear them all to rebuild later.',
    mock: [{ label: 'Space used', value: '1.2 GB' }, { label: 'Left behind', value: '310 MB' }], action: 'Clean up' },
];

/** The tools of one tab, in order. Duplicates is left out while the duplicate check is switched off. */
export function toolsIn(group: ToolGroup, features: { dupes: boolean } = { dupes: true }): ToolDef[] {
  return TOOLS.filter((t) => t.group === group && (t.id !== 'duplicates' || features.dupes));
}

export function toolById(id: string): ToolDef | null {
  return TOOLS.find((t) => t.id === id) ?? null;
}

/** The tool a tab opens at: the first one that is built. */
export function firstTool(group: ToolGroup, features: { dupes: boolean } = { dupes: true }): ToolDef {
  const list = toolsIn(group, features);
  return list.find((t) => t.built) ?? list[0];
}

/** Tools in Clean up look after the whole library; the others work on the photos chosen in "Working on". */
export const worksOnWholeLibrary = (group: ToolGroup): boolean => group === 'cleanup';

export const isToolGroup = (id: string): id is ToolGroup => GROUPS.some((g) => g.id === id);
