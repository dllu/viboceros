# EvaluatePt

[Command index](README.md)

`EvaluatePt 13,24,35` reports a location's world and construction-plane Cartesian
coordinates in model units. Command-API coordinates are world coordinates; the
application supplies the active viewport's CPlane as query context. A direct API
call without context uses World XY.

Enter `EvaluatePt` to pick or type a point using normal drafting input, including
snaps and world/CPlane/relative coordinate modes. The report uses the plane of
the viewport where the point is accepted. Esc cancels. The query creates no
geometry, changes no selection, and preserves model undo/redo. A failed evaluation
keeps its point prompt and previous point anchor available for correction.

For a CPlane with origin `(10,20,30)`, X along world Y and Y along world Z:

```text
EvaluatePt 13,24,35
World coordinates = 13,24,35
CPlane coordinates = 4,5,3
```

Numeric output retains binary64 round-trip precision, uses scientific notation
at extreme scales, and canonicalizes signed zero. Non-finite input and truly
unrepresentable local coordinates produce errors rather than misleading values.

`Label=Off` is accepted before or after the point; `EvaluatePt Label=Off` also
starts the interactive prompt. [Rhino's command](https://docs.mcneel.com/rhino/8/help/en-us/commands/evaluatept.htm)
can additionally create annotation dots or leaders. `Label=On`, `Style`, and
label `CoordinateSystem` are not implemented and are rejected explicitly.

Native tests cover translated/rotated frames, selected-object and redo retention,
subnormal/extreme coordinates, unsupported options, typed picks, cancellation,
and failed-pick recovery. A [live Rhino 8.32.26160.13001 capture](../evaluate-point-rhino-reference.json)
checks World XY, a rotated/translated CPlane, and a signed-axis CPlane. Native
coordinate reports agree with the captured three-decimal values. These probes
do not establish parity for extreme numeric ranges or annotation labels.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/evaluate-point-command.json --timeout 240
```

The live probe also established that the option is `Label=Off`, not `Label=No`;
the latter is rejected. The oracle disables labels, restores the previous CPlane,
and requires both coordinate reports after its unique history marker. Unknown
command warnings invalidate the capture even if a later point produces a report.
Zero elapsed times in the fixture are not performance measurements.
