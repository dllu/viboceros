# ExtrudeCrvToPoint surfaces

Select curves, then enter an apex or start the command and pick it:

```text
ExtrudeCrvToPoint 0,3,2 Output=Surface Solid=Yes SplitAtTangents=Yes
```

`SplitAtTangents=Yes` is the default. It partitions G2 discontinuities at one
degree before global `CreaseSplitting` independently admits G1 breaks at 0.1
degrees. Each stage creates shared edges and retains singular pole trims.
A smooth tangent join can split under G2 alone; small corners can split under
global G1 alone. Disable both to retain one wall surface.

Open and nonplanar profiles produce sheets. Their U direction runs from the
profile to the apex; V retains the profile interval. Lower-multiplicity knots
are refined through usd-brep in native builds, or shared Rust NURBS operations,
before constructing boundary tables. Subsequent partitions retain the existing
splitter's exact-locus certificates and work budget. Rational refinement retains
the exact constant apex row. Unclamped profiles use the shared exact constructor
to preserve their basis across the native importer's active-end clamping.

`Solid=Yes` caps closed planar profiles. The cap precedes the walls, with a
piecewise Bezier UV boundary and shared original-profile edges. Explicit
splitting constructs each profile piece first; global crease splitting follows.
Wall U runs along the reversed profile and V toward the apex. Its interval is
estimated from 17 profile samples, separately for explicitly split pieces.
A public OpenNURBS support-plane query supplies the cap frame. Equivalent
in-plane axes can differ from Rhino's cap chart.

`DeleteInput=Yes` removes inputs in the same undo step. Outputs have fresh
attributes on the current layer, remain unselected and ungrouped. Options are
editable before the apex pick; invalid groups and Escape preserve the model
and pending Redo. Options reset on each invocation. Switching to
[SubD output](extrude-crv-to-point-subd.md) resets Solid and SplitAtTangents to No.
`ToBoundary` remains unsupported.

The [validation record](../apex-splitting-validation.json) covers 71 native
command workflows, their application replay and 3DM round trips in both builds,
plus three independent SDK plane cases. Full used surface, edge and trim
definitions, tolerances, face senses, incidence and before/after/undo/redo states
are compared at absolute epsilon `1e-12`. Correspondence permits component
numbering and equivalent cap-axis exchanges; unused native surface-table slots
are excluded, and the two unused full-vector padding knots omitted by 3DM are
normalized. General continuous error bounds and comparative performance are
unqualified. Shared resource limits apply.

```sh
cargo test -p viboceros-oracle --test apex_splitting --test apex_splitting_archive
cargo test -p viboceros apex_splitting
python3 -m unittest tools.rhino_oracle.test_apex_splitting
```

Native captures use private Xvfb through `tools/rhino_oracle/run_headless.sh`.
Reference: [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvtopoint.htm).
