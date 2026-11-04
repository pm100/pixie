# Collage editor: design

Replaces the form-based Collage tool built on 2026-11-02 (plan `docs/superpowers/plans/2026-11-01-collage.md`, issue #12). The photomosaic stays separate (#112). Written after talking it through with the user; anything marked **(my call)** was not agreed and can be changed.

## Intent

The user picks photos from the Tools "Working on" set and composes a page from them by hand: place, resize and crop each photo, with Pixie helping to line things up. They can also say "start with these" and get a sensible first layout, or start from one of the fixed templates. The result is saved as a PNG or JPEG for printing, usually by an online print shop. The collage is kept inside Pixie, so it can be reopened and edited later.

The first build chose a template and then let you nudge a crop. That is the wrong way round for this user: the page is the thing they work on, so it must fill the middle of the screen, and the photos are what they pick from.

## Goals

- A free-form page: every photo is a **frame** that can be moved, resized, cropped, layered over others, and deleted.
- **Snapping** that helps without getting in the way, with visible guide lines.
- Three ways to begin: **Pixie lays them out** (from the selected photos), **a template** (the 12 fixed ones stay), or a **blank page**.
- **Overlap** is allowed, with a frame order (bring to front, send to back), an optional white **border** and an optional soft **shadow**, so overlapped photos read well.
- Page sizes that match what print shops sell, with the sizes of the user's region first.
- The design is **kept**: close Pixie, come back, carry on. One collage for now.
- What the preview shows is what is saved: one set of drawing rules, used by both.

## Non-goals (for this round)

- Rotation of frames (the next likely addition; keep the frame model able to take an angle later).
- Text, captions, stickers, clip-art.
- Several named collages (a "My collages" list). One collage, kept; Clear starts a new one.
- Undo and redo (see #88). Clear asks before it throws the design away.
- Printing straight from Pixie or ordering from a shop. The saved file only has to be the right size and resolution.
- Photomosaic (#112) and videos. Only photos can go in a collage.

## Anti-goals (a failure even if it "works")

- Anything that changes an original file.
- A saved picture that differs from the preview (crop, border, shadow, corners, order).
- The design being lost without the user asking (a crash, switching tools, a photo leaving the library).
- Snapping so eager that a frame cannot be put where the user wants it.

## The screen

Three areas, in Pixie's usual order (list, work area, side list):

1. **Tool list** (left, as for every tool): Create > Collage.
2. **Compose area** (middle, takes all the room): the page, drawn to scale and centred, on a grey surround. A bar across the top has, from the left: a **gear** that opens the settings pop-up, a summary of the page (for example `8 × 10 in · portrait · 2400 × 3000 px · 300 dpi`), the snapping state, **Clear**, and the main button **Save** (class `primary`; it opens the save dialog for a PNG or JPEG). A hint line at the bottom says how to drag, resize, crop and delete.
3. **Image list** (right): the Working-on photos as thumbnails, in the order of the Working-on set, each marked when it is on the page, with how many are on the page. Ctrl-click and Shift-click select several; **Start with selected** sits under the list.

The **settings pop-up** opens by itself the first time Collage is opened with an empty page, and from the gear at any time. It holds: *Start from* (Pixie lays them out / a template / a blank page, with the 12 template pictures to pick from), *Paper*, *Landscape*, *Resolution (dpi)*, *Gap (mm)*, *Corner rounding (mm)*, *Border (mm)* and its colour, *Shadow* (on or off), *Background* colour, and the two *Snap* switches. Labels come before controls, with units in the label (AGENTS.md). Its main button is **Done** (class `primary`).

## The collage (what is kept)

One record, written to the Pixie database every time something changes (a short delay after the last change is enough), and read when Collage is opened. A single JSON document in a one-row table is enough **(my call)**. It holds:

- **Page**: paper id or a custom size in mm, orientation, resolution, background colour.
- **Look**: gap (mm), corner rounding (mm), border width (mm) and colour, shadow on or off, snapping switches. These apply to the whole collage **(my call)**; per-frame values are a later step. Starting values **(my call)**: gap 4 mm, corner rounding 0, border 0 mm (white when turned on), shadow off, background white, both snaps on.
- **Frames**, bottom to top. Each has: photo id; position and size as fractions of the page (`x`, `y`, `w`, `h`), so changing the paper size keeps the arrangement; and the crop inside the frame as a focus point (0 to 1 across and down) and a zoom (1 or more; 1 is "fill the frame").
- The template the page was started from, if any (for the label only).

A frame whose photo has left the library is shown as an empty grey frame saying the photo is missing; it is not removed. Save names the missing photo and stops, as in the first build.

## Behaviour

**Adding photos.** Drag a photo from the list onto the page: a new frame appears where it is dropped, sized to the photo's shape (about a quarter of the page area **(my call)**) and kept inside the page. Double-click a photo in the list does the same, placed in the first free spot **(my call)**. Dropping a photo from the list on an existing frame puts that photo in that frame (the frame keeps its size, the crop resets). Dragging one frame onto another swaps their photos. A photo can be on the page more than once.

**Selecting.** Click a frame to select it (outline and eight handles); click the grey surround to deselect. Shift-click selects several; moving and deleting then act on all of them. Delete or Backspace removes the selection.

**Moving and resizing.** Drag the body of a frame to move it, a handle to resize it. A corner keeps the frame's shape unless Shift is held, a side handle changes one dimension. Frames cannot be dragged off the page; they are kept at least a small size (about 5% of the page) **(my call)**.

**Crop.** Resizing a frame already crops the photo (it always fills the frame: "cover"). For finer control, double-click a frame to enter crop mode: dragging pans the photo inside the frame (the focus point), the mouse wheel or a slider zooms (1× to 4× **(my call)**), Escape or a click outside leaves it. The dpi warning (below) follows the zoom.

**Order.** The selected frame can be brought to the front or sent to the back from the bar that appears beside it, or with `]` and `[`. Clicking where frames overlap selects the top one.

**Snapping.** While a frame is moved or resized, its edges and centre line snap, within about 6 screen pixels **(my call)**, to: the page edges and the page margin (half the gap, as in the first build), the page centre lines, and the edges and centres of the other frames. Moving also snaps to equal spacing between neighbours. A pink guide line is drawn for each active snap, with the distance when it is a gap. Holding Alt turns snapping off for that drag. Two switches in the settings turn the groups off (page, other frames).

**Overlap.** Nothing pushes frames apart. A frame can sit on another. The gap setting is used by templates and by the automatic layout, which keep their frames apart; a frame placed by hand stays where it is put, and moving it does not re-apply the gap.

**Start with selected.** With nothing selected in the list it uses all the Working-on photos (up to the limit below). Replaces the page contents (asking first if the page is not empty) with a layout of the selected photos **(my call on the method)**: justified rows, as Browse does. Each photo keeps its own shape (no cropping), the number of rows is chosen so the block fills the page's shape as well as it can, rows are scaled to the page width, the block is centred, and the gap setting separates them. Photos keep the order of the Working-on set. More than 24 photos are not laid out; the rest are named as left out **(my call)**.

**Start from a template.** Fills the template's spaces with the selected photos (or all Working-on photos if none are selected) in order. Spaces with no photo are left out of the page. After that the frames are ordinary frames. The template pictures are the 12 already built; the labelled outline buttons are kept.

**Blank page.** An empty page of the chosen size.

**Clear.** Asks "Clear the collage? The photos stay in your library." and empties the page (keeping the page and look settings).

**Changing the paper or orientation** with frames on the page keeps each frame's fractions and re-crops the photos to fill (focus and zoom kept). The result may stretch the shape of a frame; that is expected **(my call)**.

## Drawing rules (one set, two implementations)

The window previews with CSS; Rust draws the saved file from the original photos. They must agree on:

- The page: size in mm, converted to pixels at the chosen dpi. The longest side is at most 16,384 pixels; the tool says so and does not save otherwise (as built).
- Frames drawn bottom to top.
- A frame's photo is cropped to the frame by the *cover* rule around the focus point; zoom shrinks the visible region by the zoom factor around the same point (as built for the crop, plus zoom).
- Corner rounding applies to the frame outline (photo and border together).
- Border: a band of the border colour drawn inside the frame edge, the photo inset by the border width.
- Shadow: a soft dark shadow offset down and to the right under each frame, drawn before the frame and under the frames above it. A fixed look is enough **(my call)**: offset and blur about 1.5% of the shorter side of the page, 35% black. A rendering of the blur that is close to the preview's is enough; it need not be identical pixel for pixel.
- A frame whose photo is missing cannot be drawn: Save stops and names the photo (nothing is written).

## Sharpness warning

As built: the pixels of the used part of each photo divided by the printed size of its frame, with a "Low resolution" badge under 100 dpi and a summary line. It now follows frame size and zoom, and is shown on the frame itself.

## Page sizes

A single list, grouped, with the user's region first and the rest still available **(agreed: detect the region, put those sizes first)**. Region comes from the system language's country (`navigator.language`), the way Trips decides on miles (`usesMiles` in `ui/src/lib/trips.ts`); plain `en` with no country counts as metric. Inches first for the US, Canada, Mexico and the Philippines **(my call on the country list)**.

- **Prints (inches)** (Letter, 8.5 × 11, is added to this group for inch regions): 4×6, 5×7, 8×10, 8×12, 11×14, 12×18, 16×20, 16×24, 20×24, 20×30, 24×36, 30×40.
- **Prints (cm)**: 10×15, 13×18, 15×20, 20×30, 30×40, 40×50, 50×70, 60×90, 70×100.
- **Posters and frames**: A4, A3, A2, A1, A0; frame sizes 30×40, 40×50, 50×70, 61×91 cm.
- **Square (books, calendars)**: 8×8 in, 12×12 in, 20×20 cm, 30×30 cm.
- **Custom**: width and height in cm, up to 100 cm.

The list is from general knowledge, not checked against a particular shop; the user will order from different shops and a custom size covers the gaps. Default page: 8×10 in (inch regions) or 20×30 cm (others), portrait. The last choice is remembered. Resolution: 300 dpi below A2, 150 from A2 up (as built), changeable to 150, 200 or 300.

## What already exists and is reused

From the first build (uncommitted at the time of writing): `crates/pixie-image/src/collage.rs` (templates, `cover_crop`, `effective_dpi`, page size limit, PNG and JPEG saving with dpi recorded, tests), `src-tauri/src/collage.rs` (commands), `ui/src/lib/collage.ts`, `CollageTool.svelte`, and the Create > Collage entry in `tools.ts` and `ToolsTab.svelte`. The renderer changes from "a template and a photo per cell" to "a list of frames"; the templates become a way to *make* frames. `CollageTool.svelte` is replaced; the helpers that remain right (crop rule, dpi, size limit) stay.

## Left to the builder

The drag and drop mechanics (pointer events or HTML drag and drop), how the page is drawn in the window (HTML elements or a canvas), the shadow blur method, how the autosave is debounced, the exact justified-rows algorithm, the table name and schema version, and how the tool is laid out in `ToolsTab.svelte` (the page takes the middle area, the list the right-hand one). Every control needs a tooltip (AGENTS.md); the Save button says "Save", not "Save collage…".
