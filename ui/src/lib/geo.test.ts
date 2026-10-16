import { describe, expect, it } from 'vitest';
import { boundsOfView, clusterPoints, clusterTarget, inBounds, nearestWorldLon, parseMapView, progressZoom, project, sameBounds, type GeoPoint } from './geo';

const pt = (id: number, lat: number, lon: number): GeoPoint => ({ id, lat, lon });
const world = { south: -90, west: -180, north: 90, east: 180 };

describe('project', () => {
  it('maps the equator/prime meridian to the middle of the world square', () => {
    expect(project(0, 0, 0)).toEqual({ x: 128, y: 128 });
    expect(project(0, -180, 3).x).toBe(0);
    expect(project(0, 180, 3).x).toBe(256 * 8);
    expect(project(85.05112878, 0, 0).y).toBeCloseTo(0, 3);
    expect(project(-85.05112878, 0, 0).y).toBeCloseTo(256, 3);
  });
  it('clamps latitudes beyond the Mercator limit instead of returning infinity', () => {
    expect(Number.isFinite(project(90, 0, 4).y)).toBe(true);
    expect(Number.isFinite(project(-90, 0, 4).y)).toBe(true);
  });
  it('doubles with each zoom level', () => {
    expect(project(10, 20, 5).x).toBeCloseTo(project(10, 20, 4).x * 2, 6);
  });
});

describe('inBounds', () => {
  const plain = { south: 10, west: 20, north: 30, east: 40 };
  const wrap = { south: -20, west: 170, north: -10, east: -170 };
  it('includes the edges', () => {
    expect(inBounds(10, 20, plain)).toBe(true);
    expect(inBounds(30, 40, plain)).toBe(true);
    expect(inBounds(30.0001, 30, plain)).toBe(false);
    expect(inBounds(20, 19.9999, plain)).toBe(false);
  });
  it('handles a box that crosses the antimeridian', () => {
    expect(inBounds(-15, 175, wrap)).toBe(true);
    expect(inBounds(-15, -175, wrap)).toBe(true);
    expect(inBounds(-15, 0, wrap)).toBe(false);
  });
});

describe('boundsOfView', () => {
  it('rounds to 5 decimals', () => {
    expect(boundsOfView(10.1234567, 20.1234567, 30.1234567, 40.1234567)).toEqual({ south: 10.12346, west: 20.12346, north: 30.12346, east: 40.12346 });
  });
  it('wraps longitudes that left the +-180 range into a crossing box', () => {
    const b = boundsOfView(-20, 170, -10, 190)!; // Leaflet reports 190 east of the line
    expect(b.west).toBe(170);
    expect(b.east).toBe(-170);
  });
  it('a view 360 degrees or wider covers every longitude', () => {
    expect(boundsOfView(-50, -400, 60, 400)).toEqual({ south: -50, west: -180, north: 60, east: 180 });
  });
  it('a view that ends exactly on the 180 line keeps 180 as its east edge', () => {
    expect(boundsOfView(0, 100, 10, 180)).toEqual({ south: 0, west: 100, north: 10, east: 180 });
  });
  it('clamps latitudes and rejects nonsense', () => {
    expect(boundsOfView(-95, 0, 95, 10)).toEqual({ south: -90, west: 0, north: 90, east: 10 });
    expect(boundsOfView(Number.NaN, 0, 1, 1)).toBeNull();
    expect(boundsOfView(5, 0, 1, 1)).toBeNull();
  });
  it('sameBounds compares values, including null', () => {
    expect(sameBounds(null, null)).toBe(true);
    expect(sameBounds({ south: 1, west: 2, north: 3, east: 4 }, { south: 1, west: 2, north: 3, east: 4 })).toBe(true);
    expect(sameBounds({ south: 1, west: 2, north: 3, east: 4 }, null)).toBe(false);
    expect(sameBounds({ south: 1, west: 2, north: 3, east: 5 }, { south: 1, west: 2, north: 3, east: 4 })).toBe(false);
  });
});

