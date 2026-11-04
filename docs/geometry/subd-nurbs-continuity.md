# SubD NURBS continuity processing

[Base conversion](subd-nurbs-conversion.md) · [Limit charts](subd-limit-faces.md)

`SubD::to_nurbs_patches(SubDNurbsContinuity::G1)` applies local tangent-plane
processing to the unpacked G0 patches. `G2` processes a larger neighborhood;
`G0` retains the base conversion. `to_nurbs_brep(mode, tolerance)` assembles the
corresponding shared B-rep topology and validates the complete result.
`G1xx` adds [cyclic tangent/twist compatibility](subd-nurbs-g1xx.md), with cubic
odd-valence and chart-dependent quartic even-valence processing.

`G1x` starts with G1 and improves the sampled normal gap near each processed
edge end. It compares the two incident patches at seven interior eighths of the
first shared source interval, using a one-degree cosine threshold. Unequal first
spans use their smaller source reach. Up to four quarter-step
projections move the second edge control inward from the corner and its adjacent
interior control.
Both incident patches receive the same displacement, preserving their shared
edge. The sampling convention matters: a denser check changes native output
decisions in the near-threshold controls. It is not a continuous normal-gap bound.

Only original smooth source vertices with valence other than four receive this
processing. Crease, corner and dart vertices remain unchanged. Newly generated
n-gon face centers are also unchanged. G1 projects the nearest two-by-two control
neighborhood, keeping the corner itself fixed. G2 projects the three-by-three
neighborhood, giving zero normal curvature at that vertex. These are local
processing modes; they do not make every shared edge globally G1 or G2.

G1, G1x and G2 normals come from public OpenNURBS smooth-sector limit evaluation in a
private decoded source. Requested source vertex order and duplicate requests
are preserved. Nonsmooth indices, unavailable normals and invalid input reject.
No tessellation is constructed. Smooth-vertex cache reuse avoids repeated
high-valence face-fan searches for each corner. Projection uses coordinates
relative to the retained corner, and sources remain immutable.

## Native evidence and remaining modes

The frozen `subd_continuity` family contains 85 SDK workflows: 17 sources in all
five native modes. Sources cover previous G0 shapes plus a skewed box, one-crease
and crease-chain boxes with dart vertices, and an asymmetric fan.
`test_subd_continuity.py` verifies recipes, producer snapshots, valid outputs and
source CRC checks. G1x and G1xx observations remain distinct: the valence-seven
fan changes under G1x, and G1xx introduces double knots near extraordinary
vertices. Neither mode is silently treated as G1.

The `subd_g1x` family adds 48 G0/G1/G1x workflows across 16 valence, height,
affine and scale profiles, including valences six, eight and nine. Fixed profiles
straddle the sampled normal-gap threshold. Producer snapshots, recipes, valid
native output and source CRC checks remain immutable and independently verified.
The `subd_g1x_sectors` and `subd_g1x_charts` families add 36 workflows covering
triangle sectors, mixed quad/triangle seams, rotated face corners and reversed
winding, including seams with different parameter scales on each side.

Five G1xx families add 43 workflows covering quartics, adjacent odd/even vertices
and native UV-corner conventions; their evidence is described with the algorithm.

`subd_continuity` compares 212 implemented workflows, including 2,049
surface patches, complete edge curves and oriented B-rep topology. Exact common
basis refinement and positive control bounds verify whole images at `1e-11`;
the maximum observed surface bound is about `7.2e-13`. Separate tests check
normal selection/order, unit length, invalid queries and immutable sources.
Shared comparison helpers also serve
the original G0 regressions.

[Packing](subd-nurbs-packing.md) is available after continuity processing.
The [ToNURBS command](../commands/to-nurbs-subd.md) exposes these choices.
[Single uniform soft edges](subd-soft-nurbs.md) are supported with the documented
continuity restrictions. Domains
remain normalized, redundant knots may differ from Rhino, and this finite
conversion has no continuous error certificate against the true SubD limit.
