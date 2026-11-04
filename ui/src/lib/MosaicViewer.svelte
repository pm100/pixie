<script lang="ts">
  /** The built mosaic, to zoom and pan like a map: it is cut into small pictures at several sizes, so even a huge one moves smoothly. */
  import L from 'leaflet';
  import 'leaflet/dist/leaflet.css';
  import { onMount } from 'svelte';
  import type { MosaicBuilt } from './api';
  import { mosaicTileTemplate } from './url';

  let { built }: { built: MosaicBuilt } = $props();

  let el: HTMLDivElement;
  let map: L.Map | undefined;
  let layer: L.TileLayer | undefined;

  // y runs down the page, as in a picture (the default map orientation has it running up)
  const PICTURE = L.extend({}, L.CRS.Simple, { transformation: new L.Transformation(1, 0, 1, 0) }) as L.CRS;

  /** The size of the picture in map units at zoom 0, where the whole of it is one small tile. */
  const unit = (b: MosaicBuilt) => ({ w: b.width / 2 ** (b.levels - 1), h: b.height / 2 ** (b.levels - 1) });

  function show(b: MosaicBuilt) {
    if (!map) return;
    layer?.remove();
    const area = bounds(b);
    layer = L.tileLayer(mosaicTileTemplate(`${b.key}-${b.built_at}`), { tileSize: 256, minNativeZoom: 0, maxNativeZoom: b.levels - 1, minZoom: -2, maxZoom: b.levels + 1, noWrap: true, bounds: area }).addTo(map);
    map.setMaxBounds(area.pad(0.1));
    map.setMinZoom(-2);
    map.setMaxZoom(b.levels + 1);
    map.fitBounds(area, { animate: false });
  }
  const bounds = (b: MosaicBuilt) => {
    const u = unit(b);
    return L.latLngBounds([0, 0], [u.h, u.w]);
  };

  onMount(() => {
    map = L.map(el, { crs: PICTURE, attributionControl: false, zoomSnap: 0.25, zoomControl: true });
    show(built);
    const ro = new ResizeObserver(() => map?.invalidateSize());
    ro.observe(el);
    return () => { ro.disconnect(); map?.remove(); map = undefined; };
  });

  // A new picture was built: show that one.
  $effect(() => { show(built); });
</script>

<div class="view" bind:this={el} role="img" aria-label="The finished mosaic, to zoom and pan"></div>

<style>
  .view { position: absolute; inset: 0; background: #2b2b2f; isolation: isolate; /* Leaflet panes use z-index up to 1000 */ }
</style>
