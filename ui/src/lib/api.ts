import { invoke } from '@tauri-apps/api/core';

export interface Photo {
  id: number; folder_id: number; path: string; size: number; mtime: number;
  width: number | null; height: number | null; taken_at: number | null;
  camera: string | null; gps_lat: number | null; gps_lon: number | null; missing: boolean;
  kind: MediaKind; duration_ms: number | null;
}
export type MediaKind = 'photo' | 'video';
export interface Folder { id: number; path: string }
export type SortOrder = 'newest' | 'oldest';
/** What the library is showing: a sort order and optional folder and kind filters. */
export interface View { order: SortOrder; folderId: number | null; kind: MediaKind | null }
/** Photos taken on one day ("YYYY-MM-DD"; null = no date), in listing order. */
export interface DayGroup { day: string | null; count: number; first_index: number }

export const listFolders = () => invoke<Folder[]>('list_folders');
export const addFolder = (path: string) => invoke<number>('add_folder', { path });
export const startScan = (folderId: number | null) => invoke<void>('start_scan', { folderId });
export const cancelScan = () => invoke<void>('cancel_scan');
export const listPhotos = (
  offset: number, limit: number, order: SortOrder = 'newest', folderId: number | null = null, kind: MediaKind | null = null,
) => invoke<Photo[]>('list_photos', { offset, limit, order, folderId, kind });
export const countPhotos = (folderId: number | null = null, kind: MediaKind | null = null) =>
  invoke<number>('count_photos', { folderId, kind });
export const listDayGroups = (order: SortOrder = 'newest', folderId: number | null = null, kind: MediaKind | null = null) =>
  invoke<DayGroup[]>('list_day_groups', { order, folderId, kind });
export const printPage = () => invoke<void>('print_page');

export interface Place { landmark: string | null; locality: string | null }
/** The user's language(s), e.g. "en-GB,en", so place names come back in their language. */
const userLang = () => (navigator.languages?.length ? navigator.languages.join(',') : navigator.language) || 'en';
export const placeFor = (lat: number, lon: number) => invoke<Place | null>('place_for', { lat, lon, lang: userLang() });

export const savePoster = (id: number, bytes: Uint8Array) => invoke<void>('save_poster', { id, bytes: Array.from(bytes) });
export const openInDefaultPlayer = (id: number) => invoke<void>('open_in_default_player', { id });
