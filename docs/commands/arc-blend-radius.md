# ArcBlend radius differences

[ArcBlend](arc-blend.md) · [Adjustment handles](arc-blend-handles.md)

`RadiusDifference` followed by a number targets the first radius minus the
second. Negative targets request a smaller first radius. The scripting form is
`RadiusDifference=number`. Moving a contact solves again for that target.
Planar and spatial tangents, backward directions and AlternateSolution are
supported. The result remains two exact circular arcs.

Numeric entry accepts fractions, calculator expressions and length suffixes in
model units. A final suffix applies to the whole expression: `2+3cm` means
50 mm in a millimeter model. Parenthesized arithmetic works; embedded named-unit expressions
such as `2*(.25cm)` are rejected. Ratio entry uses the same numeric syntax,
including unit conversion. See [input validation](arc-blend-contacts.md).

A positive ratio releases the target. With the ratio handle selected, zero
or snapping to a chord end restores the last requested target, initially
zero. The target, Trim and Join are remembered across commands, including
cancellation. Each new command starts with ratio 1 and the primary solution;
zero or the radius prompt default restores the target. Esc cancels a numeric
prompt or handle preview.
Enter commits the complete blend in one undo step. Typed `Cancel` inside
the radius number prompt accepts its default and returns to the parent getter;
a second `Cancel` at the main prompt ends the command. `!` cancels all levels.

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
keeps its original scope. Curved contacts have [separate coverage](arc-blend-contacts.md). Larger scaling,
comparative speed and continuous distance outside the same-circle cases
remain unqualified.

[Getter validation](../arc-blend-getters-validation.json) adds nine native
workflows, including nested defaults and retained previews. Run
`python3 -m tools.rhino_oracle.check_arc_blend_getters` for those checks and
the earlier regressions. Fourteen partition investigations are retained
separately; exact same-circle split and handle parity remain unqualified.

[Settings validation](../arc-blend-preferences-validation.json) checks 25
ordered native workflows, including seven cancellations, hidden Join recall
and pending redo. One timed-out numeric cancellation remains a diagnostic.
Run `python3 -m tools.rhino_oracle.check_arc_blend_preferences` for both builds
and shared regressions. Preferences belong to the command registry and are
shared across documents; they are outside undo and 3DM storage, and reset
when the application restarts.
