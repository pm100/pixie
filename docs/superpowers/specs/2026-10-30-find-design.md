# Find: search and combined filters

Status: **shelved on 2026-10-31** after a first stage was built and tried (see "What was tried" at the end). Originally: proposed. Related issues: #6 (Find), #37 (combine people and tags, and / or), #41 (free-text search, removed from the UI), #89 (tree counts), #100 (limits of the tagging model). Saved searches are a separate feature, #106.

## Why

Pixie knows the tags, people, trips, places, cameras, dates, Picked photos and media types of the library, and it keeps a CLIP vector for every analysed photo. But a person can only click one filter of each kind in the tree, they always combine with "and", and there is no way to type a question. Free-text search existed and was removed from the screen because people could not tell it from choosing an Auto tag (#41).

The goal is one way to ask for photos that covers everything Pixie knows, without two systems that look alike.

## Goals

- One **Find** box that offers exact things Pixie knows (people, tags, trips and so on) and, as a clearly separate and labelled answer, an approximate "looks like" search.
- Choices become chips in the filter line that already exists. They combine with **and** between different kinds, and with **or** or **and** inside one kind, which the person can flip.
- The tree keeps working as it does now. Adding to a combination is an extra gesture, not a changed one.

## Non-goals

- Counting things in a photo ("three dogs"), positions in the frame, identity beyond named people, text inside pictures (OCR). The model reads the overall look of a picture (see #100).
- Searching videos. Videos have no vectors yet (#39). The "looks like" note says so.
- Ratings and favourites (#6 keeps those).
- Changing what plain clicks in the tree do.

## The flow

### The box
At the top of the Browse tree, above When. Typing lists matches grouped by kind:

    People      Anna
    Auto tags   Beach (Settings)
    Your tags   beach house
    Trips       Beach week, Cornwall 2019
    Looks like  "anna beach"   (approximate)

- Exact matches come from the same lists the tree shows (people, Auto tags, your tags, trips, cameras, folders, years and months). Choosing one is the same as clicking it in the tree.
- The last row is "Looks like", the old `search_photos`: the typed words are embedded with CLIP and the closest stored photo vectors are the hits. It is labelled **approximate**.
- Enter takes the first exact match, or "Looks like" when nothing matches exactly. Esc clears the box.
- Names are matched without regard to case and accents, on any part of a word.

### The chips
Each choice is a chip in the existing filter line:

    Anna  or  Ben   .   Beach   .   2019        [Clear all]

- Different kinds always combine with **and**.
- Two or more chips of the same kind (people, Auto tags, your tags) are joined by a word between them, **or** by default. Clicking the word flips the whole group to **and** (and back). One mode per group, so there is no precedence to explain (this is my call; #37 allowed a mode per pair).
- Each mode has its own colour and the word is on the line, so the mode is always visible.
- The ✕ on a chip removes only that chip. Removing the others leaves "only this one".
- "Looks like" chip: shows a small approximate mark and the number of matches. Only one at a time. While part of the library is still being analysed it says "Searched N of M photos; videos are not searched". While it is on, an order choice appears: **Best match first** (default) or By date.
- An empty result names the combination ("No photos of Anna and Ben together") and offers to drop the last chip.
- "Show all" and "Clear all" clear everything, as now.

### The tree
- A plain click works as today: it replaces that kind's choice.
- **Ctrl-click**, or a **+** shown on hover, adds the row to the combination (same kind: as the group's current word; different kind: and).
- The chip line carries a short tooltip about Ctrl-click. Nothing else about the tree changes.
- The counts beside rows follow the whole combination, as they follow filters now. Whether a section's counts leave out that section's own choice is #89; it stays as it is until that is decided.

## Stages, each usable on its own

1. **Box and exact matches, with "Looks like"** (the label, the analysed-so-far note and best-match order). All choices combine with "and". Uses what exists: the filters already restrict the listing to hits, and `search_photos` still works.
2. **Or and and within a kind**, and Ctrl-click in the tree.
3. Later: ratings and favourites, text in pictures.

Saved searches are a separate feature (#106); they build on the chips here but are not part of Find.

## Data and backend

Today `filter_state` holds one value each for text, tag, person, camera, Picked-or-your-tag and missing data, and `refresh_hits` builds the listing's hit set by INTERSECT of one hit table per kind.

Change for stage 2:
- Hold a list of choices: kind, value, and a mode per kind (`or` or `and`).
- For each kind with several choices, its hits are the UNION (or) or INTERSECT (and) of its choices' hits; the kinds are then INTERSECTed together as now.
- The "Looks like" hits keep their ranking (similarity) so the listing can be ordered by it; the other kinds do not rank.
- The listing and counts keep using `search` as the flag that the listing is restricted to hits, so the existing commands do not change shape.
- The facet counts (`FacetAsk`) take the list of choices, with each section counted without its own kind as now.

Stage 1 needs only: the box's suggestions (a command that returns matches by kind), and the "Looks like" chip using the existing text hits.

## Tests

- Suggestions: grouping, case and accent insensitivity, exact before approximate, empty box.
- Chip logic as plain functions: adding, flipping a group's word, removing, the text of the empty-result message.
- Backend: UNION and INTERSECT within a kind, INTERSECT across kinds, "Looks like" with another choice, counts under a combination.

## Decisions made

- Tree: plain click replaces, Ctrl-click or + adds (chosen over "plain click adds" so existing habits are not broken).
- One box with exact matches and a labelled approximate row, not a separate search field.
- One and/or mode per kind, flipped by clicking the word (my call).

## Left open

- Whether "Looks like" results are limited as before (similarity floor 0.2, best 500) or by a count the person can raise.
- What the box does while Auto tags or People are switched off (hide those groups).
- The wording of the approximate label and the analysed-so-far note.
- Whether suggestions should include folders and individual days, or only the groups above.

## What was tried, and why it is shelved

Stage 1 was built and tried on a real library (the Find box in the Browse tree, exact suggestions by kind, and a "Looks like" row using the existing `search_photos`). The exact part worked as designed. The approximate part did not, and the idea of typing a natural phrase was set aside.

What went wrong, measured or read from the code:

- **"Found 500" was only the cap.** `SEARCH_LIMIT` is 500 and `SEARCH_FLOOR` is 0.2. The scores of the B/32 model are about 0.23 to 0.25 for almost any label on an ordinary photo, and a clear match is usually 0.28 or more (see #100), so nearly everything passes the floor and the result is the closest 500 photos in the library, with no "nothing matches" answer. The count must never read as "found N".
- **The model does not see small things.** "dog at the beach" returned beaches with no dog: the picture-wide scene outweighs the animal. The same weakness is why a rose by a castle gets no tags (#100) and why screenshots are almost never tagged (#108).
- **The model does not know the user's world.** "alex at the beach" returned 500 beach photos, 5 of them with Alex. Pixie knows Alex (from faces) and Beach (an Auto tag) exactly; only the model is blind to the name.
- **An earlier attempt had the same problem.** "Ask Pixie" used a large language model to read the question. The language model was never the bottleneck: it can only be as good as what the picture model can see, and what Pixie stores exactly (people, places, trips, dates, cameras).

What would be worth doing if this comes back, in this order:

1. **Read a phrase into exact filters first.** Words that match a person, Auto tag, own tag, trip, camera or year become exact filters joined with "and" (this already works today, one value per kind). Small words are ignored. "Looks like" is offered only for the words left over, and is labelled approximate.
2. **Several choices of one kind with or / and** (stage 2 above): the part of the design that is exact and visible.
3. **Be honest about the approximate part:** "closest 500 of N", a stricter floor (about 0.27, to be measured on the real library) and a "nothing close" answer.
4. **Best match first** needs a ranking in the listing, which pages by date today; left out of stage 1.
5. **A better picture model** is the only thing that fixes what the model cannot see. The cost: roughly 10 to 20 times the compute per image for a large CLIP, a larger download, re-analysing the whole library, re-tuning the tag thresholds, packaging and licence checks. Try it first on a few hundred known photos (a rose by a castle, a dog in a larger scene, a named person) and compare with the current model before deciding; offer it as a Settings option, not the default.

Free-form descriptions of what is in a picture ("a rose and a big dog in Ireland") depend on that last point and cannot be promised.

The built stage 1 (Find box, suggestions, "Looks like" pill, tree counts following the search result) was taken out of the tree again and kept as a patch. The mock-up is a private artifact made in the session of 2026-10-31.
