<script lang="ts">
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount } from 'svelte';
  import { getPhoto, type Photo } from './api';
  import { debounce } from './debounce';
  import { basename, formatDateTime } from './format';
  import { boundsOfView, clusterPoints, clusterTarget, nearestWorldLon, progressZoom, type Bounds, type GeoPoint, type MapView } from './geo';
  import { posterUrl, thumbUrl, tileTemplate } from './url';

  let { points, initial, resetKey, onbounds, onmove, onpin, onopenphoto, onclose }: {
    points: GeoPoint[];
    initial: MapView | null;
    /** Changes when the user asks for the whole map again. */
    resetKey: number;
    onbounds: (b: Bounds | null) => void;
    onmove: (v: MapView) => void;
    /** A single pin was clicked: its photo. */
    onpin: (p: Photo) => void;
    /** The photo in the pin popup was clicked: open it. */
    onopenphoto: (p: Photo) => void;
    onclose: () => void;
  } = $props();

  let el: HTMLDivElement;
  let unavailable = $state(false);
  let map: L.Map | undefined;
  let markers: L.LayerGroup | undefined;
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
    };
  });

  // The user clicked Map again: back to the whole map.
  let lastReset: number | null = null; // the first run only records the starting key
  $effect(() => {
    const k = resetKey;
    if (lastReset === null) { lastReset = k; return; }
    if (k === lastReset) return;
    lastReset = k;
    map?.closePopup();
    fitAll();
  });

  // New points (the map opened, a filter changed, a scan finished): fit the first time, then redraw.
  $effect(() => {
    points;
    if (!map) return;
    if (!positioned) fitAll();
    redraw();
  });
</script>

<div class="wrap">
  <div class="map" bind:this={el}></div>
  <button class="close" onclick={onclose} title="Back to the library">✕ Close map</button>
  {#if points.length === 0}
    <div class="note">No photos with a location yet.</div>
  {/if}
  {#if unavailable}<div class="note off">Map unavailable</div>{/if}
  <div class="attr">© OpenStreetMap contributors</div>
</div>

<style>
  .wrap { position: relative; height: 100%; min-height: 0; isolation: isolate; /* keep Leaflet's z-indexes inside the map */ }
  .map { height: 100%; background: #e8e8e8; }
  .close, .attr { position: absolute; z-index: 500; background: rgba(255, 255, 255, .88); border-radius: 4px; font-size: 12px; padding: 3px 8px; }
  .close { top: 8px; right: 8px; border: 1px solid #ccc; cursor: pointer; }
  .attr { right: 8px; bottom: 4px; color: #666; padding: 1px 6px; }
  .note { position: absolute; z-index: 500; inset: 0; display: grid; place-items: center; pointer-events: none; color: #555; font-size: 14px; }
  :global(.pm) { background: none; border: 0; }
  :global(.pm .cl) { width: 100%; height: 100%; box-sizing: border-box; display: grid; place-items: center; border-radius: 50%; background: rgba(47, 111, 237, .85); color: #fff; font: 600 13px system-ui, sans-serif; border: 2px solid #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, .35); }
  :global(.pm .pin) { width: 100%; height: 100%; box-sizing: border-box; border-radius: 50%; background: #d33; border: 3px solid #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, .5); cursor: pointer; }
  :global(.pm .cl) { cursor: pointer; }
  :global(.pop) { cursor: pointer; width: 180px; }
  :global(.pop img) { display: block; width: 180px; height: 135px; object-fit: cover; border-radius: 4px; background: #2a2a30; }
  :global(.pop .noimg) { width: 180px; height: 135px; border-radius: 4px; background: #2a2a30; color: #bbb; display: grid; place-items: center; font-size: 28px; }
  :global(.pop .cap) { margin-top: 6px; font-size: 13px; font-weight: 600; word-break: break-all; }
  :global(.pop .when) { font-size: 12px; color: #666; }
</style>
