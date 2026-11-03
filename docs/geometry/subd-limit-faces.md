# SubD limit charts and exact patches

[SubD geometry](../subd.md) · [ToNURBS](../commands/to-nurbs.md)

`SubD::limit_face_grids(density)` evaluates oriented dyadic face charts without
editing the source. Quads have `2^density` intervals per axis. Other polygons
have one sector per original corner, with `2^(density-1)` intervals. A sector's
corners are the original vertex, outgoing edge midpoint, face center, and incoming
edge midpoint. Points have row-major order with u varying fastest; source face
and corner ordinals survive subdivision, including coincident components.

The adapter tracks public OpenNURBS subdivision lineage. Child-face searches use
edge-midpoint fans, avoiding repeated scans of high-valence source vertices or
polygon centers. Before subdivision it checks a one-million-face limit,
four-million-vertex/edge limits, and four-million-output-point limit. Density must
be 1–6. Nonmanifold edges reject. These are resource budgets, not geometric error
bounds; interpolation between evaluated points has no tolerance certificate.

`SubD::regular_nurbs_patches()` extracts exact bicubic Bezier patches through
public `ON_SubDFace::GetQuadSurface`, then clamps both axes to [0,1]. The returned
slots address source face ordinals. Unsupported neighborhoods remain `None`,
including n-gons, extraordinary or sharp neighborhoods, and some neighborhoods
near corners. This intentionally preserves the public extractor's conservative
admission rules. It performs no approximation.

## Native controls and remaining conversion work

The immutable `subd_to_nurbs_sdk` capture contains 30 public Rhino 8 SDK cases:
seven source nets, packed/unpacked faces, and all five extraordinary-vertex modes
for a smooth box and pentagon. Each retains source control data, a source 3DM,
full output B-rep/spline definitions, and source CRC checks. Seven cases also
record clamped and unclamped limit-edge NURBS curves. Producer snapshots and the
request hash are verified by `test_subd_to_nurbs.py`.

| Source | Unpacked faces | Packed faces |
| --- | ---: | ---: |
| Single quad, smooth or hard corners | 1 | 1 |
| 4 by 3 quad grid | 12 | 1 |
| Smooth or partially creased box | 6 | 6 |
| Pentagon | 5 | 5 |
| Regular torus | 32 | 1 |

`subd_limit_faces` compares every density-two station to the native surface mesh
for all seven nets, plus source edges and corner identities. Native n-gon fragment
charts start at the face center, so their station arrays require a fixed
180-degree rotation. Regressions cover refined active levels, soft creases,
coincident components, invalid inputs, and resource limits.

`subd_regular_patches` compares all 32 torus patches against native unpacked
ToBrep Bezier controls. Matching positive bases bound the complete surface image
within `1e-12`; this check does not rely on surface sampling. Other tests check
source slots, orientation, active levels, and soft-sharpness exclusion.

Whole-SubD ToNURBS conversion remains unimplemented. The open-source OpenNURBS
`ON_SubD::BrepForm` returns null; exact patch extraction and sampling are separate
foundations for an independent assembler. Packing, extraordinary-vertex fitting,
B-rep adjacency, command options, history and native command comparisons remain.
The SDK controls do not qualify those command workflows or performance. Rhino's
[ToNURBS reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tonurbs.htm)
describes the target packing and continuity options.
