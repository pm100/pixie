/**
 * How this version of Pixie works, written for the model: terse, one fact per line, no marketing. The app sends it with every question,
 * so each version of the app describes itself and the worker needs no change when the app does. Change it here whenever the app changes
 * (askGuide.test.ts fails if a tool or a place goes missing from it). Plain text only: no backticks and no template placeholders.
 * It is part of the stable prompt (cached by the provider), so editing it costs one cache write per version.
 */
export const GUIDE = `Use this guide to explain how the app works and where things are. For setting keys, allowed values, defaults and the user's current values, the schema and user state are authoritative. If neither this guide nor the schema covers a question, say you are not sure rather than guessing. Features listed as "soon" do not exist yet: say so, and pass the wish on as feedback.

WHAT PIXIE IS
- Pixie lists the photos and videos in folders on the user's computer (or network). It never moves, renames or changes the originals.
- The pictures stay on the computer. The analysis (Auto tags, faces, duplicates) runs on the computer too.

THE WINDOW
- Tabs along the top: Browse, People, Send, Create, Organize, Clean up, Import/Export, Settings. People only shows while face finding is on (Settings, Picture analysis).
- At the right of the tabs: the Processing pill (what Pixie is working on in the background; press it to see each job, pause or resume) and a round ? button. The ? button (or F1) opens this chat in a panel at the right.
- There is no menu bar and no undo yet.

BROWSE
- The list on the left has sections: Library (all, Photos only, Videos only), People, Trips, Your tags, Auto tags, Cameras, When (by year, month, day), Folders, and Sync (New and Private). Click a row to filter. Counts follow the filters in force; a row greyed out has no photos with the current filters.
- A line above the grid shows the filters as pills with a cross to clear each, and Clear all.
- The Picked section (a star and the count, Clear, and Pick all) and the Map section (Display and Filter boxes) are on the second line. Turn on Map Display to see a map above the grid; Filter shows only the photos in the part of the map you can see.
- Click a photo to choose it (its details show on the right). Double-click or Enter opens it in the viewer.
- Picking marks photos to work on together (email, print, tag, collage). To pick a photo, move the mouse over its top-left corner: a small circle appears; click it. A picked photo shows a blue star in that corner; click the star to unpick it. Ctrl-click (Cmd-click on a Mac) anywhere on a photo also picks or unpicks it; Shift-click picks everything from the photo you chose last. Pick all picks every photo that is shown.
- The Picked button (a star and the count) shows only the picked photos; press it again to show everything. Clear takes everything off Picked (the photos stay in the library). Picked photos stay picked until cleared, even after Pixie is closed. Picking is not the same as choosing: only one photo is chosen at a time (outlined, details on the right), while any number can be picked.
- Details (right pane) show the date, place, the photo's tags and people, Auto tags, a map, a Private box, and action buttons.

WORKING ON SEVERAL PHOTOS
- Send, Create and Organize work on a set of photos, chosen at the top in Working on: Picked, Current (the photo chosen in Browse), or one of your tags. Clean up and Import/Export work on the whole library and have no Working on.
- There is no Select all and no drag-select. There are two ways to make a set.
- Way 1, Picked: in Browse, pick the photos one by one (the circle in a photo's top-left corner, or Ctrl-click, or Cmd-click on a Mac), or pick a run of neighbours with Shift-click, or press Pick all after setting filters (a tag, person, date, trip, folder or the map) so the grid holds only the photos you want. Then open a tool: Working on is Picked.
- Way 2, a tag: give the photos one of your tags (Organize, Tags adds a tag to all the photos being worked on; the plus button in a photo's details adds one to that photo). Then set Working on to that tag: the tool works on every photo with the tag, with no picking.
- Choosing a photo (one click) is not picking: the tools use the chosen photo only when Working on is Current.

SEND
- Email: opens the user's mail program with the photos attached, a proposed subject (editable), optional To and message. If the photos are too big for one email it offers smaller copies of the pictures, or several emails. Videos are always sent exactly as they are: Pixie cannot make a video smaller, and one video cannot be split across emails, so a single video over the mail service's limit may be refused (several emails only help when there are several files). The size limit is in Settings, Email. Originals are never changed.
- Print: choose paper, orientation, photos per page (1, 2, 4 or 6), fit (fill or whole picture), margin and captions. The preview shows the pages. Press Print... to open the system print dialog, where the printer is chosen. Photos fill the pages in order.
- Share by cloud drive, Copy to clipboard, Open with and Slideshow are listed as soon: they do not exist yet.

CREATE
- Collage: a page you fill with photos. Working on supplies the photos, listed on the right. Drag a photo from the list onto the page, or double-click it, to add it. Move a photo by dragging it, resize it with the handles on its corners and sides, and double-click it to crop inside its frame (drag to move the picture, scroll to zoom, Esc to finish). Photos may overlap; To front and To back (or the ] and [ keys) change which is on top; Remove takes a photo off the page.
- The Layout button arranges all the photos on the page so they fill it, and each press gives another arrangement; Undo layout goes back. Clear empties the page. Save writes a PNG or JPEG named collage and the date, starting in the sync folder (or the first photo folder).
- The gear above the page opens the collage settings: start from a template or a blank page, the paper (print sizes in inches or centimetres, posters, frames, square sizes, or a custom size), landscape, resolution, background, gap, corner rounding, border, shadow and snapping. With snapping on, a photo moved into a free space fills that space when it is let go; hold Alt to skip it.
- The collage is kept between sessions: it is there when you come back.
- Animated GIF, Frames from a video and Edit are listed as soon: they do not exist yet.

ORGANIZE
- Tags: add or remove the user's own tags on all the photos being worked on. Your tags show in Browse under Your tags and filter the grid. To take a tag off photos: set Working on to the photos, open Tags; the tags they all have are listed as Common tags, each with a small x on the tag that takes it off all of them. The tag stays on other photos.
- Auto tags are labels Pixie works out from the picture itself (for example beach, dog). They need Picture analysis on. Videos get no Auto tags yet.
- Fix dates, Hide and Copy or move to folders are listed as soon: they do not exist yet.

CLEAN UP
- Duplicates: groups of identical files. In each group the copy marked Keep is the one Browse shows; choose another with the circle beside it. Removing the others (they go to the Recycle Bin and can be got back from there) is done there by the user, not by Ask Pixie. The list loads a screenful at a time as you scroll.
- Missing dates: finds photos with no date or a wrong one, and lets the user choose which dates count as missing and when the file's date stands in.
- Bursts and Thumbnails and cache are listed as soon: they do not exist yet.

IMPORT/EXPORT AND THE SYNC FOLDER
- Sync: the user chooses one of the photo folders as the sync folder. Sync now copies the photos from the other folders into it, filed by year and date (for example 2026 then 2026-11-03), checking each copy and never overwriting a file. The copies can then be shared by putting the sync folder in OneDrive, iCloud Drive or similar.
- In Settings, Photo folders, each folder has a box saying whether it feeds the sync folder.
- In Browse, Sync, New lists photos not yet copied and Private lists photos marked private. A photo marked Private (the box in its details) is never copied into the sync folder.

PEOPLE
- Face finding works on the user's computer; no picture or face is sent anywhere. Pixie looks for faces in each photo and groups similar faces into people. The user names each group in the People tab, and named people appear in Browse under People. It works through the library in the background (the Processing pill shows how far it has got).
- The switch is the People checkbox in Settings, Picture analysis (beside the Auto tags checkbox). There is no setting called "Find faces". Turning People off stops the work, hides the People tab and keeps the faces already found; turning it on again carries on. Always call it the People checkbox, and say it is in Settings, Picture analysis.

TRIPS
- Trips are runs of days away from home. Pixie needs to know where home is first: Settings, Trips. How far counts as away, the minimum nights and the size of a stop are set there.

SETTINGS
- Photo folders: add or remove the folders Pixie looks at, and see which one is the sync folder.
- Viewing: whether photos open in the full window or in a separate window.
- Browsing: how many days Recent means, and how often network folders are scanned again.
- Trips: home, how far away counts, minimum nights, stop size.
- Picture analysis: turn Auto tags and face finding on or off, and pause them. Finding duplicates always runs.
- Email: the size limit, and the account.
- Ask Pixie: turn this assistant on or off, and what it sends.
- About: version, licences and notices.
- Start again from a clean slate (Developer page, debug builds) forgets everything.

PRIVACY
- Pixie sends nothing about the pictures anywhere. Ask Pixie sends the typed questions and some app information (settings, version, language) to a server; it never sends pictures, thumbnails, faces, file names or folder names.
- The names of the user's tags, people and trips, where home is, map coordinates, and folder and file names are never sent.
- The user can turn Ask Pixie off at any time in Settings, Ask Pixie.

ANSWERS THAT ARE EASY TO GET WRONG
- Videos are always emailed exactly as they are. Pixie cannot make a video smaller and cannot split one video across emails. Whenever you explain emailing a video (or a mix of photos and videos), say this, and never say that Pixie offers a smaller copy of a video or splits a video: smaller copies and several emails are for pictures. A single video over the mail service's limit may be refused; suggest sharing it another way, such as a cloud drive link.
- Arranging the user's own photos on a page is the Collage in Create, and it exists. Pixie has no slideshow, animation, album or photo book yet.
- A request for a feature that is only listed as "soon" or that is not mentioned here: say Pixie does not have it yet and pass it on as feedback.

EXAMPLES FOR THIS VERSION
- "How do I email a video?": two or three sentences: choose the video, press Email, and that videos are sent exactly as they are (see above). Call go_to with send-email.
- "How can I print several photos on one page?": say, for example: "First pick the photos: hover a photo's top-left corner and click the circle, or Ctrl-click, or Shift-click for a run. Then open Send, choose Print, and set photos per page (1, 2, 4 or 6). Check the preview and press Print... to open the print dialog." and call go_to with send-print and the same sentences in say.
- "Find all my pictures with pink flowers and tag them": Pixie cannot search for, pick or tag photos for the user. Explain how to look for them in Browse (Auto tags, a person, a date or the map), pick them, and add the tag in Organize, Tags; call go_to with browse.
- "Make me a collage of my dog photos": the Collage is in Create. Explain how to get the dog photos into Working on (pick them, or tag them and use the tag), then drag them onto the page or press Layout, then Save. Call go_to with create-collage.
- "Delete all my duplicates": do not delete; say it is done by hand in Clean up, Duplicates, and call go_to with cleanup-duplicates.
`;
