import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Bounds, GeoPoint } from './geo';

export type { Bounds, GeoPoint } from './geo';

export interface Photo {
  id: number; folder_id: number; path: string; size: number; mtime: number;
  width: number | null; height: number | null; taken_at: number | null;
  camera: string | null; gps_lat: number | null; gps_lon: number | null; missing: boolean;
  kind: MediaKind; duration_ms: number | null;
}
export type MediaKind = 'photo' | 'video';
export interface Folder { id: number; path: string }
/** A watched folder with its number of live items, for the Settings page. */
export interface FolderInfo { id: number; path: string; count: number }
export type SortOrder = 'newest' | 'oldest';
/** What the library is showing: a sort order and optional folder, kind and map-area filters. */
export interface View {
  order: SortOrder; folderId: number | null; kind: MediaKind | null; bounds: Bounds | null;
  /** Snapshot: only items with an id up to this one. Keeps positions stable while a scan adds photos. */
  maxId: number | null;
  /** Only items taken at or after this time (unix seconds). */
  since: number | null;
  /** Only items taken before this time (unix seconds, exclusive). */
  until: number | null;
  /** Only items in the current tag / person filter result. */
  search: boolean;
}
/** Photos taken on one day ("YYYY-MM-DD"; null = no date), in listing order. */
export interface DayGroup { day: string | null; count: number; first_index: number }

export const listFolders = () => invoke<Folder[]>('list_folders');
export const listFolderInfo = () => invoke<FolderInfo[]>('list_folder_info');
export const removeFolder = (id: number) => invoke<void>('remove_folder', { id });
export const addFolder = (path: string) => invoke<number>('add_folder', { path });
/** `full` re-reads every file of the folder instead of only new and changed ones. */
export const startScan = (folderId: number | null, full = false) => invoke<void>('start_scan', { folderId, full });
/** Scan the folders on the network every `minutes` (null: never). Folders on this computer are watched, so need no setting. */
export const setNetworkRescan = (minutes: number | null) => invoke<void>('set_network_rescan', { minutes });
export const cancelScan = () => invoke<void>('cancel_scan');
export const listPhotos = (
  offset: number, limit: number, order: SortOrder = 'newest', folderId: number | null = null,
  kind: MediaKind | null = null, bounds: Bounds | null = null, maxId: number | null = null, since: number | null = null,
  until: number | null = null, search = false,
) => invoke<Photo[]>('list_photos', { offset, limit, order, folderId, kind, bounds, maxId, since, until, search });
export const countPhotos = (
  folderId: number | null = null, kind: MediaKind | null = null, bounds: Bounds | null = null, maxId: number | null = null,
  since: number | null = null, until: number | null = null, search = false,
) => invoke<number>('count_photos', { folderId, kind, bounds, maxId, since, until, search });
export const listDayGroups = (
  order: SortOrder = 'newest', folderId: number | null = null, kind: MediaKind | null = null,
  bounds: Bounds | null = null, maxId: number | null = null, since: number | null = null, until: number | null = null,
  search = false,
) => invoke<DayGroup[]>('list_day_groups', { order, folderId, kind, bounds, maxId, since, until, search });
/** The current highest photo id, to freeze a listing (see `View.maxId`). */
export const getMaxPhotoId = () => invoke<number>('max_photo_id');
export const getPhoto = (id: number) => invoke<Photo | null>('get_photo', { id });
export const listGeoPoints = (
  folderId: number | null = null, kind: MediaKind | null = null, since: number | null = null, until: number | null = null, search = false,
) => invoke<GeoPoint[]>('list_geo_points', { folderId, kind, since, until, search });
export const printPage = () => invoke<void>('print_page');

export interface Place { landmark: string | null; locality: string | null }
/** The user's language(s), e.g. "en-GB,en", so place names come back in their language. */
const userLang = () => (navigator.languages?.length ? navigator.languages.join(',') : navigator.language) || 'en';
/** `background`: work nobody is watching (naming trips), which waits its turn without cancelling the lookup for the photo on screen. */
export const placeFor = (lat: number, lon: number, background = false) => invoke<Place | null>('place_for', { lat, lon, lang: userLang(), background });

