# Named views

`NamedView Save Front detail` stores the active viewport's camera, projection,
zoom, target, and construction plane under that name. `NamedView Restore Front
detail` applies them to the active viewport, including when the view was saved
from a different viewport. Restore enters that viewport's view history and
updates its construction-plane history. It leaves model geometry, selection,
model undo, and display mode alone.

`NamedView` or `NamedView List` lists saved names in their current order.
`NamedView Update Front detail` replaces an existing snapshot; Save rejects a
duplicate name regardless of case. `Delete`, `MoveUp`, and `MoveDown` accept one
name. `Rename Front detail | Entrance` and `Duplicate Front detail | Entrance`
use `|` to separate two names, so spaces are allowed in either name. Names are
matched without regard to case and keep the spelling you enter.

The current registry lives in the application session. The lower-level 3DM
reader/writer retains named views, but `Import3dm` and `Export3dm` do not yet
connect that file table to this session registry. A thumbnails panel and
floating viewport restore also remain to be implemented. See
[Rhino's NamedView command](https://docs.mcneel.com/rhino/8/help/en-us/commands/namedview.htm).
