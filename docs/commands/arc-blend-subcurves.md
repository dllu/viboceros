# ArcBlend temporary portions

[ArcBlend](arc-blend.md) · [Handles](arc-blend-handles.md)

At either source prompt, enter `SubCrv`, pick a curve, then choose its start
and end points. The directed end becomes the blend contact, regardless of
where the parent was picked. Backward point order reverses the bounded
portion, including on closed parents. Endpoint handles stay within that
portion. The original parent remains unchanged when Trim or Join is enabled;
Join includes the retained private portion in its new output.

After choosing the start, a numeric length accepts model units, suffixes,
fractions and arithmetic. Confirm its direction with a point. Negative lengths
use their magnitude; zero discards the temporary selection. `Direction=Locked`
captures the hovered side; `Direction=Free` unlocks it. Open locked lengths
accept immediately. Free numeric portions on closed curves can cross the seam.
Typed Cancel returns to the parent source prompt. Esc also discards the nested
editor; `!` aborts the whole command. None of these previews creates history.

For scripts, these Viboceros bindings use native source parameters:

```text
ArcBlend SubCrv=id1,1,4 Curve=id2 10,3,0 Trim=Yes Join=Yes
```

`SubCrv=id,a,b` selects a bounded directed interval.
`SubCrvWrap=id,a,b` allows increasing traversal across a closed seam when
`a>b`, preserving numeric-length selection during command replay.
An edge binding is `SubCrv=id,Edge,index,a,b`. Whole and temporary references
to the same parent may be joined; a whole source consumed by Join is deleted.
Two whole references to one parent still cannot be joined. `Parameter1/2`
refer to the effective portion's domain, which may change when reversed or wrapped.

[Validation](../arc-blend-subcurves-validation.json) retains 29 native workflows,
including cancellation with pending redo, 68 acknowledged physical picks and
2,079 independent native point/tangent stations. Both builds compare geometry,
attributes, selection, undo/redo and 58 3DM round trips. Reproduce:

```sh
python3 -m tools.rhino_oracle.check_arc_blend_subcurves
```

Captures use private Xvfb; application and egui tests are windowless. Native
coverage includes lines, arcs/circles, cubic NURBS and a polyline in Top view.
Closed numeric locking, other pointer/view trajectories, B-rep edges and
arbitrary overlapping self-portions remain unqualified. Joined outputs compare
NURBS coefficients and local samples; continuous distance and speed remain
unqualified. Nested Escape and stale references have application safety tests,
without a native Escape comparison.