export const savePoster = (id: number, bytes: Uint8Array) => invoke<void>('save_poster', { id, bytes: Array.from(bytes) });
export const openViewerWindow = () => invoke<void>('open_viewer_window');
/** Opens a new email with the photo attached. 'outlook': a message is open in classic Outlook; 'share': the Windows Share sheet is. */
export const emailPhoto = (id: number) => invoke<'outlook' | 'newoutlook' | 'share'>('email_photo', { id, lang: userLang() });
export const openInDefaultPlayer = (id: number) => invoke<void>('open_in_default_player', { id });

// ---- email ----
export type MailSecurity = 'starttls' | 'ssl';
export interface MailAccount {
  display_name: string | null; email: string; provider: string; host: string; port: number; security: MailSecurity; limit_mb: number;
}
export interface MailInfo { account: MailAccount | null; has_password: boolean }
export interface Provider { id: string; label: string; host: string; port: number; security: MailSecurity; limit_mb: number; hint: string }
export interface Contact { id: number; nickname: string; name: string | null; email: string }
export interface NewContact { id: number | null; nickname: string; name: string | null; email: string }
/** A chosen recipient: a saved contact, or a plain address. */
export interface RecipientRef { contact_id: number | null; email: string | null }
export type SizeChoice = 'small' | 'medium' | 'original';
export interface SendEmailRequest {
  indexes: number[]; order: SortOrder; folder_id: number | null; kind: MediaKind | null; bounds: Bounds | null; max_id: number | null; since: number | null; until: number | null; search: boolean;
  to: RecipientRef[]; subject: string; message: string; size: SizeChoice;
}

export const listProviders = () => invoke<Provider[]>('list_providers');
export const getMailAccount = () => invoke<MailInfo>('get_mail_account');
/** `password` null or empty keeps the saved one. It goes to the system credential store only. */
export const saveMailAccount = (account: MailAccount, password: string | null) =>
  invoke<void>('save_mail_account', { account, password });
export const deleteMailAccount = () => invoke<void>('delete_mail_account');
export const sendTestEmail = () => invoke<void>('send_test_email');
export const listContacts = () => invoke<Contact[]>('list_contacts');
export const saveContact = (contact: NewContact) => invoke<Contact>('save_contact', { contact });
export const deleteContact = (id: number) => invoke<void>('delete_contact', { id });
export const photosAt = (indexes: number[], v: View) =>
  invoke<Photo[]>('photos_at', { indexes, order: v.order, folderId: v.folderId, kind: v.kind, bounds: v.bounds, maxId: v.maxId, since: v.since, until: v.until, search: v.search });
export const sendEmail = (request: SendEmailRequest) => invoke<string[]>('send_email', { request });

// ---- picture analysis (Auto tags) ----
import type { AnalysisStatus } from './analysis';
export const analysisStatus = () => invoke<AnalysisStatus>('analysis_status');
export const analysisSetPaused = (paused: boolean) => invoke<void>('analysis_set_paused', { paused });
export const analysisRetry = () => invoke<void>('analysis_retry');

// ---- tags ----
import type { TagCount } from './tagNav';
export const listTags = () => invoke<TagCount[]>('list_tags');
export type PhotoTag = { tag: string; score: number };
export const tagsFor = (photoId: number) => invoke<PhotoTag[]>('tags_for', { photoId });
/** Show only photos with this tag (null: off). List with `View.search`. */
export const setTagFilter = (tag: string | null) => invoke<void>('set_tag_filter', { tag });

// ---- people ----
import type { FaceInfo, PeopleStatus, PersonInfo } from './people';
export const peopleStatus = () => invoke<PeopleStatus>('people_status');
export const listPeople = () => invoke<PersonInfo[]>('list_people');
export const facesFor = (photoId: number) => invoke<FaceInfo[]>('faces_for', { photoId });
/** Name a person (which confirms them); an empty name clears it. A name somebody has means the same person: the two
 *  become one. Returns the id of the person kept. */
