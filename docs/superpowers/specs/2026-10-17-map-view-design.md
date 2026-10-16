# Pixie: map view (design)

Date: 2026-10-17. Builds on the shell and date tree, location in details (tile service, Leaflet) and videos. GitHub issue #3.

## Context

The user wants a map that spans all photos and lets them drill in to see the photos. Decision (user): **map on top, photos below**. The map fills the top of the centre pane; the grid underneath always shows the photos and videos inside the part of the map currently visible.

## Goals

- Clicking **Map** in the left tree shows a map of every geotagged photo and video, grouped into numbered clusters that split as you zoom in.
- The grid below, its day headings and the date tree reflect only what is inside the visible map area.
- Selecting, double-clicking (viewer) and the details pane work exactly as in the normal library.
- Responsive on a library of at least 100,000 located items.

## Non-goals

Place search, drawing areas, offline maps, showing a photo's thumbnail on its pin, heat maps, editing locations.

## Behaviour

- **Entering and leaving.** The tree's **Map** entry (currently a disabled placeholder) enters map mode. Clicking it again, or the **Close map** button on the map, returns to the normal library (bounds cleared). Folder entries and All photos stay in map mode and filter the map and the grid together; Dates nodes scroll the grid within the visible area. The map's last centre and zoom are remembered between runs; the first time, it fits all located items.
- **Filters.** The existing All | Photos | Videos and folder filters apply to the map: pins are exactly the items the grid could show.
- **Layout.** The centre pane splits into map (top, 45% of the pane, minimum 220 px) and grid (the rest). The split is fixed in v1.
- **Clusters.** Computed in the front end after each pan or zoom (`moveend`), not per frame. A cluster shows its item count; a single item is a pin. Clicking a cluster zooms to fit its members. Clicking a single pin, or a cluster that cannot split because its members share (almost) the same coordinates, zooms to level 17 centred on it. The grid then shows what is there.
- **Grid link.** The visible map bounds, rounded to 5 decimals, become the grid's bounding-box filter. Pan and zoom changes are debounced (200 ms) so the grid does not refetch on every frame. Selection clears when the bounds change (as it does for a sort or folder change).
- **No-location note.** A small overlay on the map says "N items have no location" (the count of items in the current filter without GPS). With no located items at all, an empty-state message replaces the map ("No photos with a location yet").
- **Offline.** Tiles fail quietly exactly as in the details map ("Map unavailable"); clusters still draw on the grey background and the grid still works.

## Backend

- `GeoPoint { id, lat, lon }` and `list_geo_points(folder_id?, kind?) -> Vec<GeoPoint>`: every non-missing item with valid GPS, honouring the folder and kind filters. About 200 KB for 6,000 items; fetched when map mode opens and after `scan-done`, not per pan.
- `PhotoQuery` gains `bounds: Option<Bounds>` where `Bounds { south, west, north, east }`. `list_photos`, `count_photos`, `list_day_groups` accept it. A box with `west > east` crosses the antimeridian and matches `lon >= west OR lon <= east`. Latitudes are clamped to [-90, 90]; items without GPS never match a bounds filter. Row filtering on top of the existing ordered indexes, no new index in v1; measure on a synthetic 100,000-item library and add an index only if a page takes more than about 50 ms.
- No schema change (GPS columns exist and are sanitised).

## Front end

- `View` gains `bounds: Bounds | null`; everything that threads `kind` today threads `bounds` (refresh, live refresh, details effect, grid, viewer, App guards).
- Pure, unit-tested modules: Web Mercator projection (`project(lat, lon, zoom)`), `clusterPoints(points, zoom, cellPx)` returning clusters (count, centroid, member ids or bounds, single id when count is 1), `boundsOfView(mapBounds)` (wrapping longitudes, clamping latitudes), `clusterTarget(cluster, ...)` (what a click should do).
- `PhotoMap.svelte`: Leaflet map using the existing tile template; renders cluster markers as `L.divIcon`s (at most a few hundred per view), removes old markers on every update, observes its container size, guards against stale async results.
- Tree: the Map entry becomes active; `App.svelte` holds `mode: 'library' | 'map'`.

## Error handling

- `list_geo_points` failure: inline message in the map area; the grid below still works with no bounds filter.
- Items whose GPS is invalid never reach the map (already nulled at scan time).
- Rapid pan/zoom: only the latest bounds is applied (token guard, as in Details).

## Testing

- Rust: bounds filter (inside/outside, edges inclusive, antimeridian, poles, items without GPS excluded, combined with kind and folder, counts and day groups agree with listing); `list_geo_points` (GPS only, excludes missing, honours filters); a timing check on a synthetic 100,000-item library.
- UI: projection round trips; clustering (counts conserved, a cluster splits with zoom, identical coordinates stay together, empty input, single point); bounds wrapping across 180 and clamping at the poles; cluster click target; debounce helper.
- Manual on the real library: whole-library fit, drill into a cluster, grid follows the map, filters, back to library, offline.

## Review Focus

- Antimeridian (Fiji, Alaska's Aleutians) and polar bounds.
- Many items at the same coordinates (a home address).
- 100,000 located items: clustering time on zoom, marker count, grid paging under a bounds filter.
- Pan/zoom storms: stale responses, marker leaks, tile request pile-up.
- Filter or mode changes while the map is open (kind, folder, leaving the map).
- The grid's selection, viewer and details pane under a bounds-filtered view.

## Known limitation

OpenStreetMap's public tile servers are not intended for heavy use by distributed apps. Fine for development and personal use; a released Pixie needs a different tile source (commercial or free-tier provider, or self-hosted). It is one URL template plus a User-Agent in `pixie-geo`, and applies to the details map as well.
