<script lang="ts">
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount } from 'svelte';
  import { tileTemplate } from './url';

  // A map to point at where home is: click to move the marker. The circle shows how far "at home" reaches.
  let { home, radiusKm, onpick }: { home: { lat: number; lon: number } | null; radiusKm: number; onpick: (lat: number, lon: number) => void } = $props();

  let el: HTMLDivElement;
  let unavailable = $state(false);
  let map: L.Map | undefined;
  let marker: L.CircleMarker | undefined;
  let circle: L.Circle | undefined;
  let loaded = 0;

  onMount(() => {
    const start = home ? ([home.lat, home.lon] as [number, number]) : ([30, 10] as [number, number]);
    map = L.map(el, { attributionControl: false }).setView(start, home ? 8 : 2);
    const layer = L.tileLayer(tileTemplate(), { maxZoom: 19 }).addTo(map);
    layer.on('tileload', () => { loaded++; unavailable = false; });
    layer.on('tileerror', () => { if (loaded === 0) unavailable = true; });
    map.on('click', (e: L.LeafletMouseEvent) => onpick(e.latlng.lat, ((e.latlng.lng + 540) % 360) - 180));
    const ro = new ResizeObserver(() => map?.invalidateSize());
    ro.observe(el);
    return () => { ro.disconnect(); map?.remove(); map = undefined; marker = undefined; circle = undefined; };
  });

  // Keep the marker and circle in step with the home and radius; frame the circle when home moves.
  let framed: string | null = null;
  $effect(() => {
    const h = home, r = radiusKm;
    if (!map) return;
    if (!h) { marker?.remove(); circle?.remove(); marker = circle = undefined; return; }
    if (!marker) marker = L.circleMarker([h.lat, h.lon], { radius: 8, color: '#fff', weight: 2, fillColor: '#2a5bd7', fillOpacity: 1 }).addTo(map);
    else marker.setLatLng([h.lat, h.lon]);
    if (!circle) circle = L.circle([h.lat, h.lon], { radius: r * 1000, color: '#2a5bd7', weight: 1, fillOpacity: 0.08 }).addTo(map);
    else { circle.setLatLng([h.lat, h.lon]); circle.setRadius(r * 1000); }
    const key = `${h.lat},${h.lon}`;
    if (framed !== key) { framed = key; map.fitBounds(circle.getBounds(), { maxZoom: 10, padding: [20, 20] }); }
  });
</script>

<div class="wrap">
  <div class="map" bind:this={el} aria-label="Map: click to set where home is"></div>
  {#if unavailable}<div class="off">Map unavailable</div>{/if}
  <div class="attr">© OpenStreetMap contributors</div>
</div>

<style>
  .wrap { position: relative; margin: 0 0 8px; isolation: isolate; /* Leaflet panes use z-index up to 1000 */ }
  .map { height: 280px; border-radius: 6px; overflow: hidden; background: #e8e8e8; cursor: crosshair; }
  .off { position: absolute; inset: 0 0 18px 0; display: grid; place-items: center; color: #666; font-size: 13px; pointer-events: none; }
  .attr { font-size: 11px; color: #888; margin-top: 2px; }
</style>
