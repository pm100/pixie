/**
 * How Pixie works, written for the model: terse, one fact per line, no marketing. It is part of the stable
 * prompt (cached), so edits cost one cache write. When the app changes, change it here too. Plain text only:
 * no backticks and no template placeholders.
 */
export const GUIDE = `Use this guide to explain how the app works and where things are. For setting keys, allowed values, defaults and the user's current values, the schema and user state are authoritative. If neither this guide nor the schema covers a question, say you are not sure rather than guessing.

WHAT PIXIE IS
- Pixie lists the photos and videos in folders on the user's computer (or network). It never moves, renames or changes the originals.
- The pictures stay on the computer. The analysis (Auto tags, faces, duplicates) runs on the computer too.

TABS
- Browse: the photos, with a list on the left, the grid in the middle, details on the right.
- Tools: work on the Picked photos, the Current photo or a tag together: Photo details, Email, Print, Tags.
- People: the faces Pixie found, grouped by person; name them here.
- Duplicates: groups of photos that look the same; choose which to keep.
- Settings: Photo folders, Viewing, Browsing, Trips, Picture analysis, Email, About.
- Ask Pixie: this chat.

BROWSE
- The list on the left has sections: Library (all, Photos only, Videos only), People, Trips, Your tags, Auto tags, Cameras, When (by year, month, day), Folders. Click a row to filter. Counts follow the filters in force; a row greyed out has no photos with the current filters.
- A line above the grid shows the filters as pills with a cross to clear each, and Clear all.
- A second line has Picked (star and count, and Clear), and the Map and Filter by map boxes. Turn on Map to see a map above the grid; Filter by map shows only the photos in the part of the map you can see.
- Click a photo to choose it (its details show on the right). Double-click or Enter opens it in the viewer.
- Picking marks photos to work on together (email, print, tag). To pick a photo, move the mouse over its top-left corner: a small circle appears; click it. A picked photo shows a blue star in that corner; click the star to unpick it. Ctrl-click anywhere on a photo also picks or unpicks it; Shift-click picks everything from the photo you chose last.
- The Picked button on the second line above the grid (a star and the count) shows only the picked photos; press it again to show everything. The Clear link beside it takes everything off Picked (the photos stay in the library). In Tools, Working on, Picked lists the same photos, and the star there unpicks one.
- Picked photos stay picked until cleared, even after Pixie is closed. Picking is not the same as choosing: only one photo is chosen at a time (outlined, details on the right), while any number can be picked.
- Details (right pane) show the date, place, the photo's tags and people, Auto tags, a map, and buttons Email, Print.

WORKING ON SEVERAL PHOTOS
- Print, Email and Tags work on a set of photos. There are two ways to make a set; there is no Select all and no drag-select.
- Way 1, Picked: in Browse, pick the photos one by one (the circle in a photo's top-left corner, or Ctrl-click, or Cmd-click on a Mac), or pick a run of neighbours with Shift-click (everything from the photo you chose last to the one you click). Filters help: set a tag, person, date, trip, folder or the map first, so the grid holds only the photos you want, then pick from it. Then open Tools; Working on at the top is Picked.
- Way 2, a tag: give the photos one of your tags (Tools, Tags adds a tag to all the photos being worked on; the plus button in a photo's details adds one to that photo). In Tools, set Working on to that tag: the tool then works on every photo with the tag, with no picking. Your tags are listed in Working on and in Browse under Your tags.
- Working on also offers Current photo, the one chosen in Browse (the Email and Print buttons in the details use it).
- Choosing a photo (one click) is not picking: only one is chosen at a time, and the tools use the chosen photo only when Working on is Current photo.
- To print several photos on one page: pick them (or tag them), open Tools, Print, set photos per page (1, 2, 4 or 6), then choose paper (A4, Letter, 4x6, 5x7), orientation, fit (fill or whole picture), margin and captions. The preview shows the pages. Press Print... to open the system print dialog. The photos fill the pages in order, as many pages as needed.
- To email several photos: pick them, open Tools, Email, check the subject, add To and a message if wanted, choose how to send them if they are big, press Email.
- To tag several photos: pick them, open Tools, Tags, and add the tag; it goes on all of them.

FINDING PHOTOS
- Ask Pixie can find photos by what is in them, by the user's own tags, by person, by date and by photo or video, and then pick them, tag them, or open them in Email or Print. It shows how many it found first, with a few pictures, and asks before it picks or tags.
- Finding by what is in the picture (for example pink flowers, a dog on a beach) works by the look of the picture, not by a label. It is approximate: it matches the overall gist, not exact counts, positions, small details or text in the picture. It only covers photos analysed so far, needs Auto tags turned on in Settings, Picture analysis, and skips videos.
- Finding photos with people in them (any person) uses the faces Pixie has found so far, so it needs Find faces turned on in Settings, Picture analysis, and leaves out photos not yet scanned. It can be combined with a content search, such as yellow flowers with people.
- When two things must be in the same picture (for example flowers and a birthday cake), each is searched for on its own and a photo must match both; the match is as strong as the weaker of the two.
- It cannot find by place name or by camera yet. It never deletes anything. Tags it adds can be undone straight afterwards from the chat.
- The photos it finds are kept in Tools under Working on as Ask Pixie results, so they can be looked at and used by Print, Email and Tags.

TOOLS
- Working on at the top chooses the photos: Picked, Current photo, or one of the user's tags.
- Email: opens the user's mail program with the photos attached, a proposed subject (editable), optional To and message. If they are too big for an email it offers smaller copies or several emails. The size limit is in Settings, Email. Originals are never changed.
- Print: choose paper (A4, Letter, 4x6, 5x7), orientation, photos per page, fit, margin and captions; Print opens the system print dialog.
- Tags: add or remove the user's own tags on all the photos being worked on.
- Photo details: the details of the chosen photo.

TAGS
- Your tags are words the user puts on photos (Tag as, the plus button in the details, or Tools, Tags). They show in Browse under Your tags and filter the grid.
- To take one of your tags off photos: pick them (or set Working on to a tag), open Tools, Tags; the tags they all have are listed as Common tags, each with a small x on the tag. Pressing the x takes that tag off all the photos being worked on. The tag stays on other photos.
- Auto tags are labels Pixie works out from the picture itself (for example beach, dog). They need Picture analysis on. Videos get no Auto tags yet.

PEOPLE
- Faces are found and grouped by person. Name a group in People. Named people appear in Browse under People. Picture analysis must have faces turned on.

TRIPS
- Trips are runs of days away from home. Pixie needs to know where home is first: Settings, Trips. How far counts as away, the minimum nights and the size of a stop are set there.

DUPLICATES
- The Duplicates tab lists groups of photos that look the same. Choose the one to keep. Removing the others is done there by the user, not by Ask Pixie.

SETTINGS
- Photo folders: add or remove the folders Pixie looks at.
- Viewing: whether the viewer opens in its own window or a pop-out.
- Browsing: how many days Recent means, and how often network folders are scanned again.
- Trips: home, how far away counts, minimum nights, stop size.
- Picture analysis: turn Auto tags, faces and the duplicate check on or off, and pause them.
- Email: the size limit, and the account.
- About: version, licences and notices.
- Start again from a clean slate (Developer pages, debug builds) forgets everything.

PRIVACY
- Pixie sends nothing about the pictures anywhere. Ask Pixie sends the typed questions and some app information (settings, version, language) to a server; it never sends pictures, thumbnails, faces, file names or folder names.
- The names of the user's tags, the people they have named and their trips, with counts, are sent only if the user turned on sharing in Settings, Ask Pixie. When they are not, the section marked LIBRARY says so, and Pixie checks names on the user's computer after the answer. Where home is, map coordinates and folders are never sent.
- The user can turn Ask Pixie off, or the sharing off, at any time in Settings, Ask Pixie.
`;
