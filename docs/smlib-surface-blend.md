# Native BlendSrf preparation

Linux `native-smlib` builds use usd-brep's checked knot edits to clamp BlendSrf
parents, its exact degree elevation to align boundary degrees, and its knot
refinement to align boundary bases. Rectangular parent restrictions also use
the existing native trim adapter. Command construction and the interactive
preview both call the same readonly preparation function.

The shared core retains edge orientation, normalized parameter domains,
independent G0/G1/G2 targets, positive bulges, exact homogeneous boundary jets,
Hermite controls, work budgets and diagnostic acceptance. Native failures
propagate before document edits. The already-qualified native editing adapters
handle complete bases and weight conditioning; no new basis evaluator,
degree-elevation algorithm or knot-blending routine is added here. Preparations
outside the pinned degree-17 limit retain the portable operation.

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

Native regressions compare complete control nets and 13×13 grids for 80
combinations of all natural edge orientations, mixed continuity, unequal
rational degrees, knot multiplicities and parameter domains. Independent
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
cargo test --release -p viboceros-command --features native-smlib blend_surface
cargo test --release -p viboceros --features native-smlib blend_surface
```

Fresh Rhino capture remains pending the Wine licensing startup. Native parent
preparation preserves the application's independent construction; it does not
claim reproduction of Rhino's default blend shape, arbitrary conditioning or
kernel performance relative to Rhino. Existing command scope and shape limits
are documented in [BlendSrf](commands/blend-srf.md).
