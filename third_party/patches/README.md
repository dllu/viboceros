# Local usd-brep fixes

`usd-brep-interior-point.patch` applies to NVIDIA usd-brep release 1.0.0,
commit `c9c5979f04cea8c38213004d4f04df0dcfb40d05` (Apache-2.0).
It checks the existing model-space tolerance before treating a nearest-point
projection as permission to trim an intersection curve. See the
[native lamp-well regression](../../docs/native-lamp-well.md).

The native CMake build applies the patch to a generated source copy using
`patch`; the upstream submodule remains unchanged. A failed patch stops the
build so an upstream update cannot silently omit the fix.
