# Pixie: location in the details pane (design)

Date: 2026-10-16. Builds on `2026-10-16-shell-and-date-tree-design.md` (details pane, GPS already stored and sanitised).

## Context

The user asked for more features: videos, and a small map in the details pane when a photo has GPS, plus naming the place when it is a well-known landmark (Louvre, Bellagio, Colosseum). Videos are a separate, larger piece and get their own spec afterwards. This spec covers the location part only.

Decision (user): contacting online services is fine. Coordinates of the *selected* photo are sent to OpenStreetMap services.

## Goals

- Details pane shows a small map with a pin for any photo with valid GPS.
- Under the map, show the landmark name when the point is a known place, plus a "town, region, country" line.
- Degrade quietly when offline: no popups, coordinates still shown.

## Non-goals

Bulk geocoding / search by place, the full-library map (separate piece), offline maps, videos.

## Design

### Tiles
- Fetched by the Rust backend, not the webview: OSM tile servers require an identifying User-Agent and the webview would also need a loosened CSP.
- Command `get_tile(z, x, y) -> bytes`, with a disk cache under the app cache dir (`tiles/{z}/{x}/{y}.png`). User-Agent `Pixie/<version> (+https://github.com/pm100/pixie)`. Zoom limited to 0-19. Requests limited to a small concurrency (2).
- Frontend uses Leaflet with a custom tile layer that turns tile URLs into `get_tile` calls and shows them as blob URLs. OSM attribution is displayed.
- The same command will serve the later full Map view.

### Place names
- Command `place_for(lat, lon) -> Option<Place>` where `Place { landmark: Option<String>, locality: Option<String> }`.
- **Changed during implementation (live test):** Nominatim reverse lookup returns the nearest object of any kind, so the Colosseum point gave a pedestrian way, the Louvre a clock, and the Bellagio a clinic inside it. It is used for `locality` only (`zoom=14`, `addressdetails=1`, `format=jsonv2`). The public Overpass server was also unreliable (502s/timeouts), so it was not used.
- `landmark` is the nearest Wikipedia article from MediaWiki `list=geosearch` (radius 200 m requested, accepted if within 150 m), with any trailing "(disambiguator)" removed. "Has an encyclopedia article" is the working definition of well known. Checked live: Colosseum, Eiffel Tower, Bellagio resolve; the Louvre pyramid point resolves to "Pavillon de l'Horloge" (a part of the Louvre); mid-ocean gives nothing.
- Privacy: the selected photo's coordinates go to OpenStreetMap (tiles, Nominatim) and Wikipedia.
- `locality` is "city/town/village, state, country" from the address block, omitting missing parts and collapsing repeats.
- Cache: SQLite table `places(key TEXT PRIMARY KEY, landmark TEXT, locality TEXT, fetched_at INTEGER)`, key = `lat,lon` rounded to 4 decimals (about 11 m). Misses (no result) are cached too. Network failures are not cached.
- Rate limit: at most one Nominatim request per second, enforced in the backend. Only called for the selected photo.

### Details pane
- When the selected photo has GPS: Leaflet map (about 220 px tall, zoom 15, pin, pan/zoom allowed), then the landmark name (bold) and locality line.
- The map and place lookup start when the photo is selected; a stale response for a previously selected photo is discarded.
- Offline / failed lookup: show map area as "Map unavailable" and the coordinates; no error dialog.
- Multi-select or no GPS: no map.

## Error handling
- Tile fetch failure: the tile is left blank; no retry storm (single retry on next view).
- Nominatim errors / timeouts (5 s): no landmark line; not cached.

## Testing
- Rust: response parsing (Colosseum-like historic, Bellagio-like hotel, plain street, empty/no-result), key rounding, cache hit skips the network (via a fake fetcher), rate limiter spacing.
- UI: pure function that turns a `Place` into display lines; stale-response discard logic.
- Manual on real library: a GPS photo shows map and name; offline shows the fallback.

## Open items resolved while writing
- Leaflet is added as an npm dependency (bundled, no CDN).
- Nominatim is a best-effort source: landmark coverage depends on OSM tagging.