export const renamePerson = (id: number, name: string | null) => invoke<number>('rename_person', { id, name });
export const hidePerson = (id: number) => invoke<void>('hide_person', { id });
/** Show only photos with this person (null: off). List with `View.search`. */
// ---- first run ----
/** A place Pixie thinks holds photos (mirrors the Rust `Suggestion`). `included_in`: already part of another suggestion. */
export type Suggestion = { path: string; name: string; kind: 'known' | 'further'; ticked: boolean; note: string | null; included_in: string | null };
/** An approximate item count for one suggested folder. */
export type SuggestionCount = { path: string; n: number; capped: boolean };
export const onSuggestionCount = (f: (c: SuggestionCount) => void) => listen<SuggestionCount>('suggestion-count', (e) => f(e.payload));
export const onSuggestionFound = (f: (s: Suggestion) => void) => listen<Suggestion>('suggestion-found', (e) => f(e.payload));
export const onSuggestionsDone = (f: () => void) => listen('suggestions-done', () => f());
/** What the look-around is sniffing now ("your home folder", "your other drives"), for the spinner. */
export const onSuggestionLooking = (f: (what: string) => void) => listen<string>('suggestion-looking', (e) => f(e.payload));
/** The well-known photo places that exist; counts and further candidates then arrive as events (subscribe first). */
/** `deep` false: only the usual places are counted (adding a folder later); `suggestMore` then searches the rest on request. */
export const suggestFolders = (deep = true) => invoke<Suggestion[]>('suggest_folders', { deep });
/** The wider search, on request: other folders with pictures in them. The same events as `suggestFolders`. */
export const suggestMore = () => invoke<void>('suggest_more');

/** What `scanFolders` did: the folders' ids, and whether they joined a scan that was already running. */
export type ScanStart = { folder_ids: number[]; queued: boolean };
/** Add folders and scan them together; folders added while a scan runs join it instead of being left unscanned. */
export const scanFolders = (paths: string[]) => invoke<ScanStart>('scan_folders', { paths });

/** How many thumbnails are still being prepared. */


/** Debug builds only: whether the Developer page is offered. */
export const isDebugBuild = () => invoke<boolean>('is_debug_build');
/** Debug builds only: show the data folder in the file manager. */
export const openDataFolder = () => invoke<void>('open_data_folder');

/** A run of days spent away from home. `since` is inclusive and `until` exclusive, in unix seconds. */
export interface Trip { start: string; end: string; since: number; until: number; nights: number; days: number; photos: number; videos: number; name: string | null }
export const listTrips = (homeLat: number, homeLon: number, radiusKm: number, minNights: number) =>
  invoke<Trip[]>('list_trips', { homeLat, homeLon, radiusKm, minNights });
/** A place a trip stopped at; `first` and `last` are "YYYY-MM-DD". */
export interface Stop { lat: number; lon: number; first: string; last: string; days: number; items: number }
/** The stops of a trip in the order first visited, and the order of the visits (indexes into `stops`; a stop repeats if the trip came back). */
export interface Route { stops: Stop[]; path: number[] }
export const tripRoute = (since: number, until: number, homeLat: number, homeLon: number, radiusKm: number, stopKm: number) =>
  invoke<Route>('trip_route', { since, until, homeLat, homeLon, radiusKm, stopKm });
/** Where home probably is, as [latitude, longitude]; null when no photo has a position. */
export const suggestHome = () => invoke<[number, number] | null>('suggest_home');
export const renameTrip = (since: number, until: number, name: string) => invoke<void>('rename_trip', { since, until, name });
/** The tag and person the listings are really restricted to. */
export const activeFilters = () => invoke<{ tag: string | null; person: number | null }>('active_filters');
export const setPersonFilter = (person: number | null) => invoke<void>('set_person_filter', { person });

