# ExtrudeCrvToPoint SubD output

Select curves, then specify the apex:

```text
ExtrudeCrvToPoint 0,3,2 Output=SubD DeleteInput=No
```

Curved profiles use the shared surface-to-SubD fitter: Greville stations, natural
open ends, periodic closed seams, and retained crease stations. An adjacent ring
supports the apex pole. Quadratic and higher noncubic linear profiles retain
their stations near a pole; natural cubic pieces can omit endpoint handles.
The fitted profile can differ from the input. The default `Output=Surface`
retains the [exact apex extrusion](surfaces.md).

Matching Rhino's public factory, degree-one profiles produce an exact uncapped
B-rep instead. Global `CreaseSplitting` controls whether its kinks become shared
edges, using the existing 0.1-degree G1 policy. It does not change SubD output.
B-rep fallback retains native edge intervals and singular pole trims. Linear UV
crossings use direct root proposals followed by the same exact partition checks.

`Solid` applies to Surface output. Switching to SubD leaves the base open, even
when Solid was enabled. This also applies to the degree-one B-rep fallback.

Start without an apex to pick it in the viewport. `Output=Surface|SubD`,
`DeleteInput`, and the stored Solid option remain editable until the pick.
Invalid option groups leave the getter unchanged; Escape preserves the model
and pending Redo. Options reset for each invocation. Outputs use fresh attributes
on the current layer, remain unselected and ungrouped, and share one undo step
with input deletion. Shared conversion and face-partition budgets apply.

The [validation record](../extrude-apex-subd-validation.json) covers 60 native
command workflows and 11 public SDK surface/factory comparisons in both builds,
with application replay and 3DM round trips. Fourteen fallback workflows also
compare complete B-rep definitions, trims, tolerances and oriented incidence.
The absolute epsilon is `1e-12`. Component numbering and SubD face insertion
order are not reproduced. Continuous approximation bounds and comparative
performance remain unqualified.

```sh
cargo test -p viboceros-oracle --test extrude_apex_subd --test extrude_apex_subd_archive
cargo test -p viboceros extrude_apex_subd
python3 -m unittest tools.rhino_oracle.test_extrude_apex_subd
```

Native captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`.
Reference: [Rhino ExtrudeCrvToPoint](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvtopoint.htm).
