# Surface Inset output modes

[Surface Inset](inset-surfaces.md) · [Curved boundaries](inset-curves.md)

`Split` and `JoinOutputs` apply to supported planar surface faces. Mesh and SubD
inputs keep their existing face-inset behavior.

| Options | Geometry | Object identity and attributes |
| --- | --- | --- |
| `Split=Yes JoinOutputs=Yes` | Joined cap and borders | Replaces the original geometry, retaining its ID, attributes and groups |
| `Split=Yes JoinOutputs=No` | Separate surface components | Removes the original; creates unselected objects with default attributes on the current layer |
| `Split=No` | Closed inset loops and crease connectors | Retains the original unchanged; creates unselected curves with default attributes on the current layer |

```text
Inset 0.3 Split=No CreaseCorners=Yes
Inset 0.3 Split=Yes JoinOutputs=No
```

Separate components remain connected through original edges. New edges across
components are cut; new edges within a component remain joined. Borders can
therefore remain attached to unselected faces of a polysurface. Curve output
creates one joined curve per cap boundary and, when applicable, individual crease
connectors. It supports rational and periodic NURBS boundaries, holes and tilted
planes with the same admission as joined Inset.

`Split=Yes` and `JoinOutputs=Yes` are initial defaults. Both retain their last
completed values, independently of Undo/Redo. `JoinOutputs` is hidden while
`Split=No`; its remembered value survives. Cancel discards pending option changes.
`Loose` still resets Off each invocation. Curve output retains source component
picks; surface outputs clear them. `SelNone` and history replay release transient
component picks.

Previews stage all output geometry before modifying the document. Completion is
one history edit; Undo restores removed originals and their attributes, groups
and selection, and Redo restores the same output IDs. Stale sources reject
completion. Creation uses current-layer default attributes, rather than copying
the source's name, color or group memberships.

Curve encodings, parameter clocks, object enumeration and raw topology table order
can differ from Rhino. Comparisons match output objects bijectively by geometry,
then check metadata, oriented trim loops, edge incidence and spatial witnesses.
Polygon, analytic and Loose loci use `1e-10`; independent tight command fits allow
`2e-6` at `1e-6` model tolerance. Outputs also roundtrip through 3DM.

Frozen evidence adds 72 curved output cases, 12 preference steps and 16 limit
controls to the earlier polygon cases. Checks cover 96 ordinary output cases,
four large ellipse insets and 12 native failures. Another 72 commands cover
[cusp limits](inset-cusps.md) in all three output modes: 66 successes and six
unchanged-source collapse failures.
Touching/crossing annuli are rejected even with curve output. Existing planar
Inset work/storage limits apply; separate output also uses the edge-separation
limit of 100,000 cuts. Preview geometry shares the 400,000-segment limit.

```sh
cargo test -p viboceros-oracle --test inset_surface output_modes
cargo test -p viboceros --bin viboceros inset_output
python3 -m unittest tools.rhino_oracle.test_inset_outputs
```