describe('clusterPoints', () => {
  it('no points, no clusters', () => {
    expect(clusterPoints([], 5, world)).toEqual([]);
  });

  it('a single point is a pin with its id', () => {
    const [c] = clusterPoints([pt(7, 48.86, 2.35)], 5, world);
    expect(c).toMatchObject({ count: 1, id: 7, stacked: false });
    expect(c.lat).toBeCloseTo(48.86, 6);
  });

  it('nearby points group at a low zoom and split at a high one', () => {
    const pts = [pt(1, 48.86, 2.35), pt(2, 48.866, 2.356)]; // about 800 m apart
    expect(clusterPoints(pts, 4, world).map((c) => c.count)).toEqual([2]);
    expect(clusterPoints(pts, 17, world).map((c) => c.count).sort()).toEqual([1, 1]);
  });

  it('identical coordinates stay together at every zoom and are marked stacked', () => {
    const pts = [pt(1, 10, 10), pt(2, 10, 10), pt(3, 10, 10)];
    for (const z of [2, 10, 19]) {
      const cs = clusterPoints(pts, z, world);
      expect(cs).toHaveLength(1);
      expect(cs[0]).toMatchObject({ count: 3, stacked: true });
    }
  });

  it('keeps every point in view exactly once and drops the ones outside', () => {
    const pts: GeoPoint[] = [];
    for (let i = 0; i < 2000; i++) pts.push(pt(i, -60 + (i % 50) * 2.4, -170 + Math.floor(i / 50) * 8));
    for (const z of [2, 5, 9]) {
      const total = clusterPoints(pts, z, world).reduce((s, c) => s + c.count, 0);
      expect(total).toBe(2000);
    }
    const box = { south: 0, west: 0, north: 40, east: 60 };
    const expected = pts.filter((p) => inBounds(p.lat, p.lon, box)).length;
    const got = clusterPoints(pts, 6, box).reduce((s, c) => s + c.count, 0);
    expect(got).toBeGreaterThanOrEqual(expected); // a margin around the view is clustered too
    expect(got).toBeLessThan(2000);
  });

  it('a cluster reports its extent and a centroid inside it', () => {
    const pts = [pt(1, 10, 10), pt(2, 10.1, 10.1), pt(3, 10.05, 10.2)];
    const [c] = clusterPoints(pts, 3, world);
    expect(c.count).toBe(3);
    expect(c.south).toBe(10);
    expect(c.north).toBe(10.1);
    expect(c.west).toBe(10);
    expect(c.east).toBe(10.2);
    expect(c.lat).toBeGreaterThan(c.south);
    expect(c.lat).toBeLessThan(c.north);
    expect(c.id).toBeNull();
  });

  it('works across the antimeridian box', () => {
    const wrapBox = { south: -30, west: 170, north: 0, east: -170 };
    const pts = [pt(1, -15, 175), pt(2, -15, -175), pt(3, -15, 0)];
    const got = clusterPoints(pts, 3, wrapBox).reduce((s, c) => s + c.count, 0);
    expect(got).toBe(2);
  });

  it('handles 100,000 points', () => {
    const pts: GeoPoint[] = [];
    for (let i = 0; i < 100_000; i++) pts.push(pt(i, -60 + (i % 1000) * 0.12, -170 + Math.floor(i / 1000) * 3.4));
    const cs = clusterPoints(pts, 4, world);
    expect(cs.reduce((s, c) => s + c.count, 0)).toBe(100_000);
    expect(cs.length).toBeLessThan(5000);
  });
});

describe('clusterTarget', () => {
  it('a pin or a stacked cluster zooms to street level on its position', () => {
    const [pin] = clusterPoints([pt(1, 5, 6)], 4, world);
    expect(clusterTarget(pin)).toEqual({ kind: 'zoom', lat: pin.lat, lon: pin.lon, zoom: 17 });
    const [stack] = clusterPoints([pt(1, 5, 6), pt(2, 5, 6)], 4, world);
    expect(clusterTarget(stack)).toMatchObject({ kind: 'zoom', zoom: 17 });
  });
  it('a splittable cluster fits its members', () => {
    const [c] = clusterPoints([pt(1, 10, 10), pt(2, 10.5, 10.5)], 2, world);
    expect(clusterTarget(c)).toEqual({ kind: 'fit', bounds: { south: 10, west: 10, north: 10.5, east: 10.5 } });
  });
});

describe('parseMapView', () => {
  it('accepts a sane saved view and rejects everything else', () => {
    expect(parseMapView({ lat: 10, lon: 20, zoom: 5 })).toEqual({ lat: 10, lon: 20, zoom: 5 });
    for (const v of [null, undefined, 3, 'x', {}, { lat: 'a', lon: 1, zoom: 2 }, { lat: 91, lon: 0, zoom: 3 },
      { lat: 0, lon: 181, zoom: 3 }, { lat: 0, lon: 0, zoom: 0 }, { lat: 0, lon: 0, zoom: 25 }, { lat: Number.NaN, lon: 0, zoom: 3 }]) {
      expect(parseMapView(v)).toBeNull();
    }
  });
});

describe('nearestWorldLon', () => {
  it('moves a longitude to the world copy nearest the view centre', () => {
    expect(nearestWorldLon(-179.95, 180)).toBeCloseTo(180.05, 6);
    expect(nearestWorldLon(178.44, 180)).toBeCloseTo(178.44, 6);
    expect(nearestWorldLon(-160, 225)).toBeCloseTo(200, 6);
    expect(nearestWorldLon(10, 0)).toBe(10);
    expect(nearestWorldLon(10, -355)).toBeCloseTo(-350, 6);
  });
});

describe('progressZoom', () => {
  it('always zooms in by at least one level, never past the maximum, and honours a deeper fit', () => {
    expect(progressZoom(15, 18, 19)).toBe(19); // the fit is already shallower than the view: still go in
    expect(progressZoom(12, 5, 19)).toBe(12);
    expect(progressZoom(25, 5, 19)).toBe(19);
    expect(progressZoom(18, 18, 19)).toBe(19);
    expect(progressZoom(19, 19, 19)).toBe(19);
  });
});

describe('stacked clusters', () => {
  it('items with a few metres of GPS jitter cannot be separated, so they count as stacked', () => {
    const jitter = [pt(1, 48.86, 2.35), pt(2, 48.86005, 2.35008), pt(3, 48.85997, 2.34995)];
    const [c] = clusterPoints(jitter, 10, world);
    expect(c.count).toBe(3);
    expect(c.stacked).toBe(true); // zooming in cannot reach a level where this splits into a useful view
    // At the deepest zoom a cell edge may split them into pieces; nothing is lost.
    expect(clusterPoints(jitter, 19, world).reduce((n, k) => n + k.count, 0)).toBe(3);
  });
  it('items hundreds of metres apart are not stacked', () => {
    const [c] = clusterPoints([pt(1, 48.86, 2.35), pt(2, 48.8645, 2.35)], 4, world);
    expect(c.count).toBe(2);
    expect(c.stacked).toBe(false);
  });
});
