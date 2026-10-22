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
export const placeFor = (lat: number, lon: number) => invoke<Place | null>('place_for', { lat, lon, lang: userLang() });

export const savePoster = (id: number, bytes: Uint8Array) => invoke<void>('save_poster', { id, bytes: Array.from(bytes) });
export const openViewerWindow = () => invoke<void>('open_viewer_window');
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
/** Name a person; an empty name clears it. */
export const renamePerson = (id: number, name: string | null) => invoke<void>('rename_person', { id, name });
export const hidePerson = (id: number) => invoke<void>('hide_person', { id });
export const mergePeople = (from: number, into: number) => invoke<void>('merge_people', { from, into });
/** "Not this person": the face leaves its person and is not grouped automatically again. */
export const rejectFace = (faceId: number) => invoke<void>('reject_face', { faceId });
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
export const suggestFolders = () => invoke<Suggestion[]>('suggest_folders');

/** What `scanFolders` did: the folders' ids, and whether they joined a scan that was already running. */
export type ScanStart = { folder_ids: number[]; queued: boolean };
/** Add folders and scan them together; folders added while a scan runs join it instead of being left unscanned. */
export const scanFolders = (paths: string[]) => invoke<ScanStart>('scan_folders', { paths });

/** How many thumbnails are still being prepared. */
export const previewsPending = () => invoke<number>('previews_pending');

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

// ---- the Faces tab ----
import type { FacePerson, FaceTile } from './people';
/** Everyone, most faces first (hidden people only when asked). */
export const listFacePeople = (includeHidden: boolean) => invoke<FacePerson[]>('list_face_people', { includeHidden });
/** A person's faces: best first, or the most doubtful (least like the rest) first. */
export const facesOfPerson = (personId: number, limit?: number, doubtfulFirst?: boolean) => invoke<FaceTile[]>('faces_of_person', { personId, limit, doubtfulFirst });
/** Faces marked "not this person" and not given to anyone since. */
export const rejectedFaces = (limit?: number) => invoke<FaceTile[]>('rejected_faces', { limit });
export const moveFace = (faceId: number, personId: number) => invoke<void>('move_face', { faceId, personId });
export const moveFaces = (faceIds: number[], personId: number) => invoke<void>('move_faces', { faceIds, personId });
export const rejectFaces = (faceIds: number[]) => invoke<void>('reject_faces', { faceIds });
/** Start a new person from a face; returns their id. */
export const newPersonFromFace = (faceId: number, name: string | null) => invoke<number>('new_person_from_face', { faceId, name });
export const unhidePerson = (id: number) => invoke<void>('unhide_person', { id });
