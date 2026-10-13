# Native morph interpolation

Linux `native-smlib` builds now reuse usd-brep collocation and matrix solving
for Bend, Twist, Taper, Maelstrom and deformable OrientOnSrf curve/surface
fitting. The public surface-morph oracle APIs and prepared viewport cages
select the same producer. Shared B-rep fitting routes each edge and face
through its configured point morph before the existing topology audit.

`PointMorph::interpolation_solver` selects an optional
`MorphInterpolationSolver` over the existing `FixedInterpolationSystem`.
Built-in morphs expose `with_interpolation_solver`; command factories select
`viboceros_smlib::morph_interpolation`. Portable construction remains the
default for geometry callers. This keeps the geometry crate independent of the
native bridge and preserves morph-specific overrides for identity, structure
preservation and multi-face B-reps.

Native preparation computes stations and fixed-row tags without building the
portable collocation rows or dense rational matrices. Three-coordinate targets
are packed into homogeneous batches with unit padding; rational candidates
retain all four channels. Every tensor axis shares one native factorization
across every row of the other axis. Cubic systems use the existing five-wide
native band; rational composition candidates use degrees three, six and nine
with the existing general banded producer. Full-order knot groups use two
independent fixed controls at identical parameters, preserving left/right
source limits. The native homogeneous back-solver retains tiny nonzero channels.

Point maps, sampling/cache policy, adaptive refinement, source domains and knot
continuity remain shared. Rational candidates still require positive finite
weights and pass independent mapped-point checks; a failed numerical candidate
can still fall back to adaptive cubic fitting. Mapping failures propagate.
Curve/tensor reconstruction retains fixed source locations and coordinate
recentering. Backend errors retain their native message through
`GeometryError::InterpolationBackendFailed`; invalid returned layouts and
nonfinite values are rejected before returning geometry.

Preview constructors accept the optional solver so cached native cages prepare
stations alone. Cages now retain the preparation solver; a point morph can
override it with its own solver. [Native preview operations](smlib-morph-preview.md)
also supply template elevation, isocurve extraction and wire trimming.
Mouse motion then maps cached points and uses native solving;
these cages remain display approximations. Preserved structures, rigid
transforms, identity operations, mesh/point maps and candidate control nets
retain their existing direct construction paths.

[Saved-producer comparisons](smlib-morph-interpolation-comparison.json) match
all 33 records from curve/rational-curve, surface, B-rep, OrientOnSrf and same-file 3DM fixtures
at absolute `1e-9`, relative `1e-12`; maximum difference is `1.22e-12`. These are
comparisons against the pre-change portable fitting producer, not fresh Rhino
captures. Existing command tests independently replay retained Rhino geometry,
metadata, selection, groups and history at their established fitting tolerances.

Native regressions compare complete polynomial/rational nets and independently
evaluate direct mapped images, covering composition degrees through nine,
full-order sided curve limits and prepared curve/surface cages. A separate cubic
Bernstein reference verifies nonconstant preview channels down to `1e-280`.
Shared tests reject invalid backend layouts and nonfinite values without editing
sources.

Interactive InterpCrv preview now calls the exported command interpolation
producer too. This repairs the existing native-feature preview/completion
control-net difference without relaxing the exact-equality preview tests.

The optimized geometry/command/native-bridge suite passes 3,654 tests
(11 ignored). An additional tiny-channel regression passes, and all 93 viewport
preview checks pass (four ignored), including exact InterpCrv completion
agreement. Workspace lint passes with warnings denied in both feature
configurations; formatting passes.

The existing 512-curve, 256-surface-axis and eight-million cached-sample budgets
remain. Native fitting uses the pinned degree-17 kernel; mixed-degree preview
axes above this limit return a backend error. Adaptive/composition fits use only
supported degrees. Exact-span/chart guards retain the existing narrow-span
restrictions. Acceptance is sampled, not a continuous bound for an arbitrary
point map; self-intersection, arbitrary-conditioning agreement and Rhino-speed
parity remain unqualified. Fresh captures remain pending the Wine licensing
startup described in [isocurve qualification](smlib-isocurves.md). See the
[usd-brep limitations ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test morph_interpolation
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros \
  --features native-smlib preview
```
