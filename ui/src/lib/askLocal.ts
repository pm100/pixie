/**
 * Help without the server. When a question cannot be answered by Ask Pixie (the day's questions are used up, there is no connection, the
 * server is down) the panel answers from these topics, which are part of the app: short, written for a person, each with the words people
 * use for it and the place a button can lead to. They are less tailored than an answer written for the question, but never a dead end.
 * askLocal.test.ts checks that every built tool has a topic and that every place exists.
 */
import { GO_TO_TARGETS } from './askGoTo';

export interface Topic {
  id: string;
  /** What the question may say; a phrase of two words counts more than one word. Matched without endings ("tagging" meets "tag"). */
  keywords: string[];
  text: string;
  goTo: string | null;
}

const PICK = 'To pick photos, move the mouse over a photo\'s top-left corner and click the circle that appears (or Ctrl-click, Cmd-click on a Mac). Shift-click picks a run of photos.';

export const TOPICS: Topic[] = [
  { id: 'pick', keywords: ['pick', 'select', 'choose', 'multiple', 'several', 'many', 'circle', 'star', 'ctrl', 'shift', 'picked'],
    text: `${PICK} Picked photos show a blue star, and the Picked button at the top shows only them. Send, Create and Organize work on the picked photos (Working on: Picked), or on all the photos with one of your tags.`, goTo: 'browse' },
  { id: 'email', keywords: ['email', 'mail', 'attach', 'outlook', 'send photo', 'send video', 'video'],
    text: `${PICK} Then open Send, choose Email, and press the button: your mail program opens with them attached. If they are too big for one email, Pixie offers smaller copies or several emails. Videos always go exactly as they are, so a video over the mail limit may be refused: share it another way, such as a cloud drive link.`, goTo: 'send-email' },
  { id: 'print', keywords: ['print', 'printer', 'paper', 'page', 'per page', 'contact sheet'],
    text: `${PICK} Then open Send, choose Print, and set the paper, the photos per page (1, 2, 4 or 6) and the margin. Check the preview and press Print... to open the print dialog.`, goTo: 'send-print' },
  { id: 'collage', keywords: ['collage', 'montage', 'mosaic', 'layout', 'poster', 'arrange'],
    text: 'Open Create, then Collage. The photos in Working on are listed on the right: drag them onto the page, or double-click them. Press Layout to have Pixie arrange them all to fill the page (press again for another arrangement), move and resize them by hand, and press Save to write a picture.', goTo: 'create-collage' },
  { id: 'tag', keywords: ['tag', 'label', 'add tag', 'tagging', 'name photo', 'organise', 'organize', 'categorise', 'group photo'],
    text: `${PICK} Then open Organize, choose Tags, type a tag and add it to all the photos being worked on. Your tags then appear in Browse, under Your tags, and filter the grid.`, goTo: 'organize-tags' },
  { id: 'untag', keywords: ['remove tag', 'delete tag', 'take off tag', 'untag', 'wrong tag'],
    text: 'Set Working on to the photos, then open Organize, Tags. The tags they all have are listed as Common tags: press the small x on a tag to take it off all of them. The tag stays on other photos.', goTo: 'organize-tags' },
  { id: 'autotags', keywords: ['auto tag', 'automatic tag', 'beach', 'dog', 'what is in', 'recognise', 'recognize', 'object'],
    text: 'Auto tags are labels Pixie works out from the picture itself, such as Beach or Dog. They are listed in Browse under Auto tags. They need the Auto tags checkbox on in Settings, Picture analysis. Videos get none yet.', goTo: 'settings-analysis' },
  { id: 'people', keywords: ['face', 'people', 'person', 'name', 'who', 'recognition'],
    text: 'Pixie finds the faces in your photos on this computer and groups similar ones. Open the People tab and name each group; named people then appear in Browse under People. The switch is the People checkbox in Settings, Picture analysis: turning it off stops the work and hides the People tab, and keeps what was found.', goTo: 'people' },
  { id: 'duplicates', keywords: ['duplicate', 'copies', 'same photo', 'identical', 'twice', 'double', 'clean up', 'cleanup', 'delete duplicate', 'remove duplicate'],
    text: 'Open Clean up, then Duplicates. Each group of identical files shows which copy is kept (the one Browse shows); choose another with the circle beside it. Removing the others is done there, by you, and they go to the Recycle Bin.', goTo: 'cleanup-duplicates' },
  { id: 'dates', keywords: ['date', 'dates', 'missing date', 'wrong date', 'no date', 'when taken', 'timeline'],
    text: 'Open Clean up, then Missing dates. It finds photos with no date or a wrong one, and lets you choose which dates count as missing and when the file\'s date stands in.', goTo: 'cleanup-missing-dates' },
  { id: 'sync', keywords: ['sync', 'onedrive', 'icloud', 'dropbox', 'cloud', 'backup', 'back up', 'copy to folder', 'share online', 'import', 'export'],
    text: 'Choose one of your photo folders as the sync folder (Import/Export). Sync now copies the photos from your other folders into it, filed by year and date, checking each copy and never overwriting a file. Put that folder in OneDrive, iCloud Drive or similar to share the copies. In Settings, Photo folders, each folder has a box saying whether it feeds the sync folder.', goTo: 'transfer-sync' },
  { id: 'private', keywords: ['private', 'hide from sync', 'not share', 'keep secret', 'confidential'],
    text: 'In Browse, choose a photo and tick Private in its details on the right. A private photo is never copied into the sync folder. Browse, Sync, Private lists them.', goTo: 'browse' },
  { id: 'folders', keywords: ['folder', 'add folder', 'remove folder', 'directory', 'location of photos', 'scan', 'rescan', 'new photos'],
    text: 'Open Settings, Photo folders. Add a folder to have Pixie list the photos and videos in it, or remove one to stop (the files are never changed or deleted). Pixie notices new and changed files in folders on this computer by itself.', goTo: 'settings-folders' },
  { id: 'network', keywords: ['network', 'nas', 'drive', 'disconnected', 'unplugged', 'rescan network'],
    text: 'Folders on the network are scanned again from time to time. In Settings, Photo folders, under Keeping up to date, tick "Check folders on the network" and choose the time between checks.', goTo: 'settings-folders' },
  { id: 'trips', keywords: ['trip', 'holiday', 'vacation', 'travel', 'home', 'away', 'journey'],
    text: 'Trips are runs of days away from home. Pixie needs to know where home is first: Settings, Trips. There you also set how far counts as away, the fewest nights, and the size of a stop. Trips then appear in Browse.', goTo: 'settings-trips' },
  { id: 'map', keywords: ['map', 'gps', 'place', 'where taken', 'location'],
    text: 'In Browse, tick Display under Map to see a map above the grid. Tick Filter to show only the photos in the part of the map you can see. Photos with no location are not on the map.', goTo: 'browse' },
  { id: 'viewer', keywords: ['viewer', 'open photo', 'full size', 'full screen', 'zoom', 'double click', 'pop out', 'popout', 'separate window'],
    text: 'Double-click a photo in Browse, or choose it and press Enter, to open it. In Settings, Viewing you choose whether it opens in the full window or in a separate window.', goTo: 'settings-viewing' },
  { id: 'filter', keywords: ['filter', 'search', 'find photo', 'look for', 'year', 'month', 'camera', 'recent', 'browse', 'narrow'],
    text: 'In Browse, click a row in the list on the left (People, Trips, Your tags, Auto tags, Cameras, When, Folders) to filter the grid. The filters show as pills above the grid, each with a cross to clear it, and Clear all. The "recent" shortcut means the last 30 days by default: change it in Settings, Browsing.', goTo: 'browse' },
  { id: 'emailsetup', keywords: ['email limit', 'size limit', 'mail limit', 'too big', 'too large', 'email account', 'smtp', 'gmail'],
    text: 'Open Settings, Email. There you set the size limit of one message. When the photos are bigger than the limit, Pixie offers to send them smaller or in several emails.', goTo: 'settings-email' },
  { id: 'processing', keywords: ['processing', 'slow', 'busy', 'background', 'pause', 'progress', 'analysis', 'analyse', 'analyze', 'cpu', 'fan', 'battery'],
    text: 'While Pixie studies your photos (Auto tags, faces, duplicates) the Processing button at the top right shows how far along it is. Press it to see each job and to pause or resume. In Settings, Picture analysis you can turn Auto tags and People off.', goTo: 'settings-analysis' },
  { id: 'privacy', keywords: ['privacy', 'private data', 'upload', 'server', 'internet', 'safe', 'secure', 'sent', 'what does ask', 'cloud ai'],
    text: 'Your pictures never leave your computer, and neither do the names of your tags, people, trips or folders. Ask Pixie sends only what you type, Pixie\'s settings, your language and your system to a server to get an answer. Settings, Ask Pixie lists exactly what is and is not sent, and turns it off.', goTo: 'settings-ask' },
  { id: 'notyet', keywords: ['text bigger', 'bigger text', 'text size', 'font', 'slideshow', 'gif', 'animation', 'animated', 'edit', 'rotate', 'crop', 'frames', 'hide photo', 'fix date', 'burst', 'thumbnail', 'clipboard', 'open with', 'dark mode', 'dark theme', 'theme', 'language', 'translate'],
    text: 'Pixie does not have that yet. Slideshow, Animated GIF, Frames from a video, Edit, Fix dates, Hide, Copy or move to folders, Bursts and Thumbnails and cache are listed in their tabs, greyed out, to show what is planned.', goTo: null },
  { id: 'askpixie', keywords: ['ask pixie', 'help', 'assistant', 'chatbot', 'turn off help', 'questions left', 'limit', 'quota', 'today'],
    text: 'Ask Pixie answers how-to questions and can change settings for you (you confirm each change). It can answer a limited number of questions a day; when they are used up, or there is no connection, this help answers from the guide inside Pixie. You can turn Ask Pixie off in Settings, Ask Pixie.', goTo: 'settings-ask' },
  { id: 'about', keywords: ['about', 'version', 'licence', 'license', 'notice', 'open source', 'credits', 'update'],
    text: 'Settings, About shows the version of Pixie and the licences of everything it uses.', goTo: 'settings-about' },
];

