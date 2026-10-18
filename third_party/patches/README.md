# Local usd-brep fixes

`usd-brep-curve-contact-tolerance.patch` evaluates bounded contact witnesses
after strict Newton attempts fail, accepting only spatial gaps inside the
caller's existing tolerance. See [the contact qualification](../../docs/native-curve-contact-tolerance.md).

`usd-brep-surface-fillet-domain.patch` keeps minimal surface fillets inside their
original offset domains. The accompanying header patch exposes the existing
solver extension factor, retaining its 0.5 default for existing callers. See
[the surface-fillet producer](../../docs/native-surface-fillets.md).

`usd-brep-tensor-bezier.patch` repairs full-order next-block copy bounds,
privately clamps both active axes and completes degree-zero spans in
`N_SrfDecomposeToBez`. See the [tensor qualification](../../docs/native-tensor-bezier.md).

`usd-brep-curve-exact-split.patch`, `usd-brep-surface-exact-split.patch` and
`usd-brep-exact-split-span.h` repair split classification and full-order copy
bounds in the pinned release below. The surface producer also checks and updates
preallocated output dimensions. See the [splitter qualification](../../docs/native-exact-splits.md).

`usd-brep-point-interpolation.patch` repairs interpolation at supplied parameters
in the pinned release below. It uses actual endpoint basis rows, preserves the
linear knot-station fast path, orders constraints privately and reuses the native
banded solver. See the [producer qualification](../../docs/native-point-interpolation.md).

`usd-brep-basis-evaluation.patch` and `usd-brep-curve-evaluation-basis.patch`
repair unclamped endpoint limits and exact nonzero evaluation spans in the
pinned release below. Curve derivatives share the stack-based basis derivative
routine; editing span helpers retain their own contracts. See the
[native basis qualification](../../docs/native-basis-evaluation.md).

`usd-brep-interior-point.patch` applies to NVIDIA usd-brep release 1.0.0,
commit `c9c5979f04cea8c38213004d4f04df0dcfb40d05` (Apache-2.0).
It checks the existing model-space tolerance before treating a nearest-point
projection as permission to trim an intersection curve. See the
[native lamp-well regression](../../docs/native-lamp-well.md).

The native CMake build applies the patch to a generated source copy using
`patch`; the upstream submodule remains unchanged. A failed patch stops the
build so an upstream update cannot silently omit the fix.

`usd-brep-blend-rail-parameterization.patch` applies to the same pinned release.
It corrects the affine offset when aligning unequal blend rail domains, removes
the second reversal that undid explicit end-to-start correspondence, and honors
that correspondence in the early ruled branch. The native build compiles a
generated `SmBrep.cpp` copy. See the [direct producer qualification](../../docs/smlib-surface-blend-producer.md).

`usd-brep-blend-derivative-scaling.patch` applies to the same pinned release.
It scales second and third surface-boundary derivatives by the square and cube
of the tangent's speed change, preserving the chain rule when takeoff length
changes. The native build compiles a generated `SmSurface.cpp` copy. See the
[curved rational qualification](../../docs/smlib-blend-derivative-scaling.md).

`usd-brep-extension-break-allocation.patch` applies to the same pinned release.
It allocates the complete knot/control arrays when an end span is separated
by a full-order interior break, in both parameter and distance extension.
The native build compiles a generated `NL_CrvAdv.cpp` copy. See the
[direct producer and adapter qualification](../../docs/smlib-extension.md).

`usd-brep-knot-removal-bound-error.patch` applies to the same pinned release.
`N_CrvRemoveKnot` already handles a failed basis-maximum search with the
conservative bound one; it must clear that handled failure from its return
status. No removal tolerance or geometry criterion changes. The build compiles
a generated `NL_CrvTool.cpp` copy. See [boundary extension](../../docs/smlib-boundary-extension.md).

`usd-brep-curve-fillet-sides.patch` applies to the same pinned release.
`SmCurveFilletExecutive` must use the second curve's side-selection parameter
for that curve. Reusing the first curve's parameter selects the wrong side
when their domains differ. The native build compiles a generated source copy.
The original direct producer's scaled-down failures are retained in the
historical comparison. Application curved NURBS contacts now run in a
normalized radius frame with checked contacts and native interval extraction.

`usd-brep-curve-fillet-seed.patch` and
`usd-brep-curve-fillet-seed-header.patch` add an executive overload with base
parameter guesses, a solver tolerance and unsnapped contact parameters.
Known junctions skip the global base-intersection choice; existing overloads
keep their previous search. Evaluation and arc-creation statuses propagate.
The object layout is unchanged. Generated header/source copies are compiled
with the earlier side repair; failed exact patch application stops the build.
See [native contact qualification](../../docs/smlib-native-fillet-contacts.md).
