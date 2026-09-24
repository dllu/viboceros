# OffsetSrf

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetsrf.htm)

Select one or more planar NURBS surfaces or single-face B-reps and run
`OffsetSrf 2` or `OffsetSrf Distance=2`. Each output is translated by the
signed distance along its face normal. A negative distance offsets in the
opposite direction. Trimmed B-reps retain their exact curves and topology.
Outputs inherit their source attributes and groups and are selected.

`BothSides=Yes` creates one disjoint two-face B-rep with offsets at both
signed distances. `Solid=Yes` makes
a capped extrusion from each source to its offset when the source has one
closed boundary loop. Rectangular bilinear surfaces produce six planar faces;
other profiles retain an exact ruled wall and two caps. Together,
`BothSides=Yes Solid=Yes` makes one solid spanning both offsets.
`DeleteInput=Yes` removes the sources. All three
options default to `No`. One Undo restores the prior document. The command
stages all outputs before editing the document, so unsupported selections
leave it unchanged.

Planar offsets are exact translations. The current solver rejects nonplanar
surfaces, multi-face B-reps, and solid offsets with multiple boundary loops.
Free-form offsets, polysurface corner joining, loose offsets, and interactive
direction arrows remain to be implemented. Tests cover signed and two-sided
offsets, trimmed face orientation, exact planar solid volume, Undo, and
atomic rejection. The [public Rhino offset-face probe](../../tools/rhino_oracle/fixtures/offset_surface_face_geometry.json)
and [Rhino 8 observations](../../tools/rhino_oracle/observations/offset_surface_face_geometry.json)
record face counts, bounds, topology counts, and signed solid volume for six
rectangular cases. Rhino reverses the inward negative solid when adding it to
the document, yielding positive volume; Viboceros follows that document
admission rule. This public API probe does not establish the interactive
command's selection or option-prompt behavior.
