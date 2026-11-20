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
in traversal order, or select only profiles and specify `RailName=name`,
`RailId=id`, or `RailEdge=id,index[,face]` with zero-based component indices.
Pick a curve or surface edge in the viewport, or use `ChainEdges` to assemble a
rail from connected curves and edges. See
[automatic placement](sweep1-placement.md) for direction, interval choices and
qualification limits.

Press Enter after selecting open profiles to review the live draft. Bare
`Sweep1` with an open rail and profiles already selected opens review immediately.
Adjust options such as `Style`, `ShapeBlending`, `RefitRail`, and `Simplify`,
then press Enter to create the sweep.

For closed profiles, Enter opens the seam editor. Drag the markers, or use
`SelID id` followed by a point or `Parameter=value`. `Flip` reverses a chosen
profile, `Automatic` aligns successive seams, and `Natural` restores their
original locations and directions. Turn `SnapToKnots` off to move freely.
The sweep preview follows the seam edits. Accept them to review the draft and
press Enter again to create it. A complete scripted sweep uses its supplied
profile seams. On a straight rail, two edited closed profiles that differ in
their rail frames use a cubic rail basis for the blend.

With two or more profiles on a closed rail, review offers `Closed`. `Closed=No`
stops at the last profile. `Closed=Yes` continues around the rail and blends
back to the first. The first profile sets the seam, two profiles follow the
rail's authored direction, and additional profiles determine traversal order.
One automatically placed profile on a closed rail sweeps the full circuit
starting at its contact. Point ends require an open sweep.

For scripts using `Parameters=` without an explicit rail reference, the **last**
selected curve is the rail. Supply one finite, strictly
increasing native rail parameter per profile. These are not normalized fractions
or distances unless that is the rail's actual domain. Profile directions and
seams are retained. A named rail must be unique, selectable, and a curve.

Parameters just outside a rail endpoint are snapped only within eight machine
epsilons of the rail's domain length, accommodating endpoint reconstruction
roundoff. Meaningful extrapolation, nonfinite values, and section parameters
that cease to be strictly increasing after snapping are rejected.

Without point ends, explicit parameters with `Closed=No` sweep from the first
profile's station to the last profile's station. A single explicitly placed
profile sweeps forward from its station to the rail's end.
Inputs, attributes, groups, and selection remain unchanged. The new unselected
B-rep uses the current layer; profile creases split into shared-topology faces.
The operation is one undo step. Invalid options/construction do not modify the
document. Closed profiles are supported. A smooth closed rail can produce a
closed seam when the swept interval covers its full loop and the generated
surface closes in the rail direction. For a straight-segment rail with one
profile, trimmed miters sew a closed rail seam. Untrimmed miters keep the two
end sections at a polygonal seam, as in the native command. The circular rail
with one profile is covered by command and kernel tests. A circular profile
produces a native one-face solid.

| Option | Implemented behavior |
| --- | --- |
| `RefitRail=No` (default) | Interpolate profiles in the existing rational rail basis when their stations are already Greville sites; otherwise refit before inserting interior stations. |
| `Rebuild=5`, `Simplify=None` / `Simplify=Rebuild` | Rebuild real sections before sweeping; see [section rebuilding](sweep1/rebuilding.md). |
| `RefitRail=Yes` | Refit the rail to cubic arc-length form, then interpolate profiles in that fixed basis. |
| `UntrimmedMiters=Yes\|No` | Choose natural or trimmed faces at straight-segment corners with one profile. Trimmed miters split extension faces at the original rail stations and split shared miter edges at profile knots. The review updates the wire preview. |
| `Closed=Yes` | Sweep the full circuit of a closed rail. Open profiles at rail knots retain the rational basis, with faces split at fully multiple knots. Closed profiles, explicit refitting, and profiles between knots use a periodic cubic arc-length surface. |
| `StartPoint=x,y,z`, `EndPoint=x,y,z` | Collapse the corresponding rail end to a point; see [point ends](sweep1/point-ends.md). |
| `FrameStyle=Freeform` (default) | Shared rotation-minimizing frame transport. |
| `FrameStyle=Roadlike Axis=0,0,1` | Use a fixed axis; default to the planar rail's normal, otherwise world Z. `Axis` overrides it. See [Roadlike frames](sweep1/roadlike.md). |
| `Style=AlignWithSurface` | Orient frames using the supporting face's normal along a single edge and use the cubic arc-length rail basis. Specify an adjacent face in `RailEdge` for shared edges. |
| `ShapeBlending=Local` (default) | Local cubic smoothstep between neighboring profiles, in rational rail parameters if retained, arc length if refitted. |
| `ShapeBlending=Global` | Refitted open sweeps blend linearly in arc length between neighboring profiles. Retained bases and closed sweeps use Local blending, as in the measured Rhino commands. `GlobalShapeBlending=Yes|No` also works. |

For two profiles at native stations 0 and 5, select them in that order and use
`Sweep1 RailName=Rail Parameters=0,5 RefitRail=Yes`. The refitted model preserves
every supplied section. Retained construction has the rational profile policy
described in the [construction guide](sweep1/construction.md). Neither path
reproduces all Rhino multi-section results; see [validation](sweep1/validation.md).

## Geometry and numerical method

The [construction guide](sweep1/construction.md) describes section matching,
frame transport, rational weight policy, retained/refitted rail bases, and
continuous-model fitting. [Region correspondence](sweep1/regions.md) covers
profiles with unequal corner counts. Construction is bounded by 256 profiles, 512 controls
per matched profile, 4,096 rail-fit/final U controls, and 262,144 surface controls.
Native builds reuse usd-brep/SMLib for section preparation and interpolation.

## Verification and known differences

The [validation guide](sweep1/validation.md) keeps current complete-image checks,
the certified spatial gap, historical oracle comparisons, and performance limits
together. Closed-rail holonomy, miters on curved or multi-profile sweeps,
general branch/seam choices, and complete Rhino multi-profile blending remain
unfinished. If an extension cut cannot be resolved at model tolerance, the
sweep retains its valid trimmed face without that partition.
