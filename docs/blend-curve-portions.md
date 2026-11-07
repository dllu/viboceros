# BlendCrv temporary sources

[Command usage](commands/blend-curve.md) · [Validation record](blend-curve-portions-validation.json)

`SubCrv` supplies an open, directed portion of a curve without changing its
parent. It supports closed parents, reversed point order, numeric lengths,
locked direction and closed-seam wrapping. A portion blends from its last
directed endpoint. PickPoint keeps that contact and initially points its tangent
toward the portion start.

Only whole inputs can replace or consume document objects. Join can include
private portions without consuming their parents. If a whole input shares that
parent, the whole input still participates normally. The output inherits the
first participating whole source's identity, attributes and groups; if both
inputs are private, it is a new current-layer object.

The shared ArcBlend/BlendCrv editor caches previews outside document history.
Nested cancellation returns to source selection; stale geometry or tolerance
clears the sources. Explicit `SubCrv[Wrap]=id,start,end` bindings use native source
parameters, with `Wrap` for directed intervals crossing a closed seam.

Qualification retains 36 complete Rhino 8 workflows across three frozen captures:
35 acceptances and one cancellation with pending redo. The cases cover lines,
arcs, circles, cubic curves, spatial rational and quintic sources, G2/G3/G4
portion contacts, point endpoints, bounded editing, shared parents and 12 joins.
Independent Native evaluations provide 1,155 point/tangent stations and 129
fourth-order derivative stations. Rhino reports `Nothing` and success=false for
accepted blends; the actual document changes and undo/redo are checked.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_portions
python3 -m tools.rhino_oracle.report_blend_curve_portions --implementation-revision <full-sha>
```

Both portable and usd-brep builds replay every workflow through the command API
and windowless editor, checking geometry, identities, attributes, selection,
undo/redo and 3DM round trips. Geometry uses a `1e-6` gate; normalized derivatives
use `1e-6` relative error and approximate native length domains use `2e-6`.
The checks include the earlier blend qualifications and shared ArcBlend editor.
Two producer setup failures are retained separately and contribute no observations.
All Native captures use private Xvfb; application tests open no desktop windows.

Later qualifications cover [whole closed sources](blend-curve-closed.md),
[zero-side joins](blend-curve-empty.md), [Shift editing](blend-curve-symmetry.md)
and [Alt editing](blend-curve-alt.md). Native surface-edge portions, arbitrary
shared-parent self-blends, curvature display and history re-editing remain
unqualified. G4 retains the measured Native zero-fourth-
derivative policy, rather than establishing general mathematical G4 continuity.
Exact Native picking, numeric/Escape trajectories, continuous distance and
comparative performance also remain unqualified.
