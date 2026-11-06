# OpenNURBS Rust adaptations

`length.rs` adapts `ON_Length3d` from `opennurbs_point.cpp` at the pinned
revision below, under the [MIT license](LICENSE). It retains component scaling,
operation order and denormal handling. Corner-surface charts use this explicit
entry point to preserve native parameter extents at remote coordinates; the
general geometry distance functions retain their existing arithmetic. Line and
Polyline commands also use this policy for their parameter domains.

`line_projection.rs` adapts `ON_Line::ClosestPointTo` and `PointAt` from
`opennurbs_line.cpp` at the pinned revision below, under the [MIT license](LICENSE).
It retains endpoint-conditioned parameter calculation/evaluation and uses validated
Rust geometry types, rejecting unrepresentable or degenerate lines. Plane's
Vertical point getter and Line construction use the actual finite endpoints
and ordinary native dot-product order; the geometry
kernel's ideal directional projection remains independent.

`chord_adjust.rs` adapts the point-list/NURBS chord-adjustment policy from
`opennurbs_brep.cpp` (`AdjustPointListAlongChord`, `AdjustNurbsCurve`) in the
pinned OpenNURBS submodule at `23fc677ba06e49212296ca75fab7fb6c2851b4ce`.
The original source copyright notice is retained; see [LICENSE](LICENSE) and
[McNeel's developer-tools license](https://developer.rhino3d.com/license/).

This modified Rust port uses exact rational chord projection and displacement
with one final rounding per coordinate. It keeps the short-chord fallback;
closed equal-endpoint curves translate without a minimum displacement cutoff.
The geometry kernel retains weights/knots and certifies resulting uncertainty.
No proprietary Rhino source was used. This is not an upstream McNeel component.

`viewport_clipping.rs` adapts the constrained `ON_Viewport::SetFrustumNearFar`
overload from `opennurbs_viewport.cpp` at the same pinned revision. It retains
the original copyright/disclaimer above and uses the [developer-tools MIT
license](LICENSE). The modified Rust routine returns near/far distances and a
parallel camera dolly rather than mutating an OpenNURBS viewport. It rejects
nonfinite inputs and uses the measured Rhino `ViewportInfo` default of 0.0005
for an invalid ratio constraint. Viboceros's document clipping padding and
frustum intersection implementation remain in `src/viewport/clipping.rs`;
those are independently calibrated/implemented from public outputs and math.

`brep_shrink.rs` adapts the interval policy of `ON_Brep::ShrinkSurface` from
`opennurbs_brep.cpp` at the same pinned revision under the [MIT license](LICENSE).
It retains outer bounds, conditional 1% padding, surface-side suppression,
domain intersection, and the `2^-32` relative stopping rule. It returns intervals
instead of mutating native surfaces and omits the unexposed side-disable mask.
Its margin switch supports the independently observed `ToEdge` command. The
kernel computes tight runtime UV bounds independently (rather than using the
archive's control-point box), crops exact NURBS surfaces, updates isoparametric
flags, and validates retained topology. No proprietary Rhino source was used.

`line_rotation.rs` adapts `ON_Xform::Rotation` and `ON_3dPoint::Transform` from
`opennurbs_xform.cpp` / `opennurbs_point.cpp` at the same pinned revision under
that [MIT license](LICENSE). This altered port retains ordinary matrix operation
order and center translation when rotating both finite Line reference endpoints.
It uses validated unit axes and Rust sine/cosine pairs; finite arithmetic failures
return errors. The Line command independently follows the public `ON_LineCurve`
constructor's unit-domain fallback at lengths no greater than `2^-32`.

`rotation.rs` adapts cardinal-angle noise cleanup from `ON_Xform::Rotation` in
`opennurbs_xform.cpp` at the same pinned revision under the [MIT license](LICENSE),
retaining the source copyright and disclaimer. It receives Rust sine/cosine
outputs, omits the native normalization of supplied components, and returns
only snapped components. Rotate, Rotate3D and the Twist point map use the
kernel’s explicit `try_rotation_with_cardinal_cleanup` entry point. The general
`try_rotation` entry point preserves small rotations without this cutoff.

`box_topology.rs` is a modified Rust adaptation of the connectivity tables in
`ON_BrepBox` (`opennurbs_brep_tools.cpp`) and parameter-extent policy in
`ON_NurbsSurfaceQuadrilateral` / `ON_LineCurve` at pinned revision
`23fc677ba06e49212296ca75fab7fb6c2851b4ce`, under the [MIT license](LICENSE).
It retains the source copyright/disclaimer, native vertex/edge/face order,
max-opposite-edge surface domains, and the tiny-length unit-domain fallback.
The Rust kernel independently constructs and validates geometry/topology in
`Brep::try_command_box`; `Brep::try_box` keeps its normalized convention.
