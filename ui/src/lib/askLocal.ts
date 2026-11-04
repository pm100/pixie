/**
 * Help without the server. When a question cannot be answered by Ask Pixie (the day's questions are used up, there is no connection, the
 * server is down) the panel answers from these topics, which are part of the app: short, written for a person, each with the words people
 * use for it and the place a button can lead to. They are less tailored than an answer written for the question, but never a dead end.
 * askLocal.test.ts checks that every built tool has a topic and that every place exists.
 */
import type { PhotoState } from './ask';

/** What the built-in help needs to know about the photos (the hint for the model is not used here). */
type Photos = Pick<PhotoState, 'picked' | 'chosen' | 'workingOn'>;
import { GO_TO_TARGETS } from './askGoTo';
import { plural } from './format';

export interface Topic {
  id: string;
  /** What the question may say; a phrase of two words counts more than one word. Matched without endings ("tagging" meets "tag"). */
  keywords: string[];
  text: string;
  goTo: string | null;
  /** The steps start from photos being chosen: the answer says what the person has now. */
  needsPhotos?: boolean;
}

export const TOPICS: Topic[] = [
  { id: 'pick', keywords: ['pick', 'select', 'choose', 'multiple', 'several', 'many', 'circle', 'star', 'ctrl', 'shift', 'picked'],
    text: "Picking marks photos to work on together.\n1. In **Browse**, hover a photo's top-left corner and click the circle.\n2. Or **Ctrl-click** it (**Cmd-click** on a Mac). **Shift-click** picks a run of photos.\n3. The **Picked** button shows only your picked photos; **Clear** takes them all off.\nSend, Create and Organize then work on them (**Working on: Picked**).", goTo: 'browse' },
  { id: 'email', needsPhotos: true, keywords: ['email', 'mail', 'attach', 'outlook', 'send photo', 'send video', 'video'],
    text: "To email photos or videos:\n1. In **Browse**, choose one photo or video with a click, or pick several (the circle in a photo's top-left corner).\n2. Open **Send**, then **Email**.\n3. Press the button: your mail program opens with them attached.\nThe Email screen shows how big they are and what is offered if they are too big for one message. **Videos** always go exactly as they are.", goTo: 'send-email' },
  { id: 'print', needsPhotos: true, keywords: ['print', 'printer', 'paper', 'page', 'per page', 'contact sheet'],
    text: "To print:\n1. Pick the photos in **Browse**.\n2. Open **Send**, then **Print**.\n3. Choose the paper and the photos per page (1, 2, 4 or 6).\n4. Check the preview, then press **Print...** to open the print dialog.", goTo: 'send-print' },
  { id: 'mosaic', needsPhotos: false, keywords: ['photomosaic', 'photo mosaic', 'mosaic', 'tiles', 'made of photos', 'made of many photos'],
    text: "To make a photomosaic, choose two things: the photo to rebuild, and the photos to build it from.\n1. In **Browse**, choose the photo to rebuild.\n2. Pick (or tag) the photos to build it from, or filter **Browse** to them. Only pictures are used; more photos make a better mosaic.\n3. Open **Create**, then **Photomosaic**. **Tiles from** at the top says which photos are used: **Picked**, **Filtered** (what Browse shows) or a tag.\n4. Drag the photo and use **Zoom** to choose the part, then set the paper and tile size.\n5. Press **Build**, then **Save**.", goTo: 'create-mosaic' },
  { id: 'collage', needsPhotos: true, keywords: ['collage', 'montage', 'layout', 'poster', 'arrange'],
    text: "To make a collage:\n1. Pick the photos in **Browse** (or tag them and use the tag).\n2. Open **Create**, then **Collage**.\n3. Drag photos from the list onto the page, or press **Layout** to fill the page for you (press again for another arrangement).\n4. Move and resize them by hand, then press **Save**.", goTo: 'create-collage' },
  { id: 'tag', needsPhotos: true, keywords: ['tag', 'label', 'add tag', 'tagging', 'name photo', 'organise', 'organize', 'categorise', 'group photo'],
    text: "To tag photos:\n1. Pick them in **Browse**.\n2. Open **Organize**, then **Tags**.\n3. Press the **+** button and type in **Find or create a tag**.\n4. Choose a tag from the list, or **+ Create tag** for a new one.\nYour tags then show in **Browse**, under **Your tags**, and filter the grid.", goTo: 'organize-tags' },
  { id: 'untag', needsPhotos: true, keywords: ['remove tag', 'delete tag', 'take off tag', 'untag', 'wrong tag'],
    text: "To take a tag off photos:\n1. Set **Working on** to the photos.\n2. Open **Organize**, then **Tags**.\n3. Press the small **x** on the tag, under **Common tags**.\nThe tag stays on other photos.", goTo: 'organize-tags' },
  { id: 'autotags', keywords: ['auto tag', 'automatic tag', 'beach', 'dog', 'what is in', 'recognise', 'recognize', 'object'],
    text: "Auto tags are labels Pixie works out from the picture itself, such as Beach or Dog.\nThey are listed in **Browse** under **Auto tags**. The **Auto tags** checkbox in **Settings, Picture analysis** turns them on and off. Videos get none yet.", goTo: 'settings-analysis' },
  { id: 'people', keywords: ['face', 'people', 'person', 'name', 'who', 'recognition'],
    text: "Pixie finds the faces in your photos on this computer and groups similar ones.\n1. Open the **People** tab.\n2. Name each group.\nNamed people then appear in **Browse** under **People**. The switch is the **People** checkbox in **Settings, Picture analysis**: off stops the work and hides the tab, and keeps what was found.", goTo: 'people' },
  { id: 'duplicates', keywords: ['duplicate', 'copies', 'same photo', 'identical', 'twice', 'double', 'clean up', 'cleanup', 'delete duplicate', 'remove duplicate'],
    text: "To tidy duplicates:\n1. Open **Clean up**, then **Duplicates**.\n2. In each group, the copy marked **Keep** is the one Browse shows; choose another with the circle beside it.\n3. Remove the others there. They go to the Recycle Bin.", goTo: 'cleanup-duplicates' },
  { id: 'dates', keywords: ['date', 'dates', 'missing date', 'wrong date', 'no date', 'when taken', 'timeline'],
    text: "To fix missing or wrong dates, open **Clean up**, then **Missing dates**.\n1. It finds photos with no date or a wrong one.\n2. Choose which dates count as missing, and when the file's date stands in.", goTo: 'cleanup-missing-dates' },
  { id: 'sync', keywords: ['sync', 'onedrive', 'icloud', 'dropbox', 'cloud', 'backup', 'back up', 'copy to folder', 'share online', 'import', 'export'],
    text: "The sync folder collects copies of your photos, ready to share.\n1. In **Import/Export**, choose one of your photo folders as the sync folder.\n2. Press **Sync now**: photos from your other folders are copied in, filed by year and date. Nothing is overwritten.\n3. Put that folder in OneDrive, iCloud Drive or similar to share it.\nIn **Settings, Photo folders**, each folder has a box saying whether it feeds the sync folder.", goTo: 'transfer-sync' },
  { id: 'private', keywords: ['private', 'hide from sync', 'not share', 'keep secret', 'confidential'],
    text: "To keep a photo out of the sync folder:\n1. Choose the photo in **Browse**.\n2. Tick **Private** in its details on the right.\n**Browse, Sync, Private** lists them.", goTo: 'browse' },
  { id: 'folders', keywords: ['folder', 'add folder', 'remove folder', 'directory', 'location of photos', 'scan', 'rescan', 'new photos'],
    text: "To change the folders Pixie looks at:\n1. Open **Settings, Photo folders**.\n2. Add a folder, or remove one (its files are never changed or deleted).\nPixie notices new and changed files in folders on this computer by itself.", goTo: 'settings-folders' },
  { id: 'network', keywords: ['network', 'nas', 'drive', 'disconnected', 'unplugged', 'rescan network'],
    text: "Folders on the network are scanned again from time to time.\n1. Open **Settings, Photo folders**.\n2. Under **Keeping up to date**, tick **Check folders on the network**.\n3. Choose the **Time between checks**.", goTo: 'settings-folders' },
  { id: 'trips', keywords: ['trip', 'holiday', 'vacation', 'travel', 'home', 'away', 'journey'],
    text: "Trips are runs of days away from home.\n1. Open **Settings, Trips**.\n2. Say where home is.\n3. Set how far counts as away, the fewest nights, and the size of a stop.\nTrips then appear in **Browse**.", goTo: 'settings-trips' },
  { id: 'map', keywords: ['map', 'gps', 'place', 'where taken', 'location'],
    text: "To use the map:\n1. In **Browse**, tick **Display** under **Map**.\n2. Tick **Filter** to show only the photos in the part of the map you can see.\nPhotos with no location are not on the map.", goTo: 'browse' },
  { id: 'viewer', keywords: ['viewer', 'open photo', 'full size', 'full screen', 'zoom', 'double click', 'pop out', 'popout', 'separate window'],
    text: "Double-click a photo in **Browse**, or choose it and press **Enter**, to open it.\nIn **Settings, Viewing** you choose whether it opens in the full window or in a separate window.", goTo: 'settings-viewing' },
  { id: 'filter', keywords: ['filter', 'search', 'find photo', 'look for', 'year', 'month', 'camera', 'recent', 'browse', 'narrow'],
    text: "To narrow down what Browse shows:\n1. Click a row in the list on the left (People, Trips, Your tags, Auto tags, Cameras, When, Folders).\n2. The filters show as pills above the grid: press the cross to clear one, or **Clear all**.\n**Last 30 days** is a shortcut: change how many days it means in **Settings, Browsing**.", goTo: 'browse' },
  { id: 'emailsetup', keywords: ['email limit', 'size limit', 'mail limit', 'too big', 'too large', 'email account', 'smtp', 'gmail'],
    text: "Open **Settings, Email** to set the **Size limit of one message**.\nWhen photos are bigger than the limit, Pixie offers to send them smaller or in several emails.", goTo: 'settings-email' },
  { id: 'processing', keywords: ['processing', 'slow', 'busy', 'background', 'pause', 'progress', 'analysis', 'analyse', 'analyze', 'cpu', 'fan', 'battery'],
    text: "While Pixie studies your photos (Auto tags, faces, duplicates), the **Processing** button at the top right shows how far along it is.\nPress it to see each job and to pause or resume. In **Settings, Picture analysis** you can turn Auto tags and People off.", goTo: 'settings-analysis' },
  { id: 'privacy', keywords: ['privacy', 'private data', 'upload', 'server', 'internet', 'safe', 'secure', 'sent', 'what does ask', 'cloud ai'],
    text: "Your pictures never leave your computer, and neither do the names of your tags, people, trips or folders.\nAsk Pixie sends only what you type, Pixie's settings, your language and your system, to a server, to get an answer.\n**Settings, Ask Pixie** lists exactly what is and is not sent, and turns it off.", goTo: 'settings-ask' },
  { id: 'notyet', keywords: ['text bigger', 'bigger text', 'text size', 'font', 'slideshow', 'gif', 'animation', 'animated', 'edit', 'rotate', 'crop', 'frames', 'hide photo', 'fix date', 'burst', 'thumbnail', 'clipboard', 'open with', 'dark mode', 'dark theme', 'theme', 'language', 'translate'],
    text: "Pixie does not have that yet.\nSlideshow, Animated GIF, Frames from a video, Edit, Fix dates, Hide, Copy or move to folders, Bursts and Thumbnails and cache are listed in their tabs, greyed out, to show what is planned.", goTo: null },
  { id: 'askpixie', keywords: ['ask pixie', 'help', 'assistant', 'chatbot', 'turn off help', 'questions left', 'limit', 'quota', 'today'],
    text: "Ask Pixie answers how-to questions and changes settings for you (you confirm each change).\nIt answers a limited number of questions a day; after that, or without a connection, this built-in help answers.\nYou can turn it off in **Settings, Ask Pixie**.", goTo: 'settings-ask' },
  { id: 'about', keywords: ['about', 'version', 'licence', 'license', 'notice', 'open source', 'credits', 'update'],
    text: "**Settings, About** shows the version of Pixie and the licences of everything it uses.", goTo: 'settings-about' },
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

/** One line about what the person has, for a topic that works on photos: it says what is already done. */
export function photosLine(photos: Photos): string {
  if (photos.workingOn === 'tag') return 'The tools are working on the photos with one of your tags, so you can go straight to the tool.';
  if (photos.picked > 0) return `You have **${plural(photos.picked, 'photo')}** picked, so you can go straight to the tool.`;
  if (photos.chosen) return 'You have a photo chosen: the tools use it by themselves (**Working on: Current**), so you can go straight to the tool.';
  return 'Nothing is picked or chosen yet, so start with the first step.';
}

/** What the help topics say about a question: the best topic, and a second if it fits nearly as well; null when nothing fits. */
export function localHelp(question: string, photos?: Photos): LocalHelp | null {
  const asked = new Set(words(question));
  const ranked = TOPICS.map((topic, order) => ({ topic, order, score: score(topic, asked) })).filter((r) => r.score > 0).sort((a, b) => b.score - a.score || a.order - b.order);
  if (ranked.length === 0) return null;
  const chosen = ranked.filter((r, i) => i === 0 || (i === 1 && r.score >= ranked[0].score * 0.75));
  const goTo = chosen[0].topic.goTo;
  // A question about one thing (a video, a photo) is not about the photos that happen to be picked.
  const aboutOne = /\b(videos?|a photo|one photo|this photo|that photo|the photo|a picture|one picture)\b/i.test(question);
  const lead = photos && chosen[0].topic.needsPhotos && !aboutOne ? `${photosLine(photos)}\n` : '';
  return { text: lead + chosen.map((r) => r.topic.text).join('\n\n'), topics: chosen.map((r) => r.topic.id), goTo: goTo !== null && (GO_TO_TARGETS as readonly string[]).includes(goTo) ? goTo : null };
}
