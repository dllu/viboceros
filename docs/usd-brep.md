# USD BRep kernel integration

[Project overview](../README.md)

[Limitations and bugs found during integration](usd-brep-limitations.md)

NVIDIA's [USD BRep](https://github.com/NVIDIA-Omniverse/usd-brep) is pinned as
`third_party/usd-brep` at release 1.0.0, commit
`c9c5979f04cea8c38213004d4f04df0dcfb40d05`. The Apache-2.0 SMLib kernel offers
NURBS solid modeling, curved booleans, fillets, offsets, healing and tessellation.
The native `SM_API` is a C++ API with opaque topology pointers, C++ argument
types and overloads; an [optional Rust C ABI bridge](smlib-bridge.md) now exposes
native solids, copied-operand booleans, native mass queries, mesh output and
[validated NURBS B-rep transfer](smlib-brep-transfer.md). Application builds with
`--features native-smlib` use the kernel directly for [BooleanDifference](smlib-import.md),
and also enable curved union/intersection, split, edge fillets and STEP material
regions, plus [native surface joining](smlib-joining.md) and
[curve extrusion](smlib-extrusion.md), [revolution](smlib-revolution.md) and
[planar surfaces](smlib-planar-surfaces.md), [apex extrusion](smlib-apex.md) and
[exact curve-path extrusion](smlib-translational-sweep.md).
Broader arbitrary-input qualification remains open. The kernel can be
built without the USD scene-format layer.
Standalone [modeling curve import](smlib-curve-import.md) now uses native
active-interval clamping rather than the Rust endpoint extractor.
[Exact modeling bases](smlib-modeling-basis.md) carry those knots through native
copies and standalone surface construction.
[Numeric extrusion directions](smlib-extrusion-direction.md) also use the existing
kernel point-set classifier rather than adding a command-specific plane fitter.
[EdgeSrf](smlib-edge-surfaces.md) now uses the existing ruled and Coons producers,
including native compatibility, degree elevation and sheet topology.
[InsertKnot](smlib-knot-refinement.md) uses exact native curve/tensor refinement
and endpoint splitting with the complete original NURBS definitions.
[MakeNonPeriodic](smlib-nonperiodic.md) reuses that endpoint path for periodic
curves and tensor directions, including the existing split/reversal workarounds.
[ChangeDegree](smlib-degree-change.md) shares the same clamping adapter and
uses native exact elevation or homogeneous interpolation for all command modes.
[RemoveKnot](smlib-knot-removal.md) reuses the same homogeneous fitting producer
and shared selection policy, including exact chart preservation through clamping.
[RemoveMultiKnot](smlib-multiple-knot-removal.md) reuses that native fitter for
ordered group collapse and U-then-V tensor edits with shared crease selection.
[ConvertToBeziers and ConvertToSingleSpans](smlib-bezier.md) compose the existing
native curve decomposer for curves, directional strips and tensor patches.
[Curve segment extraction](smlib-curve-segments.md) uses the native segment
producer for intersection overlaps and surface-cut clipping.
[Directed segments and parameter splits](smlib-directed-segments.md) reuse it
with shared curve reversal, closed-seam and composite-leaf policies.
[Numeric-length and midpoint SubCrv](smlib-length-segments.md) also use the shared
extractor for anchor-relative preparation and final NURBS intervals.
[Closed-curve seams](smlib-curve-seams.md) use native knot refinement or checked
segment extraction for CrvSeam, numeric SubCrv preparation and cutting Split/Trim.
[Surface seams](smlib-surface-seams.md) use native tensor refinement and batched
directional extraction, retaining the complete unselected knot basis.
[Surface splitting and trimming](smlib-surface-segments.md) reuse that checked
extractor for isocurve Split, visible-face preparation, separate ExtendSrf output
and round Pipe caps.
[Isocurve extraction](smlib-isocurves.md) uses the native tensor producer with
checked endpoint preparation for ExtractIsocurve, trim-aware face portions,
Contour boundaries and UV rectangle sizing.
[Loft, Sweep1 and Pipe section matching](smlib-section-bases.md) reuses native
clamping, degree elevation and knot refinement with shared section-weight and
interpolation policies.
[Loft interpolation](smlib-loft-interpolation.md) now uses native basis evaluation
and one factorization for all homogeneous channels: banded for open systems,
pivoted for periodic closed systems.
[InterpCrv and curve-through interpolation](smlib-curve-interpolation.md) share
the native cubic solver, preserving point/tangent policy and using banded
storage and solving for open systems.
[Curve Rebuild](smlib-curve-rebuild.md) shares native basis and matrix solving
for all degrees 1–11, including reordered periodic bands and command previews.
[Surface Rebuild](smlib-surface-rebuild.md) reuses native station/pole isocurves
and batches the same banded tensor interpolation across all rows of each axis.
[TweenSurfaces](smlib-surface-tween.md) uses native forced-open rebuilding,
exact basis edits and sampled point-grid solving for all three matching modes.
[FitCrv](smlib-curve-fit.md) reuses native fixed-row banded interpolation while
retaining adaptive sampling, kink and endpoint-handle policy.
[Sweep1 and Pipe rail/section interpolation](smlib-sweep-interpolation.md) now
use native clamping, section-station refinement, curve fitting and batched
homogeneous section solves, including continuous-model Pipe fitting.