/** The words of a text without small words or endings, so "printing", "prints" and "print" meet. */
const STOP = new Set(('how do does did can could would should the a an and or of to in on for with my me i you your it is are was be this that these those what where when why which who ' +
  'pixie about from at as by if so there their them then than into onto just want need make get use using please show tell find some any all').split(' '));

function stem(word: string): string {
  if (word.length > 5 && word.endsWith('ing')) return word.slice(0, -3);
  if (word.length > 4 && word.endsWith('ed')) return word.slice(0, -2);
  if (word.length > 4 && word.endsWith('ies')) return `${word.slice(0, -3)}y`;
  if (word.length > 4 && /(ss|x|z|ch|sh)es$/.test(word)) return word.slice(0, -2);
  if (word.length > 3 && word.endsWith('s') && !word.endsWith('ss')) return word.slice(0, -1);
  return word;
}

const words = (text: string): string[] => (text.toLowerCase().match(/[a-z0-9]+/g) ?? []).filter((w) => !STOP.has(w)).map(stem);

/** How well a topic fits the question: each keyword found in it counts, a phrase of several words only if all its words are found. */
function score(topic: Topic, asked: Set<string>): number {
  let total = 0;
  for (const k of topic.keywords) {
    const parts = words(k.replace(/[^a-z0-9 ]/gi, ' '));
    if (parts.length > 0 && parts.every((p) => asked.has(p))) total += parts.length;
  }
  return total;
}

/** `topics`: the ids of the topics the text came from, best first. */
export interface LocalHelp { text: string; goTo: string | null; topics: string[] }

/** What the help topics say about a question: the best topic, and a second if it fits nearly as well; null when nothing fits. */
export function localHelp(question: string): LocalHelp | null {
  const asked = new Set(words(question));
  const ranked = TOPICS.map((topic, order) => ({ topic, order, score: score(topic, asked) })).filter((r) => r.score > 0).sort((a, b) => b.score - a.score || a.order - b.order);
  if (ranked.length === 0) return null;
  const chosen = ranked.filter((r, i) => i === 0 || (i === 1 && r.score >= ranked[0].score * 0.75));
  const goTo = chosen[0].topic.goTo;
  return { text: chosen.map((r) => r.topic.text).join('\n\n'), topics: chosen.map((r) => r.topic.id), goTo: goTo !== null && (GO_TO_TARGETS as readonly string[]).includes(goTo) ? goTo : null };
}
