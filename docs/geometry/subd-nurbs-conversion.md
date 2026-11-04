# SubD to NURBS base conversion

[Limit charts](subd-limit-faces.md) · [SubD geometry](../subd.md) · [ToNURBS command](../commands/to-nurbs.md)

`SubD::to_nurbs_patches_g0()` builds unpacked cubic surfaces in normalized source
charts. Regular faces retain exact bicubic Bezier patches. Other quads use eight
uniform spans per axis; n-gon sectors use four. This implements the base G0
approximation before extraordinary-vertex continuity processing.

The native adapter collects refined control grids and neighboring control rows
through public OpenNURBS topology. Smooth sides use the adjacent face; crease
sides reflect the interior row. Rust completes each missing outside diagonal
using the uniform cubic `[1,4,1]/6` tensor mask and the exact limit corner. Original
quad corners with two crease sides use the corresponding one-level tensor
refinement mask. N-gon sectors account for their initial subdivision separately.
There is no global least-squares fit or geometry-dependent tolerance search.

`SubD::to_nurbs_brep_g0(tolerance)` sews these surfaces by source component
identity. An n-gon creates one face per corner. Its boundary midpoints split
shared spatial edges and neighboring quad trims, retaining the quad's whole
surface. Source vertices, edge midpoints and face centers have distinct topology
keys. Coincident disconnected components remain separate. All shared curves,
trim directions, isoparametric classes and complete output topology pass B-rep
validation before a result is returned. Source geometry is immutable.

Stencil extraction retains the limit-query resource budgets, including a
one-million-refined-face limit and four-million-point limit including halo data.
An entirely regular input bypasses subdivision. A regression converts 32,768
regular faces even though unnecessary stencil refinement would exceed its budget.

## Native qualification and limits

The immutable `subd_to_nurbs_g0` and `subd_to_nurbs_g0_holdouts` families contain
13 public Rhino 8 SDK workflows and 112 output patches. They include smooth and
hard boundary corners, a quad grid, smooth/creased boxes, hard/smooth pentagons,
a regular torus, valence-five/seven fans, a mixed quad/ngon prism, an affine grid,
and reversed winding. Request hashes and frozen producer snapshots are verified
by `tools.rhino_oracle.test_subd_g0`.

`subd_g0_patches` checks the complete surface images by exact refinement to a
common positive tensor basis. Maximum observed bound is about `1.6e-13`, against
an asserted `1e-11` limit. Spatial edge curves receive the same control-basis
comparison. Vertex/edge counts, oriented face cycles, trim incidence and solidity
are compared to native outputs. Further tests cover coincident shells, mixed
trim splits, resource admission and immutable variable-sharpness rejection.

Surface domains are normalized to [0,1], and redundant knots can differ from
Rhino. [G1, G1x, G1xx and G2 processing](subd-nurbs-continuity.md) is available through the
mode-based API; [rectangular packing](subd-nurbs-packing.md) combines the patches
and assembles periodic seams. [Single uniform soft edges](subd-soft-nurbs.md)
add local dyadic spans. The finite G0
approximation has no continuous error certificate against the mathematical SubD
limit. Arbitrary nonmanifold
inputs, degenerate patches and comparative performance remain unqualified.
Parallel control edges joining the same vertex pair currently reject.
[Whole-SubD command options, metadata and undo/getter workflows](../commands/to-nurbs-subd.md)
are covered by separate native command comparisons.
