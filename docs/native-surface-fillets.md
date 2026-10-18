# Native surface-to-surface fillets

`viboceros_smlib::fillet_surfaces` now constructs independently owned fillet
sheets through usd-brep's existing surface/surface rolling-ball producer.
The FilletSrf command and its picking, parent trimming, joining, attribute and
history policies remain in progress. This API is their geometry foundation.

```rust
use viboceros_smlib::{fillet_surfaces, SurfaceFilletOptions, SurfaceFilletSide};
let options = SurfaceFilletOptions {
    radius: 1.0,
    sides: [SurfaceFilletSide::Positive; 2],
    extend: false,
};
let result = fillet_surfaces(&first, &second, options, tolerance)?;
```

Each side selects the signed offset along increasing-U cross increasing-V.
`None` means the native producer returned no fillet; kernel/preparation failures are
errors. Inputs share the existing intersection adapter's local spatial frame,
weight conditioning, unit parameter charts and native active-domain clamping.
The producer imports separate private parent bodies, traces their offset
intersection and requests rational circular cross sections. It returns copied
NURBS topology in world coordinates. Sources and native tolerance defaults
remain unchanged; no native handles enter document history.

Minimal mode now disables the kernel's default half-span extrapolation. That
extrapolation had put parts of a plane/quarter-cylinder fillet off its expected
torus, even with `SM_BT_MINIMAL`. Extended mode uses beveled end caps, retaining
each rail's endpoint when parent lengths differ. A half-span search can stop
inside the longer parent; the adapter doubles its private search until both
rail/end-cap junctions reach original parent boundaries, with a 32-attempt
limit. It reuses native isocurves and existing closest-parameter queries.

Two transfer fixes support this path. Surface preparation now ignores only the
two artificial full-knot endpoints when checking the reparameterized stored
basis; valid unclamped padding no longer forces a fallback. Fillet export
requests exact UV closure before world rescaling. It welds roundoff-sized joints while keeping trim tolerances unchanged. Every
adjusted p-curve must pass the existing continuous spatial correspondence
certificate against its 3D edge. Separate parameter branches remain separate,
and a tiny UV gap with excessive physical deviation is rejected.

The six bridge regressions check four offset-side choices, constant-radius
cylinder identities, rail tangency, exact planar fillet area, beveled extents,
unequal lengths up to 500:1, rotated/transformed scales `1e-40` to `1e40`,
unclamped/distant charts, signed weight gauges, invalid/disjoint requests,
source purity and a plane/cylinder torus. Five shared regressions check native
preparation and certified UV closure. The complete bridge run passes 429 tests,
with one existing ignored test; final focused results and broader caller checks
are retained in the [validation record](native-surface-fillets-validation.json).

The direct C ABI probe checks 56 geometry/lifetime cases plus 10 rejected
requests normally and under AddressSanitizer. Source buffers and private
parents are gone before export; serialized definitions are read after deleting
the native result. Vertices and sampled edges satisfy independent cylinder or
torus equations and endpoint extents at `1e-8`. Maximum observed error is
`7.08e-9`. The controlled original-policy baseline fails 24 geometry cases:
16 extended endpoint ranges and eight curved minimal fillets. Its linkage shim
accepts the new optional argument while retaining the original 0.5 extension
policy. It is a producer comparison, not a previously published Rust API.
Leak detection is disabled; this is not a leak-free claim.

The oracle adds `surface_fillet`, using the public
[RhinoCommon CreateFilletSurface API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createfilletsurface).
Six fresh SDK recipes retain complete source/output definitions, topology,
bounds, areas and closest-point queries. Five native recipes agree at 20 sampled
closest points within `1e-8`; maximum observed coordinate difference is about
`5.1e-9`. Raw NURBS definitions differ between constructors and remain retained.
SDK area integration differences remain diagnostics, alongside independent
analytic area checks. These samples do not prove a continuous global Rhino
error bound. The curved `Extend=Yes` recipe succeeds in Rhino; the retained
native baseline fails with status 1001. Its complete failure is retained.
The subsequent [curve contact repair](native-curve-contact-tolerance.md) fixes
in-tolerance boundary proposals found during this investigation. Curved extended
fillets remain a required follow-up.

The later [circular rail correspondence repair](native-circular-fillet-rails.md)
removes a revolution shortcut that changed the rail parameterization. It keeps
the existing native loft and original parent UV curves together. This also
unblocks a separate smooth-continuation prototype; the production curved
extension policy and FilletSrf command workflow remain in progress.

The subsequent [UV pick preparation and precision repair](native-surface-fillet-picks.md)
adds `SurfaceFilletOptions::try_from_picks`. The native oracle now uses the
original UV picks when no explicit side override is supplied. Construction
reserves precision for rail fitting and loft normals while retaining the caller's
acceptance tolerance. Its validation record supersedes the earlier counts and
sampled error measurements above for the current implementation.

The [parent trim/split adapter](native-surface-fillet-parents.md) additionally
returns optional owned replacements for the two support surfaces. It reuses
the kernel's insertion and trimming operations while preserving source inputs.

The native bridge retains its degree-17 and input/resource limits, positive
conditioned weights, and normalized accuracy floor. Full-order discontinuities,
severe independent weight ratios, arbitrary conditioning, closed-contact
extension behavior and general performance remain unqualified. Upstream
`repo.sh`/`prog_test`, USD, Python-binding, OCCT and GUI harnesses were not run;
this ARM bridge builds the actual SMLib and SM_API libraries.

```sh
cargo test --release -p viboceros-smlib --features native --test surface_fillets
cargo test --release -p viboceros-oracle --features native-smlib surface_fillet
python3 -m unittest tools.rhino_oracle.test_surface_fillet_probe
```
