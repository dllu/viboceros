# AreaCentroid

[Command index](README.md) · [Area and volume integration](../mass-properties.md)

Select objects and enter `AreaCentroid`, or enter the command first, pick objects,
and press Enter. One point marks the cumulative area-weighted centroid of the
eligible selection. It is not a vertex average, perimeter centroid, or volume
centroid. Groups do not partition or multiply the measurement; a directly selected
subset of a group contributes only that subset.

Supported inputs are closed planar curves, NURBS surfaces, trimmed B-reps
(including holes), and polygon meshes. Open/nonplanar curves and points are
excluded. A numerical failure on eligible geometry aborts the complete operation,
without leaving a partial marker. If no selected geometry is eligible, the command
fails and clears selection.

The marker is unselected, on the current layer, unnamed, and ungrouped. Sources
and their attributes remain unchanged. Preselection is preserved, including
excluded objects in a mixed selection; successful command-first picking clears
selection. Marker creation is one native undo step. Esc cancels picking.

## Kernel and numerical limits

The independent `AreaMassProperties` kernel type stores area and three first
moments. Exact rational aggregation avoids premature rounding or overflow of
weighted coordinates across objects; a centroid can remain representable when
the total area is not. This does not make numerical quadrature or square roots
exact.

- Circles and ellipses use analytic area and center.
- Other closed planar curves become rational B-rep trim boundaries, without
  display tessellation.
- Surfaces integrate area and first-moment densities over each knot rectangle.
  Arbitrary B-rep trims reuse the Green-theorem boundary traversal, including
  holes and surface-knot crossings. Face reversal does not change area mass.
- Mesh faces contribute triangle areas and their centroids. Unused vertices
  contribute nothing. An allocation-free exact-product accumulator handles the
  ordinary mesh hot loop; exceptional coordinate/area ranges use rational edges.

Surface controls are centered and scaled before integration, then the first
moments are transformed back without prematurely rounding the area. Four scalar
adaptive integrations compute the moments. Rectangle integration has a shared
two-million-evaluation limit; arbitrary trims have the existing limit for each
density. Invalid geometry, lost coordinate range, nonconvergence, and exhausted
work budgets return errors. Arbitrarily ill-conditioned UV domains are not
guaranteed to succeed. There is no performance-parity claim for this new command.

### Warped quads

Mesh `Area`, `AreaCentroid`, and `TriangulateMesh` use the shorter spatial quad
diagonal; an exact tie retains A-C. Near-equal or overflowing chord lengths use
exact squared-distance comparison. For a warped quad with equal diagonals,
cyclically reordering vertices can therefore change its measured area. Display
triangulation is unchanged. The subsequent [VolumeCentroid audit](volume-centroid.md)
also established and implemented the shorter-diagonal policy for mesh `Volume`.

## Validation and retained differences

The [source-only request](../../tools/rhino_oracle/fixtures/area_centroid.json)
and [complete observations](../../tools/rhino_oracle/observations/area_centroid.json)
retain 54 public Rhino 8.32.26160.13001 cases from an owned private-Xvfb session:
group and direct-subset selection, command-first picking, rejected open input,
concave and curved boundaries, six trimmed B-reps, mixed geometry, and 32 quad
rotation/winding controls. There are 53 successful commands and one no-eligible-
geometry failure. All 35 mesh source occurrences preserve binary64 vertices and
face indices exactly. Geometry is recorded before and after command execution.

Rhino reverses the inward capped B-rep on document insertion. The harness accepts
that only after a public duplicate-and-Flip operation reproduces the complete
stored geometry definition. Both constructed and stored definitions are retained;
area mass is orientation-independent. See [provenance](../area-centroid-provenance.json).

At absolute `1e-9`, relative zero, **46/54** default API plus actual-command
comparisons pass. Success, selection, marker count, and marker attributes agree
in all cases. The eight numerical differences are `shape-4`, `mixed-group`, and
the six `brep-paraboloid-*` cases. Rhino's
[default mass-property integration tolerances are 1e-6](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_AreaMassProperties_Compute_2.htm).
Separate API comparisons requesting absolute `1e-9` and relative `1e-12`
integration tolerances pass **52/54**; `shape-4` and
`brep-paraboloid-disk` still differ by up to `8.23e-9` and `1.43e-9`, respectively.
These are retained failures, not widened-epsilon passes or replacement targets.

Seven [independent references](../../tools/rhino_oracle/fixtures/area_centroid_integrals.json)
use analytic radial integrals and 68-digit Decimal Gauss-Legendre integration
for the bilinear surface. Independent 32/64-point rules agree within `1.4e-54`;
native area and centroid components agree with those references within `7.5e-14`
in this run (regression threshold `1e-11`). The reference generator uses source
definitions, never observed Rhino values. Additional native tests cover large
translations, tiny/huge areas, exact-product accumulation, invalid boundaries,
atomic failure, actual UI picking/cancellation, and undo/redo. Rhino Undo/Redo
and exact command-history strings are not compared in this probe.

```sh
# Retained replay; exits 1 while the documented numerical differences remain.
python3 -m tools.rhino_oracle.area_centroid_replay \
  tools/rhino_oracle/fixtures/area_centroid.json \
  tools/rhino_oracle/observations/area_centroid.json
# The same command with --tight-api compares per-object API values only.
python3 -m unittest tools.rhino_oracle.test_area_centroid
```

Fresh public-API capture uses `tools/rhino_oracle/run_headless.sh rhino` with the
same request and an owned isolated Rhino session. General behavior is described
in [Rhino's AreaCentroid reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/areacentroid.htm).