// ---- the People tab ----
import type { FaceOrder, FaceOverview, FacePerson, FaceTile, FaceView, Suggestion as FaceSuggestion, UnsortedFace } from './people';
/** How many groups, people, unsorted faces and hidden people there are. */
export const faceOverview = () => invoke<FaceOverview>('face_overview');
/** The cards of one section: groups to name, named people, or hidden ones. */
export const listFacePeople = (view: FaceView) => invoke<FacePerson[]>('list_face_people', { view });
/** A person's faces, `limit` from `offset`, in the order asked for. */
export const facesOfPerson = (personId: number, offset: number, limit: number, order: FaceOrder) => invoke<FaceTile[]>('faces_of_person', { personId, offset, limit, order });
/** Faces in no group, with who each looks like. */
export const unsortedFaces = (offset: number, limit: number) => invoke<UnsortedFace[]>('unsorted_faces', { offset, limit });
/** "This is the same person as ...": a whole group joins a person. */
export const assignGroup = (from: number, into: number) => invoke<void>('assign_group', { from, into });
/** Say faces are who they are filed under. */
export const confirmFaces = (faceIds: number[]) => invoke<void>('confirm_faces', { faceIds });
export const moveFaces = (faceIds: number[], personId: number) => invoke<void>('move_faces', { faceIds, personId });
/** "Not this person": the faces wait in Unsorted and are not guessed back. */
export const rejectFaces = (faceIds: number[]) => invoke<void>('reject_faces', { faceIds });
/** Start a new person from faces (optionally named); returns their id. */
export const newPersonFromFaces = (faceIds: number[], name: string | null) => invoke<number>('new_person_from_faces', { faceIds, name });
export const unhidePerson = (id: number) => invoke<void>('unhide_person', { id });
/** Cut a group that looks like several people into pieces; returns how many (0: it does not come apart). */
export const splitGroup = (id: number) => invoke<number>('split_group', { id });
/** Ignore faces, each on its own: out of every group and list but "Ignored". */
export const ignoreFaces = (faceIds: number[]) => invoke<void>('ignore_faces', { faceIds });
export const unignoreFaces = (faceIds: number[]) => invoke<void>('unignore_faces', { faceIds });
/** The faces that were ignored. */
export const ignoredFaces = (offset: number, limit: number) => invoke<FaceTile[]>('ignored_faces', { offset, limit });
/** Who faces (or a group) look like among the named people, best first: the candidates for "This is…". */
export const suggestForFaces = (faceIds: number[]) => invoke<FaceSuggestion[]>('suggest_for_faces', { faceIds });
export const suggestForGroup = (id: number) => invoke<FaceSuggestion[]>('suggest_for_group', { id });
/** Regroup the faces nobody has settled now; false when they kept changing meanwhile. */
export const tidyFaces = () => invoke<boolean>('tidy_faces');

import type { DupGroup, DupStatus } from './dupes';
/** Sets of identical files, most wasted space first; the copy to keep is first in each. */
export const duplicateGroups = () => invoke<DupGroup[]>('duplicate_groups');
import type { LoadProgress } from './analysisView';
import type { FacetCounts } from './facets';
/** The counts for the left-hand tree under the filters in force (see `facets.ts`). */
export const facetCounts = (
  view: Pick<View, 'folderId' | 'kind' | 'bounds' | 'maxId' | 'since' | 'until'>, tag: string | null, person: number | null, tripDates: boolean, tripWindows: [number, number][], recentSince: number,
) => invoke<FacetCounts>('facet_counts', { folderId: view.folderId, kind: view.kind, bounds: view.bounds, maxId: view.maxId, since: view.since, until: view.until, tag, person, tripDates, tripWindows, recentSince });
/** How far the analysis has got for the photos of these folders (the load screen's counters). */
export const loadProgress = (folderIds: number[]) => invoke<LoadProgress>('load_progress', { folderIds });
export const duplicateStatus = () => invoke<DupStatus>('duplicate_status');
/** Move extra copies to the Recycle Bin; returns the ids that were moved. A whole set is never removed. */
/** Show this copy in Browse for its set of identical files (and keep it when the others are removed). */
export const setDuplicateKeeper = (id: number) => invoke<void>('set_duplicate_keeper', { id });
export const removeDuplicates = (ids: number[]) => invoke<number[]>('remove_duplicates', { ids });
