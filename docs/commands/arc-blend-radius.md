# ArcBlend radius differences

[ArcBlend](arc-blend.md) · [Adjustment handles](arc-blend-handles.md)

For parallel forward tangents, `RadiusDifference` followed by a number locks
the first radius minus the second. Negative values make the first smaller.
The scripting form is `RadiusDifference=number`. Moving a contact recomputes
the ratio to retain the target. The result remains two exact circular arcs.

A positive ratio releases the lock. With the ratio handle selected, zero or
snapping to either end of the chord restores the last requested target.
That target starts at zero and is recalled within the active command;
settings between separate commands are not remembered. Esc cancels a numeric
prompt or handle preview; Enter commits the complete blend in one undo step.

The radius solver bounds the control ratio to `[0.01,100]`, as observed in
Rhino. An unreachable target produces the bounded blend and an
“Unable to achieve requested radius difference” warning. Explicit positive
ratios retain their wider range. Nonparallel tangents, straight chords,
backward-facing chords and alternate-branch radius solving are rejected
without changing the accepted preview or document.

The geometry API is `try_parallel_arc_blend_radius_ratio`. For a chord of
length `L` with unit tangent `t`, the radius sum is
`R=L²/(2*|t cross chord|)`. At chord fraction `a`, the radii are `a*R` and
`(1-a)*R`; solving their signed difference gives the control ratio `a/(1-a)`.
No native output coordinates seed the construction.

[Validation](../arc-blend-radius-validation.json) retains 41 new Rhino
observations: 18 matching workflows and 23 explicit diagnostics. Both builds
check coefficients, domains, attributes, selection, Undo/Redo and 36 total
command/UI 3DM round trips. Independent public ArcCurve and PolyCurve
measurements provide 1,782 point/tangent stations, plus arc radii. Cases
include translation into 3D, scaling, signed targets, contact edits, ratio
overrides, target recall and two unattainable targets. Reproduce:

```sh
python3 -m tools.rhino_oracle.check_arc_blend_radius
```

Rhino captures run in private Xvfb sessions; egui tests run without a window.
Twenty-one diagnostic cases use general tangent configurations: some attain
their targets, while others warn and leave changed previews. Their numerical
search behavior remains unresolved. Two further captures inherit settings
from earlier commands. None of these diagnostics counts as matching geometry.
Historical creation and handle records keep their original scope. Nonlinear
source contacts, continuous distance and comparative speed remain unqualified.
