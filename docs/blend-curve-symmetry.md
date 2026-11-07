# BlendCrv Shift editing

[Command usage](commands/blend-curve.md) · [Validation record](blend-curve-symmetry-validation.json)

Hold Shift while selecting a shape handle to copy its new length to the other
end. Both ends must have the same G1–G4 continuity. Each handle follows its own
tangent constraint. Contact edits, mixed continuities and G0 peers remain
independent. The choice ends when that adjustment finishes.

This follows measured Rhino 8 behavior. The [command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/blendcrv.htm)
describes Shift symmetry; captures additionally establish that it copies physical
length, including unequal tangents and previously unequal handles.

Four immutable captures retain 33 complete workflows and 104 physical clicks.
Public [ShiftKeyDown](https://developer.rhino3d.com/api/rhinocommon/rhino.ui.mousecallbackeventargs/shiftkeydown)
receipts confirm 32 Shift selections. Cases cover G1–G4, both handle orders,
contacts, mixed/G0 constraints, point endpoints, reversed and unequal handles,
arcs, cubic/quartic curves, spatial rational/quintic curves and three joins.
Twenty-three workflows contain a paired shape edit. Three additional length
observations repeat earlier input recipes; all 33 workflows are replayed.

Both portable and usd-brep builds compare command and windowless editor results,
source identities and attributes, selection, undo/redo, 3DM round trips, 1,089
Native point/tangent stations and 483 derivative stations. Windowless tests also
check Shift release before mouse-up, held dragging, preview cancellation, typed
unit lengths, atomic invalid edits and one history entry. Native capture input
releases Shift in a `finally` block and targets only the owned private viewport.

| Measurement | Gate |
| --- | --- |
| Geometry coefficients and point/tangent samples | `1e-6` |
| Normalized derivatives | `1e-6` relative |
| Parameter-domain coordinates | `max(2e-6, 1e-8 × Native interval span)` |
| Length against Native `GetLength(1e-12)` | `1e-7` absolute |

The extended rational holdout produces a 635-unit curve and exceeds the earlier
absolute-only domain gate. Independent public measurements show that Native
output domains equal [GetLength()](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/getlength?version=8.x),
whose documented default desired fractional precision is `1e-8`. Tighter Native
measurements agree with our length calculation. Absolute domain differences and
the scaled gate ratio are retained; output lengths never seed construction.
The earlier qualifications keep their existing gates.

```sh
python3 -m tools.rhino_oracle.check_blend_curve_symmetry
python3 -m tools.rhino_oracle.report_blend_curve_symmetry --implementation-revision <full-sha>
```

All captures run in private Xvfb; application tests open no desktop windows.
Native held dragging and numeric/Escape trajectories, surface-edge Alt editing,
exact hit-box/depth ranking, history re-editing, continuous distance and speed
remain unqualified. G4 retains the measured zero-fourth-derivative policy; general
mathematical G4 continuity and the Native approximate length algorithm are not
established by these checks.
