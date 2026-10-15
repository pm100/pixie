import { invoke } from '@tauri-apps/api/core';

export interface Photo {
  id: number; folder_id: number; path: string; size: number; mtime: number;
  width: number | null; height: number | null; taken_at: number | null;
  camera: string | null; gps_lat: number | null; gps_lon: number | null; missing: boolean;
}
export interface Folder { id: number; path: string }

export const listFolders = () => invoke<Folder[]>('list_folders');
export const addFolder = (path: string) => invoke<number>('add_folder', { path });
export const startScan = (folderId: number) => invoke<void>('start_scan', { folderId });
export const cancelScan = () => invoke<void>('cancel_scan');
export const listPhotos = (offset: number, limit: number) => invoke<Photo[]>('list_photos', { offset, limit });
export const countPhotos = () => invoke<number>('count_photos');
export const printPage = () => invoke<void>('print_page');
