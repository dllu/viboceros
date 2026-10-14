# Native exact BlendSrf construction

Linux `native-smlib` builds use usd-brep's checked knot edits to clamp BlendSrf
parents, its exact degree elevation to align boundary degrees, and its knot
refinement to align boundary bases. Rectangular parent restrictions also use
the existing native trim adapter. Command construction and the interactive
preview both call the same readonly preparation function. Exact mixed endpoint
interpolation also uses the native kernel: `N_BasisDerivs` supplies the
requested derivative rows, and the existing homogeneous matrix solver shares
one factorization across the entire control net.

The shared core retains edge orientation, normalized parameter domains,
independent G0/G1/G2 targets, positive bulges, exact homogeneous boundary jets,
work budgets and diagnostic acceptance. The endpoint constraints are ordered
to match control-row order, and derivative basis coefficients account for the
native binary parameter chart. Four-channel solving preserves tiny coordinate
channels independently of the rational weight. Native failures
propagate before document edits. The already-qualified native editing adapters
handle complete bases and weight conditioning; no new basis evaluator,
degree-elevation algorithm or knot-blending routine is added here. Preparations
outside the pinned degree-17 limit retain portable parent editing; the blend's
own degree is always 1–5 and its endpoint interpolation remains native.

The upstream topology blend producer exists: `SmBrep::CreateEdgeEdgeBlend`
accepts one blend flag for the operation, while `SmDerivSurfDefinition` supplies
one tangency type and one curvature type. `SmApiCreateBlendPrimitive` exposes
fewer controls and forwards a null derivative definition to the G2 default.
These source-verified interfaces are not a direct replacement for independently
chosen boundary constraints and the current exact Hermite representation.
Whole-producer correspondence, sampling policy and shape parity remain further
qualification work. This is a producer-model difference, not evidence of an
upstream numerical defect. Sources:
[SmBrep.h](../third_party/usd-brep/source/SMLib/inc/SmBrep.h),
[SmSurface.h](../third_party/usd-brep/source/SMLib/inc/SmSurface.h), and
[SmApiPrimitives.cpp](../third_party/usd-brep/source/SM_API/src/SmApiPrimitives.cpp).

Separate [direct producer qualification](smlib-surface-blend-producer.md) found
and repaired unequal-domain heap corruption and ignored explicit reverse
correspondence. Those repairs are applied to the native build; the shared
command still retains its exact independent boundary construction.
The [curved rational qualification](smlib-blend-derivative-scaling.md) also
repairs higher-derivative scaling while retaining the native skinning residuals
as diagnostics.

Native regressions compare complete control nets and 13×13 grids for 432
combinations of all natural edge orientations, all nine continuity pairs,
automatic/explicit correspondence, unequal
rational degrees, knot multiplicities and parameter domains. Independent
monomial and linear-precision fixtures check every continuity pair at 1, 513
and 4,096 edge columns, with coordinate scales of zero, `1e-280`, one and
`1e280`. Each homogeneous channel has its own relative `1e-12` check.
Portable delegation tests reject malformed controls, invalid weights and
boundary mismatches while retaining both sources. Independent
linear precision checks unclamped parent limits and rectangular trimming on
`S(u,v)=(u,v,uv/5)`, including a trim station `1e-9` from the boundary. Common
signed gauges from `1e-280` to `1e280` and degree-18 fallback are checked.
The native-prepared supplied ASTRA collar pair matches the independent
transition within `1e-12` on a 41×41 grid, including scaled second-derivative
checks. This retained model fixture is separate from a fresh Rhino capture.
Command and GUI regressions cover preview cancellation, source identities,
trimming, joining, undo/redo and 3DM export.

```sh
cargo test --release -p viboceros-smlib --features native --test surface_blend
cargo test --release -p viboceros-smlib --features native --lib native_blend_hermite_rows
cargo test --release -p viboceros-geometry surface_blend
cargo test --release -p viboceros-command --features native-smlib blend_surface
cargo test --release -p viboceros --features native-smlib blend_surface
cargo run --release -p viboceros-smlib --features native --example blend_surface
```

The example checks full control-net agreement and reports complete construction
time for 3, 64 and 512 edge columns. It includes parent preparation and boundary
diagnostics and compares native and portable paths on the same process.
The [retained AArch64 measurements](smlib-blend-hermite-performance.json)
include the initial unpinned run and three runs pinned to one CPU. For 64/512
columns the native path costs about 3–6% more than the portable path; tiny
three-column fixtures add roughly 10–15 microseconds per construction in the
pinned runs. Complete
control nets agree within `3e-15` in position and `5e-16` in weight. These local
measurements do not establish performance relative to Rhino.

Fresh Rhino capture remains pending the Wine licensing startup. Native
construction preserves the application's independent boundary policy; it does not
claim reproduction of Rhino's default blend shape, arbitrary conditioning or
kernel performance relative to Rhino. Existing command scope and shape limits
are documented in [BlendSrf](commands/blend-srf.md).
