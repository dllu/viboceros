# Public fitting tolerance diagnostics

[Fitting guides](geometry/surface-fitting.md) · [Sampled witnesses](geometry/surface-deviation-samples.md)

The public C++ [RhinoFitSurface API](https://developer.rhino3d.com/api/cpp/group___rhino.html)
exposes an achieved-tolerance output absent from RhinoCommon `Surface.Fit`.
The `surface_fit_achieved` probe calls that documented function and the managed
method on owned surfaces, recording both outputs and unchanged source snapshots.
It uses documented [native geometry interop](https://developer.rhino3d.com/api/rhinocommon/type/rhino.runtime.interop/)
for pointer ownership. No proprietary implementation is inspected.

The oracle bridge is specific to the installed Win64 Rhino SDK. Its exported
name is generated from the public function declaration with Clang's MSVC target;
the test derives the name independently from that declaration. Calls run inside
private Xvfb sessions through `tools/rhino_oracle/run_headless.sh`.

## Fifteen observations

All 15 C++ results equal their managed counterparts byte-for-byte: 14 surfaces
and one null. Each successful output reproduces within the complete `1e-9`
parameter bound when its **captured basis** is supplied to the independent
polynomial interpolator, including a 3DM round-trip. This does not qualify
automatic knot selection. Source definitions remain unchanged.

Byte-identical source/output pairs can have different reported tolerances:

| Requested tolerance | Output controls | Native reported tolerance |
| --- | --- | --- |
| 0.001 | 7×2 | 0.0008471617 |
| 0.01 | 7×2 | 0.0014096687 |
| 0.03 | 4×2 | 0.0024230042 |
| 0.05 | 4×2 | 0.0045062040 |

The null case reports zero; that value is not an accuracy certificate for a
returned surface. Reported native tolerances, complete source parameter bounds,
and sampled directional witnesses are recorded separately.

A naive sampled-distance cutoff also chooses different bases. For the strip
requested at `0.01`, an initial 4×2 patch has sampled directional witnesses below
`0.01`, while Rhino produces 7×2. Further investigation of native error estimation
is required. Existing fitters and their complete certification gates remain
unchanged; physical `FitSrf` is still unregistered.

The [validation record](surface-fit-achieved-validation.json) retains the
successful capture, one failed import attempt with its exact producer closure,
ABI declaration, source snapshots and portable/native replay logs.
