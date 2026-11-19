# EvaluatePt

[Command index](README.md)

`EvaluatePt 13,24,35` reports a location's world and construction-plane Cartesian
coordinates in model units. Command-API coordinates are world coordinates; the
application supplies the active viewport's CPlane as query context. A direct API
call without context uses World XY.

Enter `EvaluatePt` to pick or type a point using normal drafting input, including
snaps and world/CPlane/relative coordinate modes. The report uses the plane of
the viewport where the point is accepted. Esc cancels. With `Label=Off`, the query
preserves the document and its undo/redo history. A failed evaluation keeps its
point prompt and previous point anchor available for correction.

For a CPlane with origin `(10,20,30)`, X along world Y and Y along world Z:

```text
EvaluatePt 13,24,35
World coordinates = 13,24,35
CPlane coordinates = 4,5,3
```

The command report retains binary64 round-trip precision, uses scientific notation
at extreme scales, and canonicalizes signed zero. Non-finite input and truly
unrepresentable local coordinates produce errors rather than misleading values.

`Label=On Style=Dot` previews a text dot at the current location and adds it to
the model when the point is accepted. Choose `CoordinateSystem=CPlane` for local
coordinates. Labels default to world coordinates and follow the active
annotation style for length units, scale, rounding, and precision. The report
always includes both coordinate systems. Options can precede or follow the
inline point, or be changed at the interactive prompt.

With `Style=Leader`, pick or type the measured point, then place the tail.
The preview shows the arrow and coordinate label on the construction plane
through the measured point. Inline commands can include extra path vertices
after the measured point. Saving to 3DM preserves the annotation and active style.
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/evaluatept.htm)

A [live Rhino 8.32.26160.13001 capture](../evaluate-point-rhino-reference.json)
checks World XY, a rotated/translated CPlane, and a signed-axis CPlane. It also
confirms `Label=Off`. Extreme numeric ranges and native annotation formatting
remain unverified by the probe.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/evaluate-point-command.json --timeout 240
```
