# CurveBoolean

Select curves and enter `CurveBoolean`, or start the command and select curves
before pressing Enter. Click inside bounded regions to keep them; clicking a
region again removes it. `AllRegions` selects every bounded region. Enter accepts
the preview and Escape discards it.

```text
CurveBoolean AllRegions
CurveBoolean 1,1,0 5,3,0 CombineRegions=No Output=Surfaces DeleteInput=Used
```

Scripted points are world coordinates. `Sources=id,...` supplies explicit inputs.
Options also accept `Name Value` during interactive input:

| Option | Default | Behavior |
| --- | --- | --- |
| `CombineRegions` | `Yes` | Merge adjacent selected cells. Point contacts remain separate. |
| `Output` | `Curves` | Return closed curves or trimmed planar surfaces. |
| `SimplifyCurves` | `Yes` | Simplify curve output using the existing SimplifyCrv adapter. |
| `DeleteInput` | `None` | Keep inputs, delete `All` inputs, or remove `Used` boundary portions. |

While selecting inputs, type `SubCrv`, pick a curve or surface edge, then pick
its temporary start and end. Closed curves can cross the seam. A numeric length
uses its magnitude; move along the curve to choose the direction, or use `D`
to toggle direction locking. Zero and full closed lengths contribute no input.
Escape returns from the nested picker; another Escape cancels CurveBoolean.
Temporary parents stay unchanged under every deletion mode. Selecting the same
parent as a whole curve also makes it subject to the chosen deletion policy.

Scripts can repeat `SubCrv=id,start,end` using curve parameters, or use
`SubCrv=id,Edge,index,start,end` for a B-rep edge. `SubCrvLength=id,start,length`
uses a signed length to choose direction. Temporary references can share a parent.

Coplanar inputs retain their common plane. Noncoplanar inputs project onto the
active CPlane. Open curves can enclose regions together; dangling branches add
no area. Holes, bridges, duplicate curves, self-crossings and rational curves
use the shared planar arrangement graph.

Outputs are selected, ungrouped and use fresh attributes on the current layer.
Used input remnants have fresh identities and retain source attributes, groups
and geometry text. Closed remnants remain split at the original seam. For
rational degree-one inputs, Used trimming follows Rhino's measured affine
span-fraction policy; the arrangement itself uses exact rational parameters.
One Undo/Redo step covers the accepted result. Options persist through Undo,
document changes and cancellation within the application session.

The [validation record](../curve-boolean-validation.json) covers native command,
UI, history and output archive checks in both builds. The comparison gate is
`1e-6`. Six circle cases remain outside it: Rhino's computed intersection ends
differ from the exact circles, with a larger absolute gap in the scaled case.
Surface edge/vertex counts and trim types also differ in recorded cases.
The [temporary-input validation](../curve-boolean-subcurves-validation.json)
adds ordinary and wrapped ranges, numeric lengths, repeated parents, nested
cancellation and planar surface edges. Native curve classes, seams, parameter
charts, full B-rep topology and comparative speed remain unqualified. Sampling
checks do not prove a continuous error bound for arbitrary curves.

Input and derived storage are bounded by the planar graph's resource checks,
including 100,000 input Bezier controls, a 64 MiB storage estimate and 16 million
curve-pair comparisons.

```sh
cargo test -p viboceros-command --lib curve_boolean
cargo test -p viboceros-oracle --test curve_boolean
cargo test --bin viboceros curve_boolean
python3 -m unittest tools.rhino_oracle.test_curve_boolean
```

Repeat Rust checks with `--features native-smlib` for the native adapter.
Licensed captures use private Xvfb sessions. Reference:
[Rhino CurveBoolean](https://docs.mcneel.com/rhino/8/help/en-us/commands/curveboolean.htm).
