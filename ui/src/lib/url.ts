export type ThumbClass = 'grid' | 'view';

export function thumbUrl(
  cls: ThumbClass,
  id: number,
  mtime: number,
  size: number,
  isWindows: boolean = /Windows/i.test(navigator.userAgent),
  retry = 0,
): string {
  const base = isWindows ? 'http://pixie.localhost' : 'pixie://localhost';
  return `${base}/${cls}/${id}?v=${mtime}-${size}${retry ? `&r=${retry}` : ''}`;
}

/** Leaflet tile URL template served by the backend through the pixie: scheme. */
export function tileTemplate(isWindows: boolean = /Windows/i.test(navigator.userAgent)): string {
  const base = isWindows ? 'http://pixie.localhost' : 'pixie://localhost';
  return `${base}/tile/{z}/{x}/{y}`;
}

const base = (isWindows: boolean) => (isWindows ? 'http://pixie.localhost' : 'pixie://localhost');
const win = () => /Windows/i.test(navigator.userAgent);

export function videoUrl(id: number, mtime: number, size: number, isWindows: boolean = win()): string {
  return `${base(isWindows)}/video/${id}?v=${mtime}-${size}`;
}

export function posterUrl(id: number, mtime: number, size: number, isWindows: boolean = win(), retry = 0): string {
  return `${base(isWindows)}/poster/${id}?v=${mtime}-${size}${retry ? `&r=${retry}` : ''}`;
}

/** A square picture of one face (the backend cuts it from the photo); `size` is 96 (default), 160 or 320 pixels. */
export function faceUrl(id: number, isWindows: boolean = win(), size?: 96 | 160 | 320): string {
  return `${base(isWindows)}/face/${id}${size ? `?s=${size}` : ''}`;
}
