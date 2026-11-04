# Photomosaic design

Issue: #112 (split from #12). Status: draft for approval, nothing built.

## Intent

Make one big picture out of many small photos. The person chooses a photo (or part of one) as the **target**; Pixie rebuilds it as a grid of small square tiles, each a photo from their own library, chosen so that from a distance the tiles add up to the target. The result is a print-quality picture file, made the same way and saved by the same rules as a Collage.

Why: it is a keepsake that uses the whole library, not a hand-picked few. The earlier limit on pick-list size was the obstacle; that is fixed (Tools list work, #79), and this feature does not need a pick list at all, because tiles come from the library or the Browse filters.

## Goals

- A **Photomosaic** tool in the Create tab, next to Collage.
- Target: one photo, cropped and zoomed to the part wanted, the same crop and zoom controls as a Collage frame.
- Tiles from the whole library, from what Browse currently shows, or from the picked photos. No limit on how many photos can be candidates.
- A live **draft** on screen that redraws as settings change, then an explicit **Build** that makes the full-size picture, which the person looks at and only then **Saves**.
- Paper size, orientation and resolution as in Collage; tile size in mm.
- Repeats allowed but spread apart, with a setting for how strict that is.
- A **Blend with target** slider, default 0.
- Works for libraries of tens of thousands of photos without freezing the window.

## Non-goals

- The mosaic is not added to the library automatically (same as Collage: save it into a watched folder if wanted).
- Tiles are square only. No hexagons, no mixed sizes.
- No per-photo include or exclude list beyond the three sources.
- No printing from inside the tool (the saved file prints like any photo).
- No video tiles.

## Anti-goals (a failure even if it technically works)

- Originals are never changed or deleted.
- The window must not freeze during indexing or building; both show progress and can be cancelled.
- A stale picture must never be saved as if it matched the current settings.
- The same photo must never sit next to itself.

## The flow

1. **Choose the target.** Pick the photo (the one chosen in Browse, or from the tool), then crop and zoom as in a Collage frame. The crop keeps the page's proportions.
2. **Choose the tile source.** Library, Browse filters (what Browse shows now), or Picked. The tool shows a line such as "About 3,200 tiles, from 18,000 photos." If there are fewer photos than make sense, it says so in words (it still works, with more repeats).
3. **Settings.** Paper size and orientation (as Collage), tile size (mm), repeats (how far apart, in tiles), Blend with target (0 to 100%, default 0).
4. **Live draft.** A small picture built from the existing grid thumbnails' colours, redrawn when the crop, tile size, source, repeats or blend change. It is for judging the look; it is not print quality.
5. **Build.** Makes the full-size picture at the paper size and resolution, by opening each chosen original once, cropping it to a centred square and placing it. Progress bar and Cancel. The result is kept in Pixie's cache folder; nothing is written where the person would look for it.
6. **Look.** The built picture opens in the tool with zoom and pan, so each tile can be checked at full sharpness.
7. **Save.** Opens the save dialog with the Collage rules below, then copies the built file there (instant, no rebuild).
8. Changing any setting marks the built picture out of date; Save is greyed out until it is rebuilt, and also while nothing has been built. A tooltip says why.

## Matching

- Each candidate photo has an **average colour**, taken from its existing grid thumbnail (never the original). It is computed once, stored in the library database keyed by photo id, modification time and size (so edits invalidate it), and reused by every later mosaic. The first run over a large library therefore takes a while (thumbnails missing from the cache are made on demand) and shows progress; later runs start straight away.
- Each photo also keeps a 12 × 12 miniature of its centre square, made from the same thumbnail, so the live draft shows real tiles rather than flat colours.
- Each cell of the target gets the candidate whose colour is nearest, compared in a perceptual colour space.
- **Repeats:** a photo may appear more than once, but a photo used in one cell is penalised for other cells near it. The "repeats" setting is the distance, in tiles, within which the same photo may not appear again (default chosen by the builder, with a minimum of 1 so no photo touches itself). When there are too few photos to honour it, the distance shrinks and the tool says so.
- **Blend with target:** at 0 each tile is untouched. Above 0 each tile is tinted towards the colour of the target cell it fills, in proportion to the slider. Applies to the draft and the built picture alike.
- **Tile crop:** the centred square of the photo, turned upright by its orientation.

## Which photos can be tiles

- Pictures only; videos are never used.
- Photos marked private are left out. (My call; the sync feature also keeps them out.)
- A photo hidden as a duplicate counts once, as the copy Browse shows.
- A photo whose file cannot be read is skipped, and the tool says how many were skipped.
- A photo with no cached thumbnail is not skipped; its thumbnail is made when the colour index needs it.

## Size, resolution and saving: the Collage rules

Reuse the Collage code and wording wherever it exists rather than copying it.

- Paper sizes and orientation as Collage; resolution 300 dpi up to A3, 150 dpi for A2 and larger (`defaultDpi`).
- The longest side may be at most 16,384 pixels (`MAX_SIDE_PX`); a larger page gets the same "That page would be … pixels" message and Build is disabled.
- Save: a save dialog offering PNG and JPEG, starting in the sync folder, or else the first watched folder, with a date-stamped default name (`defaultSavePath`). Button shows "Saving…", then "Saved." or the error.
- A built picture the person never saves is only cache; Pixie replaces it on the next build. It stays after the tool is closed.

## Where it lives and how it is made

- Rust: colour index and matching in `pixie-index` (database table for the colours, query for candidates by source); building the full-size picture in `pixie-image` (it already owns decoding, thumbnails and the Collage renderer); Tauri commands in `src-tauri/src/lib.rs` for index progress, draft, build (cancellable, with progress events), and save.
- UI: a new tool component beside `CollageTool.svelte`, using the existing crop/zoom frame behaviour and the Collage page-size settings.
- Candidate sources use the database directly (`photo_ids_in`, `list_picked_ids`, the library as a whole), never an id list passed from the UI, so there is no size limit.
- Ask Pixie's guide (`askGuide.ts`) and its fixture are updated; every control has a `title` (and `aria-label` where it has no visible words); counts use `plural`; no control in the middle of a sentence; the Build button is the one `primary` button until a picture is built, then Save is.

## Decisions left to the builder

- The colour space and distance for matching; whether to store more than one colour per photo (for example four quadrants) if it clearly improves the result.
- The assignment method (greedy with penalties, or better), as long as it is deterministic for the same settings and fast enough for the draft to feel live on a library of 100,000 photos.
- The default repeats distance and the draft's resolution.
- The viewer used to zoom and pan the built picture, as long as it copes with 16,384 px images.
- Wording of the messages not given above, in the app's plain style.

## Open risks to check while building

- Time to build the colour index for the first time on a very large library.
- Memory and speed of the full-size build near the 16,384 px limit (stream the picture in strips if needed).
- Speed of zooming around a very large built picture.

## Calls I made (for the person to check)

- Square tiles taken from the centre of each photo.
- Private photos excluded; videos excluded; duplicates count once.
- The built picture is cached, not added to the library.
- Colour index stored in the library database and reused.
