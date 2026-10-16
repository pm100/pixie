<script lang="ts">
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount } from 'svelte';
  import { tileTemplate } from './url';

  let { lat, lon }: { lat: number; lon: number } = $props();

  let el: HTMLDivElement;
  let unavailable = $state(false);
  let map: L.Map | undefined;
  let marker: L.CircleMarker | undefined;
  let loaded = 0; // tiles shown for the current location

  onMount(() => {
    // No default attribution control: its link would navigate the webview. We show plain text below.
    map = L.map(el, { attributionControl: false }).setView([lat, lon], 15);
    const layer = L.tileLayer(tileTemplate(), { maxZoom: 19 }).addTo(map);
    layer.on('tileload', () => {
      loaded++;
      unavailable = false;
    });
    layer.on('tileerror', () => {
      if (loaded === 0) unavailable = true;
    });
    marker = L.circleMarker([lat, lon], { radius: 8, color: '#fff', weight: 2, fillColor: '#d33', fillOpacity: 1 }).addTo(map);
    // The pane can be resized by dragging the divider.
    const ro = new ResizeObserver(() => map?.invalidateSize());
    ro.observe(el);
    return () => {
      ro.disconnect();
      map?.remove();
      map = undefined;
    };
  });

  $effect(() => {
    const la = lat; // read first so the effect tracks them even before the map exists
    const lo = lon;
    if (map && marker) {
      loaded = 0;
      unavailable = false;
      map.setView([la, lo], 15);
      marker.setLatLng([la, lo]);
    }
  });
</script>

<div class="wrap">
  <div class="map" bind:this={el}></div>
  {#if unavailable}<div class="off">Map unavailable</div>{/if}
  <div class="attr">© OpenStreetMap contributors</div>
</div>

<style>
  .wrap { position: relative; margin: 0 0 8px; isolation: isolate; /* Leaflet panes use z-index up to 1000 */ }
  .map { height: 220px; border-radius: 6px; overflow: hidden; background: #e8e8e8; }
  .off { position: absolute; inset: 0 0 18px 0; display: grid; place-items: center; color: #666; font-size: 13px; pointer-events: none; }
  .attr { font-size: 11px; color: #888; margin-top: 2px; }
</style>
