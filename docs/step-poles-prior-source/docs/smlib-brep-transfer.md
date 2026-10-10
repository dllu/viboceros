# Native SMLib B-rep export

The optional [Rust SMLib bridge](smlib-bridge.md) now exposes
`Solid::to_brep(tolerance)`. It returns owned, validated Viboceros `Brep` geometry
for exact NURBS-derived native surfaces and edges. It copies the solid before
conversion, preserves shared vertex/edge identity, face flips, outer/inner loops,
edge domains and native vertex/edge tolerances, and releases the native snapshot
after constructing Rust geometry. No kernel pointers enter the document model.

## Conversion and poles

The C boundary snapshots typed arrays of topology and NURBS data under the
existing kernel lock. Control nets are transposed from SMLib's U-outer enumeration
to Viboceros's U-fast order. The exporter rejects surface/edge classes requiring
approximation rather than converting those silently. Rust reconstructs the exact
NURBS definitions, clips edges to their native intervals and classifies mated,
seam and isoparametric trims before normal B-rep validation.

Planar trims use the Rust kernel's exact pullback when available. This avoids
fitted native UV projection curves and retains the spatial edge's degree, weights
and parameterization. Curved faces use the native trim representation after
conversion. Missing UV connections at a sphere pole become explicit singular
trims only if they lie along a natural surface boundary and the sign-coherent
isocurve control hull lies within the corresponding native vertex tolerance.
Other UV gaps are rejected. The shape and validation tolerances are retained.
The use of a straight UV connector at a collapsed surface boundary is also
described in [Open CASCADE's shape-healing documentation](https://github.com/Open-Cascade-SAS/OCCT/wiki/shape_healing).

## Generate editable CAD artifacts

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run -p viboceros-smlib --features native \
  --example export_breps --release -- /tmp/viboceros-smlib-breps
```

The example generates boxes, a cylinder, a sphere, curved through-hole/pocket
and cavity results, three overlapping-box Booleans, and a plate with sixteen
holes. It writes 3DM files, supported STEP files, a JSON reference and a request
for the existing Rhino interchange probe. Models retain exact surface/edge
definitions rather than display meshes.

The private-Xvfb Rhino capture verifies that all ten exported 3DM B-reps are
valid, then records topology, definitions, edge samples, surface grids and
lifted trim samples. [Comparison](smlib-brep-comparison.json) and
[provenance](smlib-brep-provenance.json) retain the qualified fields and limits.
Local regressions check analytic oriented volumes, source preservation, shared
topology and 3DM round trips. Floating-point homogeneous coordinate conversion
is compared within 1e-12 rather than requiring identical projected bits.

## STEP seam selection

The STEP reader now chooses alternative seam p-curves by cyclic UV continuity.
Their array order no longer determines which chart boundary a use gets. The
bounded search retains predecessor paths, considers alternate loop starts and
rejects disconnected or excessive branches. Regression cases reverse candidate
order and rotate the loop start; native cylinder/through-hole round trips exercise
the complete writer/reader path.

STEP singular pole trims remain unsupported by the existing writer. Sphere,
spherical-pocket and enclosed spherical-cavity outputs therefore remain 3DM-only
at this checkpoint. Their rejection is explicit and tested; their exact topology
is retained in Rust and 3DM. Other imported periodic topology, vertex-only loops,
non-manifold boundaries, native classes requiring approximation, scale extremes,
and Rust-to-SMLib B-rep import remain further work. The application does not yet
route its commands through SMLib.
