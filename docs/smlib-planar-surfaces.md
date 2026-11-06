# Planar surface construction

[PlanarSrf behavior and qualification](commands/planar-srf.md) ·
[Native modeling](native-modeling.md)

Both command builds share the closed-loop region planner. It distinguishes
stored seam parity from whole-loop hole admission, isolates source planes and
preserves Rhino's observed duplicate-encoding behavior. The earlier portable
fixture produced areas 99, 16 and 16; the corrected planner produces areas 83
and 16, matching Rhino. Wider input-order and seam captures also exposed native
multi-profile grouping differences. Native construction now receives each
planned region independently.

The native adapter imports all profiles before creating output. Each region
uses usd-brep's `CreatePlanarFacesWith3DCurves`; construction, transfer or loop
multiplicity failure falls back to the exact Rust constructor. The receiving
B-rep gets the document tolerance explicitly because the upstream high-level
wrapper ignores its numeric tolerance argument. Source copies are consumed on
success, and partial failure detaches copied curves before cleanup. Caller-owned
profiles remain usable. The pinned upstream submodule is unchanged.

PlanarMesh retains its separate independent-contact policy. Its native plane
grouping still uses `AreCoPlanar` on the entire candidate group. Existing concave
and contacting-boundary fallbacks remain in place. See [PlanarMesh](commands/planar-mesh.md)
for its separate surface and tessellation qualification.

Repeated polygon hole loops remain in the model. Triangulation coalesces only
identical sampled polygon walks, keeping constraints and area audits on their
geometric region. The boundary audit shares a witness only between complete
identical same-face linear edges; a shared segment cannot hide a missing hole.

The [original six-case comparison](smlib-planar-comparison.json) remains immutable
and now replays in both builds. The [expanded validation](planar-regions-validation.json)
retains full native definitions, independent overlap/contact/plane cases,
application history, output archives and normal-mesh checks. These checks do not
establish arbitrary self-intersection handling or complete native parameter and
performance parity.
