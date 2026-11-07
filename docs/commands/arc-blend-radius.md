# ArcBlend radius differences

[ArcBlend](arc-blend.md) · [Adjustment handles](arc-blend-handles.md)

`RadiusDifference` followed by a number targets the first radius minus the
second. Negative targets request a smaller first radius. The scripting form is
`RadiusDifference=number`. Moving a contact solves again for that target.
Planar and spatial tangents, backward directions and AlternateSolution are
supported. The result remains two exact circular arcs.

A positive ratio releases the target. With the ratio handle selected, zero
or snapping to a chord end restores the last requested target, initially
zero. Targets are recalled within the active command; settings between
commands are not remembered. Esc cancels a numeric prompt or handle preview.
Enter commits the complete blend in one undo step.

Inside ratio editing, `RadiusDifference` opens a numeric prompt and returns
to ratio editing. Enter accepts the displayed default; zero applies that
target. Enter at the ratio prompt keeps the current preview. Changing an
already active target can therefore leave a preview that misses the new
target: the warning appears on return, and final acceptance preserves that
curve. The next geometry adjustment solves for the new target.

The search bounds the control ratio to `[0.01,100]`. At each step it compares
the errors at the interval ends, retains the closer half and keeps the best
sampled ratio. A target missed by this search produces a provisional blend
and an “Unable to achieve requested radius difference” warning. This
reproduces Rhino's observed behavior, including cases where another ratio
could reach the target. Explicit positive ratios retain their wider range.
Straight or zero-length arc pieces are rejected without changing the preview.

The geometry API is `try_arc_blend_radius_ratio`. It shares the tangent-length
equation with construction and evaluates trial radii without allocating
curves or converting NURBS. The older `try_parallel_arc_blend_radius_ratio`
API remains available for closed-form parallel-tangent solutions.

[General-search validation](../arc-blend-radius-general-validation.json)
retains 99 new observations and 28 earlier target observations. Both builds
check 79 command/UI workflows, attributes, selection, history and 158 total
3DM round trips per build. Forty-eight independent public SDK curves check
trial radii, coefficients and native point/tangent samples. Reproduce:

```sh
python3 -m tools.rhino_oracle.check_arc_blend_radius_general
```

Seventy-seven workflows match NURBS representations within `1e-6`. Two
same-circle targets choose different internal arc partitions: complete-curve
samples and supporting-circle/sweep bounds verify equal geometry, while
coefficient differences remain explicit diagnostics, and ratio-handle station
parity is unqualified for those two cases.
Captures use private Xvfb; application and egui tests are windowless.
Historical [parallel qualification](../arc-blend-radius-validation.json)
keeps its original scope. Nonlinear source contacts, larger scaling,
comparative speed and continuous distance outside the same-circle cases
remain unqualified.

[Getter validation](../arc-blend-getters-validation.json) adds nine native
workflows, including nested defaults and retained previews. Run
`python3 -m tools.rhino_oracle.check_arc_blend_getters` for those checks and
the earlier regressions. Fourteen partition investigations are retained
separately; exact same-circle split and handle parity remain unqualified.
