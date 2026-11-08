# OffsetSrf getters and admission

Bare `OffsetSrf` supports command-first selection, source direction arrows,
cached surface previews, signed/unit-aware distances, two-point distance input,
`FlipAll`, individual source flips, and immediate `ThroughPoint` acceptance.
Distance, Solid, BothSides and DeleteInput preferences live outside model history.
Esc first leaves an active distance or through-point subprompt; another Esc
cancels the command. Previews preserve geometry and Redo, and changes to sources,
model settings or the displayed scene invalidate them.

Eight private-Xvfb captures retain 56 licensed Rhino 8 outcomes. Forty-seven complete
Sharp workflows are compared in both builds: 39 admissions and eight cancellations.
Nine discovery or Round observations remain separate, including timed-out and
overlong input sequences. A failed IronPython import retains its request, frozen
sources and host error; its temporary response was removed by the client and
qualifies no geometry. Nothing was recaptured over an existing successful family.

The captures distinguish current-layer copies, separate BothSides skins, and
in-place nonzero solid replacement. Zero open offsets create identical copies
with real history. Zero walls retain Rhino's coincident caps, collapsed sheets,
and separate deletion/replacement policies. Through-point cases cover reversed
directions, a point outside the natural boundary, ignored face trims, and a batch
whose second source has the opposite normal. Moving the target near either
source establishes that the closest selected underlying surface supplies the
shared distance. Positive-distance getter captures retain rejection of negative
answers, unchanged values after zero answers, and the default 1 after a zero
current distance.

Replays build sources from authored corners and trims. Native output definitions
serve as expectations. Comparisons include bidirectional surface and trim stations,
oriented normals where defined, edge/vertex stations, bounds, topology counts,
attributes, groups, source identity, actual Undo/Redo events and 3DM round trips.
Headless egui pointer events exercise whole-face selection and flipping in
wireframe and shaded views, excluding preview objects from input picks. Separate
controls check cached admission, mixed invalid selections and stale-source rejection.

Run the reproducible checks with:

```sh
python3 -m tools.rhino_oracle.check_offset_surface_getter
```

The [pinned validation record](offset-surface-getter-validation.json) identifies
the implementation, evidence and check logs. Its model-space gate is 1e-6.
This finite set does not qualify Round/Loose options, native factory preferences,
rendering materials, arbitrary curved through-point branches, singular inputs,
all solid fallback partitions, subobject workflows, selection-distance ties,
new-object creation order, continuous bounds, pixels or
comparative performance. Earlier [SDK geometry evidence](smlib-offset-surfaces.md)
remains a separate qualification with its original replay bytes pinned in Git.
