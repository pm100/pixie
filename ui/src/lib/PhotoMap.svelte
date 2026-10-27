<script lang="ts">
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount, untrack } from 'svelte';
  import { getPhoto, type Photo, type Route } from './api';
  import { debounce } from './debounce';
  import { basename, formatDateTime } from './format';
  import { boundsOfView, clusterPoints, clusterTarget, greatCircle, nearestWorldLon, progressZoom, type Bounds, type GeoPoint, type MapView } from './geo';
  import { tripDates } from './trips';
  import { posterUrl, thumbUrl, tileTemplate } from './url';

  let { points, fitMap = 0, initial, route = null, stopNames = [], onbounds, onmove, onpin, onopenphoto }: {
    points: GeoPoint[];
    /** Changes whenever a choice is made in the browse tree: once the new points are in, the map shows all of them. */
    fitMap?: number;
    /** A chosen trip: its stops are numbered and arrows show the way from one to the next. */
    route?: Route | null;
    /** The name of each stop of `route`, where known. */
    stopNames?: (string | null)[];
    initial: MapView | null;
    onbounds: (b: Bounds | null) => void;
    onmove: (v: MapView) => void;
    /** A single pin was clicked: its photo. */
    onpin: (p: Photo) => void;
    /** The photo in the pin popup was clicked: open it. */
    onopenphoto: (p: Photo) => void;
  } = $props();

  let el: HTMLDivElement;
  let unavailable = $state(false);
  let map: L.Map | undefined;
  let markers: L.LayerGroup | undefined;
  let routeLayer: L.LayerGroup | undefined;
  let loaded = 0; // tiles shown since the last view change
  let errors = 0; // tiles that failed since the last view change
  // After a click on a pin the grid shows just that photo. While this is true the map reports no area
  // changes, whatever moves it (the zoom to the pin, an animation that was already running, a resize),
  // until the user acts on the map themselves.
  let focusActive = false;
  let positioned = false; // the view is set once: from the saved position or by fitting the points

  const emitBounds = debounce((b: Bounds | null) => onbounds(b), 200);
  const emitMove = debounce((v: MapView) => onmove(v), 500);

  function viewBounds(): Bounds | null {
    if (!map) return null;
    const b = map.getBounds();
    return boundsOfView(b.getSouth(), b.getWest(), b.getNorth(), b.getEast());
  }

  function redraw() {
    if (!map || !markers) return;
    markers.clearLayers(); // always remove the old ones: no leaks across pans
    const zoom = map.getZoom();
    const centerLon = map.getCenter().lng;
    for (const c of clusterPoints(points, zoom, viewBounds())) {
      // Draw on the copy of the world that is on screen (the view can cross the date line).
      const lon = nearestWorldLon(c.lon, centerLon);
      const html = c.count === 1 ? '<div class="pin"></div>' : `<div class="cl">${c.count > 9999 ? '10k+' : c.count}</div>`;
      const size = c.count === 1 ? 20 : c.count < 100 ? 34 : c.count < 1000 ? 42 : 50; // a pin gets a roomy click target
      const m = L.marker([c.lat, lon], {
        icon: L.divIcon({ html, className: 'pm', iconSize: [size, size] }),
        keyboard: false,
      });
      m.on('click', () => {
        if (!map) return;
        if (c.count === 1 && c.id !== null) { // one photo
          if (POPUP_ON_PIN) void showPin(c.id, c.lat, lon);
          else void focusPin(c.id, c.lat, lon);
          return;
        }
        const t = clusterTarget(c);
        const here = map.getZoom();
        if (t.kind === 'zoom') {
          map.setView([t.lat, lon], Math.max(here, t.zoom)); // never zoom out
        } else {
          const shift = lon - c.lon; // same world copy as the marker that was clicked
          const b = L.latLngBounds([t.bounds.south, t.bounds.west + shift], [t.bounds.north, t.bounds.east + shift]);
          map.setView(b.getCenter(), progressZoom(map.getBoundsZoom(b), here));
        }
      });
      markers.addLayer(m);
    }
  }

  /** Off: a click on a red dot zooms to that photo. Set to true to show a thumbnail popup instead (kept for later). */
  const POPUP_ON_PIN = false;

  /**
   * Click on one red dot: zoom to just that photo. The app narrows the grid to that spot and selects
   * the photo, so the grid and the details pane show it. Nothing reports bounds again (which would
   * widen the grid) until the user acts on the map.
   */
  async function focusPin(id: number, lat: number, lon: number) {
    if (!map) return;
    emitBounds.cancel(); // a report from an earlier pan must not undo the focus
    focusActive = true;
    map.setView([lat, lon], Math.max(map.getZoom(), 18));
    const p = await getPhoto(id).catch(() => null);
    if (p) onpin(p);
  }

  /** Click on one red dot: a small popup with that photo; click the picture to open it. */
  async function showPin(id: number, lat: number, lon: number) {
    const p = await getPhoto(id).catch(() => null);
    if (!p || !map) return;
    const box = document.createElement('div');
    box.className = 'pop';
    const img = document.createElement('img');
    img.alt = '';
    img.title = 'Open';
    img.src = p.kind === 'video' ? posterUrl(p.id, p.mtime, p.size) : thumbUrl('grid', p.id, p.mtime, p.size);
    img.onerror = () => {
      const ph = document.createElement('div');
      ph.className = 'noimg';
      ph.textContent = p.kind === 'video' ? '▶' : '⚠︎';
      img.replaceWith(ph);
    };
    const name = document.createElement('div');
    name.className = 'cap';
    name.textContent = basename(p.path); // file names are untrusted text
    const when = document.createElement('div');
    when.className = 'when';
    when.textContent = formatDateTime(p.taken_at);
    box.append(img, name, when);
    box.onclick = () => { map?.closePopup(); onopenphoto(p); };
    L.popup({ maxWidth: 240, autoPanPadding: [20, 20] }).setLatLng([lat, lon]).setContent(box).openOn(map);
    onpin(p);
  }

  /** The chosen trip: a line from stop to stop in the order visited, an arrowhead on each leg, and the stops numbered. */
  function drawRoute(): L.LatLngBounds | null {
    if (!map || !routeLayer) return null;
    routeLayer.clearLayers();
    if (!route || route.stops.length === 0) return null;
    const stops = route.stops;
    const all: L.LatLng[] = [];
    const lonAt = new Map<number, number>(); // where each stop is drawn, on the copy of the world the route is on
    let cur = stops[route.path[0] ?? 0].lon;
    lonAt.set(route.path[0] ?? 0, cur);
    all.push(L.latLng(stops[route.path[0] ?? 0].lat, cur));
    for (let k = 1; k < route.path.length; k++) {
      const a = stops[route.path[k - 1]], b = stops[route.path[k]];
      const pts = greatCircle([a.lat, cur], [b.lat, b.lon]);
      cur = pts[pts.length - 1][1];
      if (!lonAt.has(route.path[k])) lonAt.set(route.path[k], cur);
      for (const pt of pts) all.push(L.latLng(pt[0], pt[1]));
      L.polyline(pts, { color: ROUTE_COLOUR, weight: 3, opacity: 0.9, interactive: false }).addTo(routeLayer);
      const mid = Math.floor((pts.length - 1) / 2);
      const p1 = map.latLngToLayerPoint(pts[mid]), p2 = map.latLngToLayerPoint(pts[mid + 1]);
      const deg = (Math.atan2(p2.y - p1.y, p2.x - p1.x) * 180) / Math.PI;
      const at: [number, number] = [(pts[mid][0] + pts[mid + 1][0]) / 2, (pts[mid][1] + pts[mid + 1][1]) / 2];
      L.marker(at, {
        icon: L.divIcon({ html: `<div style="transform:rotate(${deg.toFixed(1)}deg)">▶</div>`, className: 'rt-arrow', iconSize: [20, 20] }),
        interactive: false, keyboard: false,
      }).addTo(routeLayer);
    }
    // No numbered stops (a round number on the map means a count of photos): only where the trip starts and where it ends.
    const first = route.path[0] ?? 0;
    const last = route.path[route.path.length - 1] ?? first;
    const flag = (i: number, kind: 'start' | 'finish', title: string) => {
      const st = stops[i];
      const when = tripDates({ start: st.first, end: st.last });
      L.marker([st.lat, lonAt.get(i) ?? st.lon], {
        icon: L.divIcon({ html: kind === 'start' ? START_FLAG : FINISH_FLAG, className: 'rt-flag', iconSize: [30, 38], iconAnchor: [6, 37] }),
        keyboard: false, zIndexOffset: 1000,
      }).bindTooltip(`${title}${stopNames[i] ? ` · ${stopNames[i]}` : ''} · ${when}`, { direction: 'right', offset: [12, -18] }).addTo(routeLayer!);
    };
    if (first === last) flag(first, 'start', 'Start and finish');
    else { flag(first, 'start', 'Start'); flag(last, 'finish', 'Finish'); }
    stops.forEach((st, i) => {
      const lon = lonAt.get(i) ?? st.lon;
      if (!all.some((q) => q.lat === st.lat && q.lng === lon)) all.push(L.latLng(st.lat, lon));
    });
    return L.latLngBounds(all);
  }

  /** The route: green, which the rest of the map does not use (clusters are blue, single photos red). */
  const ROUTE_COLOUR = '#2e9d4a';
  // The start and finish flags: a pole and a pennant (green), and a chequered flag. The pole's foot is the place.
  const START_FLAG = '<svg width="30" height="38" viewBox="0 0 30 38"><line x1="6" y1="37" x2="6" y2="3" stroke="#333" stroke-width="3" stroke-linecap="round"/><path d="M7 3 L27 3 L22 10.5 L27 18 L7 18 Z" fill="#2e9d4a" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>';
  const FINISH_FLAG = '<svg width="30" height="38" viewBox="0 0 30 38"><defs><pattern id="rt-chk" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/><rect width="4" height="4" fill="#222"/><rect x="4" y="4" width="4" height="4" fill="#222"/></pattern></defs><line x1="6" y1="37" x2="6" y2="3" stroke="#333" stroke-width="3" stroke-linecap="round"/><rect x="7" y="3" width="20" height="16" fill="url(#rt-chk)" stroke="#333" stroke-width="1.5"/></svg>';

  function fitAll() {
    if (!map || points.length === 0) return;
    let s = 90, n = -90, w = 180, e = -180;
    for (const p of points) {
      if (p.lat < s) s = p.lat;
      if (p.lat > n) n = p.lat;
      if (p.lon < w) w = p.lon;
      if (p.lon > e) e = p.lon;
    }
    map.fitBounds([[s, w], [n, e]], { padding: [30, 30], maxZoom: 14 });
    positioned = true;
  }

  onMount(() => {
    map = L.map(el, { attributionControl: false, minZoom: 2, worldCopyJump: false });
    markers = L.layerGroup().addTo(map);
    routeLayer = L.layerGroup().addTo(map);
    map.on('zoomend', () => { drawRoute(); });
    const layer = L.tileLayer(tileTemplate(), { maxZoom: 19 }).addTo(map);
    layer.on('tileload', () => { loaded++; unavailable = false; });
    layer.on('tileerror', (e) => {
      errors++;
      // A fast pan can leave tiles behind in the backend queue; try each failed tile once more shortly after.
      const img = e.tile as HTMLImageElement & { retried?: boolean };
      if (!img.retried) {
        img.retried = true;
        setTimeout(() => { if (img.isConnected) img.src = img.src.split('?')[0] + '?r=1'; }, 800);
      }
      // One or two failures are a busy queue; a run of them with nothing loaded means no network.
      if (loaded === 0 && errors >= 4) unavailable = true;
    });
    // `initial` is read once, at mount: a later change must not move the map under the user.
    if (initial) {
      map.setView([initial.lat, initial.lon], initial.zoom);
      positioned = true;
    } else {
      map.setView([20, 0], 2);
    }
    map.on('movestart', () => { loaded = 0; errors = 0; });
    map.on('moveend', () => {
      redraw();
      if (!focusActive) emitBounds(viewBounds());
      const c = map!.getCenter();
      emitMove({ lat: c.lat, lon: c.lng, zoom: map!.getZoom() });
    });
    // The user taking over (drag, wheel, zoom buttons, keys, touch) ends the focus; the next move reports as usual.
    const takeOver = () => { focusActive = false; };
    const inputs = ['pointerdown', 'wheel', 'keydown', 'touchstart'] as const;
    for (const ev of inputs) el.addEventListener(ev, takeOver, { capture: true, passive: true });
    // A click on empty map (not on a pin) also ends it, and puts the grid back on the visible area.
    map.on('click', () => {
      if (!focusActive) return;
      focusActive = false;
      emitBounds(viewBounds());
    });
    const ro = new ResizeObserver(() => map?.invalidateSize());
    ro.observe(el);
    if (!positioned) fitAll();
    redraw();
    emitBounds(viewBounds());
    return () => {
      for (const ev of inputs) el.removeEventListener(ev, takeOver, { capture: true });
      ro.disconnect();
      emitBounds.cancel();
      emitMove.cancel();
      map?.remove();
      map = undefined;
      routeLayer = undefined;
    };
  });

  // A trip was chosen (or cleared, or its stops named): draw it, and show all of it when it first appears.
  let routeShown: Route | null = null;
  $effect(() => {
    const r = route;
    stopNames;
    if (!map) return;
    const bounds = drawRoute();
    if (bounds && r !== routeShown) {
      map.closePopup();
      focusActive = false;
      map.fitBounds(bounds, { padding: [40, 40], maxZoom: 12 });
    }
    routeShown = r;
  });

  // A choice in the browse tree: when its points arrive, show all of them (the same as Recenter).
  let fitSeen = untrack(() => fitMap); // the value at mount: only later changes count
  let fitWanted = false;
  $effect(() => {
    if (fitMap !== fitSeen) {
      fitSeen = fitMap;
      fitWanted = true;
    }
  });

  // New points (the map opened, a filter changed, a scan finished): fit the first time, then redraw.
  $effect(() => {
    points;
    if (!map) return;
    if (!positioned || fitWanted) {
      fitWanted = false;
      map.closePopup();
      fitAll();
    }
    redraw();
  });
