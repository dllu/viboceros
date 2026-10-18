# Local usd-brep fixes

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
