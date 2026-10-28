# One-rail sweeps

[Architecture](architecture.md) · [Surface commands](commands/surfaces.md) · [Oracle](oracle.md)

`Sweep1` has an independent geometry model, rail-basis construction,
and a command adapter. Rhino compatibility is still in progress;
[validation and remaining gaps](sweep1/validation.md) distinguish complete-image
proofs, sampled comparisons, and known differences.
Linux `native-smlib` builds use [native section matching](smlib-section-bases.md)
for clamping, exact degree elevation and knot refinement.
[Rail and section interpolation](smlib-sweep-interpolation.md) also reuse native
clamping, insertion, curve fitting and homogeneous banded solves. Frame transport,
blending and geometry audits retain their shared implementation.

## Command workflow

```text
Line 0,0,0 0,0,5
SelLast
SetObjectName Rail
Line 0,0,0 2,0,0
SelLast
Sweep1 RailName=Rail Parameters=0
```

`Sweep1` can place profiles automatically: select the rail first, then profiles
in traversal order, or select only profiles and specify `RailName=name` or
`RailId=id`. The viewport picker accepts the same order. See
[automatic placement](sweep1-placement.md) for direction, interval choices and
qualification limits.

Explicit `Parameters=` scripts retain their existing policy: without a named
rail or ID, the **last** selected curve is the rail. Supply one finite, strictly
increasing native rail parameter per profile. These are not normalized fractions
or distances unless that is the rail's actual domain. Profile directions and
seams are retained. A named rail must be unique, selectable, and a curve.

Parameters just outside a rail endpoint are snapped only within eight machine
epsilons of the rail's domain length, accommodating endpoint reconstruction
roundoff. Meaningful extrapolation, nonfinite values, and section parameters
that cease to be strictly increasing after snapping are rejected.

Without point ends, explicit parameters sweep from the first profile's station to the last profile's station.
A single profile instead sweeps forward from its station to the rail's end.
Inputs, attributes, groups, and selection remain unchanged. The new unselected
B-rep uses the current layer; profile creases split into shared-topology faces.
The operation is one undo step. Invalid options/construction do not modify the
document. Closed profiles are supported. A smooth closed rail can produce a
closed seam when the swept interval covers its full loop and the generated
surface closes in the rail direction. The command rejects an open seam instead
of adding a slit B-rep; a corner at the rail seam requires a miter and is
rejected. The circular rail with one profile is covered by
command and kernel tests; a circular profile produces a native one-face solid.

| Option | Implemented behavior |
| --- | --- |
| `RefitRail=No` (default) | Interpolate profiles in the existing rational rail basis when their stations are already Greville sites; otherwise refit before inserting interior stations. |
| `Rebuild=5`, `Simplify=None` / `Simplify=Rebuild` | Rebuild real sections before sweeping; see [section rebuilding](sweep1/rebuilding.md). |
| `RefitRail=Yes` | Refit the rail to cubic arc-length form, then interpolate profiles in that fixed basis. |
| `StartPoint=x,y,z`, `EndPoint=x,y,z` | Collapse the corresponding rail end to a point; see [point ends](sweep1/point-ends.md). |
| `FrameStyle=Freeform` (default) | Shared rotation-minimizing frame transport. |
| `FrameStyle=Roadlike Axis=0,0,1` | Use a fixed axis; default to the planar rail's normal, otherwise world Z. `Axis` overrides it. See [Roadlike frames](sweep1/roadlike.md). |
| `GlobalShapeBlending=No` (default) | Local cubic smoothstep between neighboring profiles, in rational rail parameters if retained, arc length if refitted. |
| `GlobalShapeBlending=Yes` | Refitted path: linear arc-length blending between neighboring profiles (three-plus-profile compatibility remains incomplete). Retained path: Local blending, as in the measured Rhino commands. |

For two profiles at native stations 0 and 5, select them in that order and use
`Sweep1 RailName=Rail Parameters=0,5 RefitRail=Yes`. The refitted model preserves
every supplied section. Retained construction has the rational profile policy
described in the [construction guide](sweep1/construction.md). Neither path
reproduces all Rhino multi-section results; see [validation](sweep1/validation.md).

## Geometry and numerical method

The [construction guide](sweep1/construction.md) describes section matching,
frame transport, rational weight policy, retained/refitted rail bases, and
continuous-model fitting. Construction is bounded by 256 profiles, 512 controls
per matched profile, 4,096 rail-fit/final U controls, and 262,144 surface controls.
Native builds reuse usd-brep/SMLib for section preparation and interpolation.

## Verification and known differences

The [validation guide](sweep1/validation.md) keeps current complete-image checks,
the certified spatial gap, historical oracle comparisons, and performance limits
together. Closed-rail holonomy, miters, general branch/seam choices, and complete
Rhino multi-profile blending remain unfinished.
