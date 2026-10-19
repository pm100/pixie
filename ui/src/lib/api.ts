import { invoke } from '@tauri-apps/api/core';
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
  /** Only items in the current search result (see `searchPhotos`). */
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

// ---- content analysis and search ----
import type { AnalysisStatus } from './analysis';
export const analysisStatus = () => invoke<AnalysisStatus>('analysis_status');
export const analysisSetPaused = (paused: boolean) => invoke<void>('analysis_set_paused', { paused });
export const analysisRetry = () => invoke<void>('analysis_retry');
/** Make `text` the current search and return how many photos match; blank text clears it. List with `View.search`. */
export const searchPhotos = (text: string) => invoke<number>('search_photos', { text });

// ---- tags ----
import type { TagCount } from './tagNav';
export const listTags = () => invoke<TagCount[]>('list_tags');
export type PhotoTag = { tag: string; score: number };
export const tagsFor = (photoId: number) => invoke<PhotoTag[]>('tags_for', { photoId });
/** Show only photos with this tag (null: off). List with `View.search`, like a text search. */
export const setTagFilter = (tag: string | null) => invoke<void>('set_tag_filter', { tag });
