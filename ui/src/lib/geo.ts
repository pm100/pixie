export interface GeoPoint { id: number; lat: number; lon: number }
/** A latitude/longitude rectangle; west > east means it crosses the antimeridian. */
export interface Bounds { south: number; west: number; north: number; east: number }
export interface MapView { lat: number; lon: number; zoom: number }

const MAX_LAT = 85.0511287798; // the Web Mercator limit
const round5 = (v: number) => Math.round(v * 1e5) / 1e5;

/** Web Mercator world pixel position at a zoom level (256 px tiles). */
export function project(lat: number, lon: number, zoom: number): { x: number; y: number } {
  const size = 256 * 2 ** zoom;
  const la = Math.max(-MAX_LAT, Math.min(MAX_LAT, lat));
  const s = Math.sin((la * Math.PI) / 180);
  return {
    x: ((lon + 180) / 360) * size,
    y: (0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI)) * size,
  };
}

export function inBounds(lat: number, lon: number, b: Bounds): boolean {
  if (lat < b.south || lat > b.north) return false;
  return b.west <= b.east ? lon >= b.west && lon <= b.east : lon >= b.west || lon <= b.east;
}

const wrapLon = (v: number) => ((((v + 180) % 360) + 360) % 360) - 180;

/** Bounds as Leaflet reports them (longitudes may leave +-180) -> a valid, rounded box, or null. */
export function boundsOfView(south: number, west: number, north: number, east: number): Bounds | null {
  if (![south, west, north, east].every(Number.isFinite) || south > north) return null;
  const lat = (v: number) => Math.max(-90, Math.min(90, v));
  const s = round5(lat(south)), n = round5(lat(north));
  if (east - west >= 360) return { south: s, west: -180, north: n, east: 180 };
  const w = wrapLon(west);
  let e = wrapLon(east);
  if (e === -180 && east > west) e = 180; // wrapLon maps +180 to -180; an east edge on the line stays +180
  return { south: s, west: round5(w), north: n, east: round5(e) };
}

export function sameBounds(a: Bounds | null, b: Bounds | null): boolean {
  if (a === null || b === null) return a === b;
  return a.south === b.south && a.west === b.west && a.north === b.north && a.east === b.east;
}

export interface Cluster {
  count: number;
  /** Centroid. */
  lat: number;
  lon: number;
  /** The item's id when `count` is 1. */
  id: number | null;
  /** Extent of the members. */
  south: number; west: number; north: number; east: number;
  /** Several items at (almost) one spot: zooming in will not separate them. */
  stacked: boolean;
}

const MAX_ZOOM = 19; // the deepest level the tile source serves

function widen(b: Bounds, f: number): Bounds {
  const dLat = (b.north - b.south) * f, dLon = (b.east - b.west) * f;
  return {
    south: Math.max(-90, b.south - dLat), north: Math.min(90, b.north + dLat),
    west: Math.max(-180, b.west - dLon), east: Math.min(180, b.east + dLon),
  };
}

/**
 * Group points into pixel cells of `cellPx` at `zoom`. Only points inside `bounds` (plus a margin
 * so markers just off screen exist when the user pans) are clustered.
 */
export function clusterPoints(points: GeoPoint[], zoom: number, bounds: Bounds | null, cellPx = 60): Cluster[] {
  const area = bounds && !(bounds.west > bounds.east) ? widen(bounds, 0.25) : bounds;
  const cells = new Map<string, { n: number; sLat: number; sLon: number; id: number; s: number; w: number; nn: number; e: number }>();
  for (const p of points) {
    if (area && !inBounds(p.lat, p.lon, area)) continue;
    const { x, y } = project(p.lat, p.lon, zoom);
    const key = `${Math.floor(x / cellPx)}:${Math.floor(y / cellPx)}`;
    const c = cells.get(key);
    if (c) {
      c.n++; c.sLat += p.lat; c.sLon += p.lon;
      if (p.lat < c.s) c.s = p.lat;
      if (p.lat > c.nn) c.nn = p.lat;
      if (p.lon < c.w) c.w = p.lon;
      if (p.lon > c.e) c.e = p.lon;
    } else {
      cells.set(key, { n: 1, sLat: p.lat, sLon: p.lon, id: p.id, s: p.lat, w: p.lon, nn: p.lat, e: p.lon });
    }
  }
  return [...cells.values()].map((c) => ({
    count: c.n,
    lat: c.sLat / c.n,
    lon: c.sLon / c.n,
    id: c.n === 1 ? c.id : null,
    south: c.s, west: c.w, north: c.nn, east: c.e,
    stacked: c.n > 1 && fitsOneCellAtMaxZoom(c.s, c.w, c.nn, c.e, cellPx),
  }));
}

/** Members that still share one cell at the deepest zoom can never be separated by zooming in. */
function fitsOneCellAtMaxZoom(s: number, w: number, n: number, e: number, cellPx: number): boolean {
  const a = project(s, w, MAX_ZOOM), b = project(n, e, MAX_ZOOM);
  return Math.abs(b.x - a.x) < cellPx && Math.abs(b.y - a.y) < cellPx;
}

/** The copy of the world (longitude + k * 360) nearest the view centre, so markers show where the map is. */
export function nearestWorldLon(lon: number, centerLon: number): number {
  return lon + 360 * Math.round((centerLon - lon) / 360);
}

/** A click on a cluster must always get closer: at least one level in, never past the maximum. */
export function progressZoom(fitZoom: number, currentZoom: number, max = MAX_ZOOM): number {
  return Math.min(max, Math.max(fitZoom, currentZoom + 1));
}

export type ClusterTarget =
  | { kind: 'fit'; bounds: Bounds }
  | { kind: 'zoom'; lat: number; lon: number; zoom: number };

/** What clicking a cluster should do. */
export function clusterTarget(c: Cluster): ClusterTarget {
  if (c.count === 1 || c.stacked) return { kind: 'zoom', lat: c.lat, lon: c.lon, zoom: 17 };
  return { kind: 'fit', bounds: { south: c.south, west: c.west, north: c.north, east: c.east } };
}

/** A saved map position may be anything (or corrupt); only a sane one is used. */
export function parseMapView(v: unknown): MapView | null {
  if (typeof v !== 'object' || v === null) return null;
  const { lat, lon, zoom } = v as Record<string, unknown>;
  if (typeof lat !== 'number' || typeof lon !== 'number' || typeof zoom !== 'number') return null;
  if (![lat, lon, zoom].every(Number.isFinite)) return null;
  if (Math.abs(lat) > 90 || Math.abs(lon) > 180 || zoom < 1 || zoom > 19) return null;
  return { lat, lon, zoom };
}
