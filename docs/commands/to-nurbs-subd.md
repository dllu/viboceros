# ToNURBS: whole SubD objects

```text
ToNURBS SubDOptions Faces=Packed ExtraordinaryVertex=G1x DeleteInputObjects=No
ToNURBS Faces=Unpacked ExtraordinaryVertex=G2 DeleteInputObjects=Yes
```

Selected SubDs become one B-rep each, including disconnected components.
`Faces=Packed` joins rectangular quad grids and assembles periodic seams;
`Unpacked` retains one surface per quad face. N-gons retain their corner sectors.
`ExtraordinaryVertex` accepts `G0`, `G1`, `G1x`, `G1xx` and `G2`.
The native bootstrap observed on the private oracle uses packed faces and the
G1x result; restart persistence and factory defaults are not established.

With preselection, enter `ToNURBS`, then `SubDOptions`. Type `Faces` or
`ExtraordinaryVertex` to choose a value, or use `name=value`. Choosing a value
returns to the submenu. Enter keeps a displayed choice, returns from the submenu
to confirmation, or converts from confirmation. Without preselection, pick the
objects first and press Enter. Mesh and SubD option menus coexist for mixed picks;
each submenu accepts only its own choices. The headless API also accepts flat options.

The [ordinary ToNURBS](to-nurbs.md) attribute, group, identity, ordering,
selection and deletion rules apply. Options are retained after real conversion,
including partial changes, and survive undo. Cancellation and failures preserve
the previous choices. Escape cancels from either the submenu or a choice prompt;
preselection stays selected, while command-first picks are cleared. Geometry is
staged before one model edit; resource and topology failures reject the entire mixed
conversion without consuming redo or changing sources or preferences.

## Geometry and evidence

The [base converter](../geometry/subd-nurbs-conversion.md),
[continuity processing](../geometry/subd-nurbs-continuity.md) and
[packing](../geometry/subd-nurbs-packing.md) supply the geometry. This is a finite
approximation to the SubD limit, without a continuous error certificate against
that mathematical surface. Domains are normalized with uniform source cells;
Rhino may use unequal parameter intervals and different knot vectors.

Two frozen actual-command families contain 35 workflows and 61 command steps.
They exercise all five modes, both packing choices, open grids, a subdivided box,
a torus, an n-gon prism and an extraordinary fan; mixed SubD/mesh/curve/no-op
inputs; both deletion choices; reversed command-first picks; partial option
memory; cancellation in selection, confirmation and the submenu; and undo/redo.
The second family observes settled selection after the macro, separately from
Rhino's EndCommand event, which can precede pick cleanup.

Rust replays import the captured 3DM sources. Complete positive tensor bounds
compare every source cell through recovered native charts at `1e-11`, with full
native coverage and oriented vertex/edge/trim incidence, including closed seams.
Spatial edge images are bounded against incident isocurves.
Maximum observed surface and spatial-edge bounds are `9.4e-14` and `2.2e-13`.
Identity, chronological order, names, layers, colors, ordered group memberships and complete group tables
match the captures; exact geometry snapshots also round-trip through undo/redo.
Python checks request/source hashes, native source CRCs, defaults and lifecycle
records. In-memory GUI tests exercise submenu nesting, invalid atomic edits,
mixed options, cancellation and history without opening windows.

[Soft edges and variable profiles](../geometry/subd-soft-nurbs.md) also convert through
these options. Their geometry has separate SDK qualification; the actual-command
families above use smooth and hard creases. Mesh/SubD edge conversion, `ShowFacePacks` and
`ForceUpdateFacePacks` prompt actions remain open; the standalone
[PackSubDFaces](pack-subd-faces.md) command can reset source packing before conversion.
Output B-reps do not yet carry Rhino's per-face pack IDs.
General performance parity is unqualified.