</script>

<div class="wrap">
  <div class="map" bind:this={el}></div>
  <button class="showall" onclick={() => { map?.closePopup(); fitAll(); }} disabled={points.length === 0} title="Centre the map on every photo and video shown, and zoom to fit them">Recenter</button>
  {#if points.length === 0}
    <div class="note">No photos with a location yet.</div>
  {/if}
  {#if unavailable}<div class="note off">Map unavailable</div>{/if}
  <div class="attr">© OpenStreetMap contributors</div>
</div>

<style>
  .wrap { position: relative; height: 100%; min-height: 0; isolation: isolate; /* keep Leaflet's z-indexes inside the map */ }
  .map { height: 100%; background: #e8e8e8; }
  .showall, .attr { position: absolute; z-index: 500; background: rgba(255, 255, 255, .88); border-radius: 4px; font-size: 12px; padding: 3px 8px; }
  .showall { top: 8px; right: 8px; border: 1px solid #ccc; cursor: pointer; font: inherit; font-size: 12px; color: #111; }
  .showall:disabled { color: #999; cursor: default; }
  .attr { right: 8px; bottom: 4px; color: #666; padding: 1px 6px; }
  .note { position: absolute; z-index: 500; inset: 0; display: grid; place-items: center; pointer-events: none; color: #555; font-size: 14px; }
  :global(.pm) { background: none; border: 0; }
  :global(.pm .cl) { width: 100%; height: 100%; box-sizing: border-box; display: grid; place-items: center; border-radius: 50%; background: rgba(47, 111, 237, .85); color: #fff; font: 600 13px system-ui, sans-serif; border: 2px solid #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, .35); }
  :global(.pm .pin) { width: 100%; height: 100%; box-sizing: border-box; border-radius: 50%; background: #d33; border: 3px solid #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, .5); cursor: pointer; }
  :global(.pm .cl) { cursor: pointer; }
  :global(.rt-flag) { background: none; border: 0; }
  :global(.rt-flag svg) { display: block; filter: drop-shadow(0 1px 2px rgba(0, 0, 0, .5)); }
  :global(.rt-arrow) { background: none; border: 0; }
  :global(.rt-arrow div) { width: 20px; height: 20px; display: grid; place-items: center; font-size: 15px; line-height: 1; color: #2e9d4a; text-shadow: 0 0 3px #fff, 0 0 3px #fff; }
  :global(.pop) { cursor: pointer; width: 180px; }
  :global(.pop img) { display: block; width: 180px; height: 135px; object-fit: cover; border-radius: 4px; background: #2a2a30; }
  :global(.pop .noimg) { width: 180px; height: 135px; border-radius: 4px; background: #2a2a30; color: #bbb; display: grid; place-items: center; font-size: 28px; }
  :global(.pop .cap) { margin-top: 6px; font-size: 13px; font-weight: 600; word-break: break-all; }
  :global(.pop .when) { font-size: 12px; color: #666; }
</style>
