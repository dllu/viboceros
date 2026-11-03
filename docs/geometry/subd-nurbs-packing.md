# SubD NURBS face packing

[Base conversion](subd-nurbs-conversion.md) · [Continuity modes](subd-nurbs-continuity.md)

`SubD::to_packed_nurbs_patches(mode)` combines rectangular quad grids after
extraordinary-vertex processing. `to_packed_nurbs_brep(mode, tolerance)` assembles
the corresponding B-rep. An n-gon retains its individual corner-sector surfaces.
`SubDNurbsPackedPatch` records the surface, grid dimensions and source tiles;
each tile identifies its face/sector, cell and chart rotation.

Domains are normalized to `[0,1]²`, with one uniform parameter cell per original
patch. Rhino can use different domains and unequal cell intervals. Comparisons
therefore certify complete images through recovered native cell rectangles;
they do not assert identical numeric parameters or minimal knot vectors.

## Partitioning and assembly

Retained native pack IDs are honored when their groups still form valid quad
rectangles. Public OpenNURBS queries read IDs and packing rectangles from a
private decode without changing the source archive. Topologically stale or incomplete
partitions fall back to topology-based packing, including after crease edits.

Automatic packing stops at nonregular corners, nonquad faces and crease
barriers. Tensor lines extend these barriers through regular smooth vertices.
Connected rectangles are laid out with oriented source-face charts. A shifted
period that cannot close a rectangle is split into a rectangular block and strip.
Coincident disconnected nets retain separate component identities.

Each tile is oriented, raised to the common degree and refined to the row/column
knot bases. Shared boundary controls are checked before tensor concatenation.
Local knot cleanup accepts a removal only when reinsertion gives a complete
positive-control bound within arithmetic roundoff. No global geometry fit is
used. Partitioning and cyclic grid traversal avoid quadratic component scans.

Perimeter chains merge at pack corners, including corners of neighboring packs
and n-gon edge midpoints. Shared source-edge paths identify spatial edges even
when a closed edge has identical endpoints. Wrapped charts get seam trims;
their measured cut conventions also cover one-cell strips and neighboring packs
with different cuts. Oriented incidence and complete B-rep validation precede
returning a result. Unmerged faces retain source order, followed by merged grids
in pack order. Sources remain immutable.

## Native evidence and limits

The frozen `subd_packing`, `subd_packing_holdouts` and `subd_packing_modes`
families contain 80 SDK workflows across 36 source profiles, producing 272 packed
surfaces from 1,724 source tiles. They cover open
grids, holes, L/T layouts, full/partial crease chains, rotated and reordered faces,
cylinders, tori, shifted periodic joins, coincident shells, extraordinary fans,
mixed quad/n-gon sources and twice-subdivided boxes. Packed grids adjacent to
extraordinary vertices exercise all five continuity modes. Recipes, producer
snapshots, source CRC checks and valid native outputs are independently verified.

Tests recover native tile domains, then compare whole surface images by exact
degree/knot refinement and positive tensor bounds at `1e-11`. Coverage checks
exclude missing or duplicate native rectangles. Oriented vertex/edge/trim
matching includes periodic seams and coincident components. Complete spatial
curves are bounded against incident surface isocurves. A native shared edge can
inherit one incident chart's parameter speed; the other chart may use different
cell lengths while retaining the same certified boundary image.
Maximum observed surface and spatial-curve bounds are about `1.4e-13` and
`2.6e-13`, respectively.

Separate 3DM-import tests verify retained IDs/charts, packed topology, source
immutability and invalidation after a crease edit. Output is limited to four
million controls, with tensor admission before common-basis refinement; existing
SubD refinement budgets also apply. Soft sharpness, command integration and
general performance parity remain open. The finite approximation still has no
continuous error certificate against the mathematical SubD limit. B-rep output
does not yet carry Rhino's per-face pack-ID user data.
