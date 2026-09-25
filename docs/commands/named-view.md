# Named views

`NamedView Save Front detail` stores the active viewport's camera, projection,
zoom, target, and construction plane under that name. `NamedView Restore Front
detail` applies them to the active viewport, including when the view was saved
from a different viewport. Restore enters that viewport's view history and
updates its construction-plane history. It leaves model geometry, selection,
model undo, and display mode alone. The restored name becomes the viewport title;
an asterisk marks later camera or construction-plane changes.

`NamedView` or `NamedView List` lists saved names in their current order.
`NamedView Update Front detail` replaces an existing snapshot; Save rejects a
duplicate name regardless of case. `Delete`, `MoveUp`, and `MoveDown` accept one
name. `Rename Front detail | Entrance` and `Duplicate Front detail | Entrance`
use `|` to separate two names, so spaces are allowed in either name. Names are
matched without regard to case and keep the spelling you enter.

The current registry lives in the application session and is read or written
with `Open3dm`, `Save`, `Import3dm`, and `Export3dm`. A thumbnails panel and
floating viewport restore remain to be implemented. See
[Rhino's NamedView command](https://docs.mcneel.com/rhino/8/help/en-us/commands/namedview.htm).
