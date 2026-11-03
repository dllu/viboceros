# SubD NURBS continuity processing

[Base conversion](subd-nurbs-conversion.md) · [Limit charts](subd-limit-faces.md)

`SubD::to_nurbs_patches(SubDNurbsContinuity::G1)` applies local tangent-plane
processing to the unpacked G0 patches. `G2` processes a larger neighborhood;
`G0` retains the base conversion. `to_nurbs_brep(mode, tolerance)` assembles the
corresponding shared B-rep topology and validates the complete result.

Only original smooth source vertices with valence other than four receive this
processing. Crease, corner and dart vertices remain unchanged. Newly generated
n-gon face centers are also unchanged. G1 projects the nearest two-by-two control
neighborhood, keeping the corner itself fixed. G2 projects the three-by-three
neighborhood, giving zero normal curvature at that vertex. These are local
processing modes; they do not make every shared edge globally G1 or G2.

Normals come from the public OpenNURBS smooth-sector limit evaluation in a
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

`subd_continuity` compares the 51 implemented G0/G1/G2 workflows, including 405
surface patches, complete edge curves and oriented B-rep topology. Exact common
basis refinement and positive control bounds verify whole images at `1e-11`;
the maximum observed surface bound is about `1.6e-13`. Separate tests check
normal selection/order, unit length, invalid queries, immutable sources and
explicit rejection of unfinished modes. Shared comparison helpers also serve
the original G0 regressions.

G1x and G1xx are represented in `SubDNurbsContinuity` but currently reject before
fitting. Packing, soft sharpness and command integration remain open. Domains
remain normalized, redundant knots may differ from Rhino, and this finite
conversion has no continuous error certificate against the true SubD limit.
