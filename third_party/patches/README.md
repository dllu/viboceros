# Local usd-brep fixes

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
