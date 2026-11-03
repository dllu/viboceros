# NURBS polygon meshing

[Mesh and mesh primitives](commands/mesh-primitives.md) · [B-rep meshing](brep-meshing.md)

`Mesh Density=0..1` adapts the surface grid to the source geometry. Positive
density maps object size to a chord-height target using public OpenNURBS's
`ON_MeshParameters::ToleranceFromObjectSize` rule. Knot directions of degree greater than one start
with four intervals; degree-one directions start with one. The mesher doubles
whichever direction most reduces the geometric error until every cell passes.
This avoids filling straight extrusion directions with unnecessary rows.
Zero density retains the coarse grid. Bilinear and `SimplePlanes=Yes` planar
fast paths keep their existing coarse behavior.

Each knot rectangle is extracted as a rational Bézier net. For a patch
`S=N/W` and its corner bilinear interpolant `L`, degree elevation and the
Bernstein product give control coefficients for `(N-WL)/W`. Same-sign
denominator coefficients bound the residual by the largest residual-control
norm. Adding one quarter of the corner-twist norm bounds the displacement
from the bilinear interpolant to the two mesh triangles. This detects bumps
and oscillations which endpoint/midpoint sampling can miss. Homogeneous
subdivision retains intermediate zero weights; unresolved poles return an
error. The computation includes a floating-point rounding allowance and does
not provide an interval-arithmetic certificate.

U and V counts can differ for standalone surfaces. B-reps currently use their
maximum count in both directions so shared-edge audits and the conforming
fallback retain a common boundary grid. Both paths check a 1,048,576-vertex
grid budget before allocation; refinement also has an arithmetic-work budget.
Refinement failures leave a `Mesh` command's sources, selection and redo intact.

The error estimate concerns untrimmed knot cells before seam snapping.
Trim-boundary approximation and constrained trimmed-face triangles do not yet
have the same bound. Detailed native meshing controls and identical Rhino
tessellation counts remain unqualified. Analysis panels still use their existing
NURBS/B-rep sampling; their density controls require further integration.

Frozen Rhino 8.32 records cover 54 public SDK combinations: six surface shapes
with density presets, default analysis settings, chord, angle and maximum-edge
settings. Captured vertex samples agree with the corresponding Rust surface
geometry within `1e-6`. Independent refinement tests cover sampled chord errors,
rational weights, missed midpoint waves, translated geometry and pole failures.

```sh
cargo test -p viboceros-geometry --lib meshing::quality
cargo test -p viboceros-command --lib mesh_quality
python3 -m unittest tools.rhino_oracle.test_nurbs_meshing
```

References: [public OpenNURBS density rule](https://github.com/mcneel/opennurbs/blob/23fc677ba06e49212296ca75fab7fb6c2851b4ce/opennurbs_mesh.cpp#L7383),
[Rhino mesh parameters](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.meshingparameters/).
