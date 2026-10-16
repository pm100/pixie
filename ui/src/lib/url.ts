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
