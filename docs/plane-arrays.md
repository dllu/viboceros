# Construction-plane arrays

[Transforms and arrays](commands/transforms.md) · [CPlane](cplane.md)

`Array`, `ArrayLinear`, and `ArrayPolar` share the independent `arrays` command
module. All retain the original selection and object attributes. Each command
is one atomic model edit; failed geometry preparation creates no partial copies.
No-op rectangular arrays add no undo record. CPlane edits remain outside model
undo history.

| Command | Layout coordinates |
| --- | --- |
| `Array nx ny nz dx dy dz [Mode=UnitCell\|Fill]` | Active construction-plane X/Y/Z axes; the plane origin does not affect spacing. |
| `ArrayLinear count base destination` | Full world-space vector between the reference points, independent of plane orientation. |
| `ArrayPolar count center angle [Rotate=Yes\|No] [ZOffset=height]` | Rotation about the plane normal through the world-space center; height accumulates along that normal. |

Counts include the original item. Rectangular axis counts may be one; the other
two commands require at least two items. Exactly +360 or -360 degrees omits the
duplicate endpoint; other nonzero sweeps, including multi-turn angles, include
both endpoints. `Rotate=No` translates the source set by orbiting the center of
its combined **world-axis tight bounding box**, even on a tilted CPlane.

Enter `ArrayLinear 4` to pick two references, or `ArrayPolar 4 180` to pick the
center. Transparent `CPlane` changes during these prompts are supported. Polar
orientation comes from the plane at the center pick. One-line point arguments
are world coordinates; interactive points use the shared [typed-point rules](point-input.md).
`Array 3 2` accepts two corners measured in the first pick's plane, ignoring
the second pick's normal component. A third count requires `ZDistance=...`.
This two-corner convenience prompt is not Rhino's full count/spacing/preview UI.

## Rectangular distances and groups

`UnitCell` uses the signed distances directly. `Fill` measures source bounds in
construction-plane orientation. For an active axis with extent `e`, requested
length `L`, and count `n`, the measured Rhino spacing rule is:

```text
spacing = sign(L) * abs(L - e) / (n - 1)
```

Thus small positive lengths are valid, and negative lengths add the source
extent to the spacing magnitude. Zero active Fill lengths are rejected.
Equal lengths/extents give zero spacing. Every zero-displacement cell is
omitted, but coincident copies at nonzero displacements are retained.

An array of one selected object leaves copied objects ungrouped, but still
allocates corresponding empty group definitions. An array of multiple objects
recreates each selected group membership independently per copy, including
single-member groups. Ordinary `Copy` has the same single-source distinction;
morph-copy APIs retain their own policy. Original groups are never removed.
The document's explicit `CopyGroupPolicy` distinguishes preserved memberships,
definitions-only copies, and complete omission. See [ordered groups](groups.md).

## Bounds and remaining scope

All seven curve families use [tight curve bounds](curve-bounds.md) for Fill
extents and the nonrotating polar anchor. Point, point-cloud, polyline, and mesh
extents come from their exact vertex sets. Standalone NURBS surfaces use
[tight untrimmed surface bounds](surface-bounds.md). B-reps use
[complete trimmed-face bounds](trimmed-face-bounds.md), including holes and
interior extrema. Surface layout is
not yet guaranteed to match Rhino; the retained comparisons below expose the
remaining differences.

This change does not add Array's graphical preview, full spacing prompts, preview
length-edit prompts, ArrayPolar's picked angle, or CPlane adaptation for
`ArrayCrv`/`ArraySrf`. Command parsing and geometry remain independent of egui.

## Verification and oracle distinction

`plane_arrays.json` contains 64 actual Rhino command cases: all six world
planes, oblique/shifted planes, asymmetric polynomial and rational curves,
closed rational curves, signed/small/zero-spacing Fill cases, both rotation
policies, multi-turn sweeps, group memberships, selection, native domains,
and 33 unrounded parameter samples per output curve. All 64 agree at absolute
`1e-8`, relative `1e-12`; maximum observed coordinate error is `1.92e-9`.
This older probe records only populated groups on both sides. The separate
`group_memberships` probe compares the entire table, including automatic empty
definitions, and retains per-object membership order rather than sorted subsets.
Unit tests separately check failure atomicity, undo/redo, large plane origins,
interactive plane changes, trimmed B-rep extents, and preservation of their
underlying surfaces and UV loops.
`surface_array_bounds.json` adds 32 surface/mixed-source cases; 24 pass at the
same epsilon. Eight retain translation-only discrepancies, including oblique
Fill errors up to `0.01817`. See [surface-bound evidence](surface-bounds.md)
for exact coverage and separate Rhino boxes that exclude their own samples.
`trimmed_brep_array_bounds.json` adds 32 trimmed/multi-face B-rep arrays.
13 polar cases pass; 19 retain translation-only differences from `2.39e-8`
through `0.001563`. See [trimmed-face evidence](trimmed-face-bounds.md) for the
case groups, analytic bounds, and unchanged strict epsilon.
A private-display release UI check additionally exported 20 line objects in
10 groups after nested CPlane edits, polar undo/redo, rectangular UnitCell/Fill,
and picked linear references; all exported endpoints agreed within `1e-10`.

Normal Fill fixtures explicitly set XLength/YLength/ZLength in Rhino's preview.
The initial numeric 3D height prompt produced viewport-dependent Z placements
in the tested Rhino 8 Wine/FEX session, despite reporting the requested ZLength.
`plane_array_diagnostics.json` retains two initial-height cases separately;
they are not passing geometry references and their epsilon is not widened.
The worker restores all construction planes, model-aid settings, and selection,
deletes only owned objects/groups, and handles reused deleted group-table slots.

Public command semantics are documented in [Rhino's Array help](https://docs.mcneel.com/rhino/8/help/en-us/commands/array.htm).
Implementation and edge-case policies were checked through public command/API
outputs, without inspecting proprietary implementation code.