## Reproduce the native trial

The experiment uses Linux, CMake 3.20+, a C++17 compiler, Ninja and system oneTBB
development headers/libraries (`libtbb-dev` on Ubuntu). On the DGX Spark it ran
natively on ARM64 with GCC 13.3, CMake 3.28.3 and oneTBB 2021.11.0.

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
cmake -S tools/usd_brep -B /tmp/viboceros-usd-brep-build \
  -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/viboceros-usd-brep-build -j 10
ctest --test-dir /tmp/viboceros-usd-brep-build --output-on-failure
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_smoke
```

Skipping LFS downloads is sufficient for this primitive-based experiment.
Large upstream fixtures remain pointer files; fetch them with Git LFS before
using those fixtures. Normal Viboceros builds only require OpenNURBS.

`tools/usd_brep/CMakeLists.txt` builds unmodified SMLib and SM_API sources as
shared libraries. It preserves the upstream release graphics-callback class
layout and uses the system TBB dependency. It builds the upstream Boolean/bounds
example and our separate analytic smoke test. This is a Linux kernel-only build,
not a replacement for upstream's full build, packaging or USD/Python tests.

The standard upstream `./prebuild.sh` was also attempted. Its Packman setup
downloaded ARM64 premake, then failed because the public server did not contain
the pinned Python package `3.12.15+nv1-manylinux_2_35_aarch64`. The standalone
trial bypasses that dependency without changing the submodule. USD import/export,
Python bindings and STEP/OCCT translation were not tested.

## Results

Both CTest targets passed on October 9, 2026. The upstream example performs
sixteen cylindrical through-hole differences in each of three bounding-box
query configurations, checking analytic volume, manifold status and bounds.
Our seven fixtures separately check finite volume, tight bounds, manifold solid
status, successful tessellation and closed manifold output meshes:

| Fixture | Expected solid volume | Observed relative volume error |
| --- | ---: | ---: |
| 10 × 10 × 10 box | 1000 | 0 |
| Overlapping-box union | 1720 | 0 |
| Overlapping-box intersection | 280 | 2.1e-16 |
| Overlapping-box difference | 720 | 0 |
| Radius-2 cylindrical through-hole | 1000 − 40π | 1.6e-9 |
| Enclosed radius-2 spherical cavity | 1000 − 32π/3 | 6.2e-10 |
| Radius-2 hemispherical pocket | 1000 − 16π/3 | 1.7e-9 |

The box operands overlap over a 5 × 8 × 7 region. The through-hole and pocket
exercise curved/planar surface intersections; the cavity exercises an interior
void shell. Solid-volume tolerance is 1e-6 relative and bounds tolerance is
1e-4 absolute. Tessellation uses chord height 0.005 and both angle controls 15°;
mesh volume must agree within 0.5% relative. These checks qualify these fixtures,
not full geometric equivalence, arbitrary input robustness or Rhino parity.
The captured measurements and source hashes are in
[usd-brep-trial.json](usd-brep-trial.json).

Boolean calls in this capture took roughly 0.28–1.22 ms each on this host, excluding
creation, queries, tessellation and destruction. These single-run measurements
are diagnostic; no comparison against native Rhino performance has been made.

## Integration assessment

The successful curved cuts make this a useful candidate for the general curved
B-rep Boolean gap in Viboceros. Keep geometry conversion and kernel calls behind
a separate adapter: transfer NURBS surfaces, spatial curves, parameter curves,
edge orientations, periodic seams and model tolerances explicitly; validate
results before committing document changes. Tessellated output can serve display
and STL export, but cannot replace exact B-reps for 3DM/STEP interchange.

The API's three tested Boolean overloads modify and return operand A, deleting
operand B on success. Failure leaves both allocated and potentially modified.
The trial uses fresh operands, checks status against `SM_SUCCESS`, and follows
that ownership contract. A production adapter must operate on temporary copies
to preserve Viboceros's transactional undo and cancellation behavior.

The default context has unsynchronized state. Upstream recommends one worker
process per concurrent job rather than arbitrary concurrent calls in one process;
see [its concurrency guide](../third_party/usd-brep/.agents/operations/concurrency.md).
An initial bridge should serialize calls or use isolated workers, translate
errors at the C ABI boundary, and avoid exposing kernel pointers to document code.

Upstream's [known issues](../third_party/usd-brep/KNOWN_ISSUES.md) include healing
that can report success after failed repairs, periodic seam problems, inaccurate
default mass properties on some circular trims, and tessellation winding defects
near fillets. The trial tightens property accuracy and uses 15° tessellation
angles. Healing, fillets, imported periodic topology, tangent/coincident cases,
scale extremes and broader Rust round trips still need qualification. The
application now uses the [optional Rust bridge](smlib-bridge.md) for supported
curved Boolean commands when built with `--features native-smlib`.

## Licensing

The submodule retains [LICENSE](../third_party/usd-brep/LICENSE) and
[THIRD_PARTY_NOTICES.md](../third_party/usd-brep/THIRD_PARTY_NOTICES.md).
NVIDIA sources are Apache-2.0 except the listed MIT bootstrap scripts; system
oneTBB is Apache-2.0. Full USD/Python distributions carry additional dependency
notices. No upstream sources were copied into Viboceros or modified for this trial.
