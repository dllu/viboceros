# Weight

Use `PointsOn`, select curve or surface control points, and run `Weight`.
The numeric field and slider preview the shape in every viewport. Choose OK or
press Enter to accept; Cancel or Escape preserves the model and redo history.
The slider starts at 0.1–10 and its endpoints are editable. Typed weights may
fall outside that range, and must be positive and finite.

Use `-Weight` for command-line input, or `Weight 2` to apply a value directly to
preselected controls. With no selected controls, pick controls and press Enter
before entering the weight. Enter retains a common weight; when selected
weights vary, Enter uses 1. Mesh vertices and edit points have no rational
weights and are skipped.

The command sets absolute weights while retaining Euclidean control locations,
degree and knots. Closed and periodic seams share their logical controls.
Uniform weights normalize to 1. Composite curves rebuild their shared
controllers and NURBS leaves; new first-derivative breaks at repeated knots can
split a curve into a polycurve. Object identity, attributes and groups survive.
One Undo step restores the changed owners; Redo reapplies the edit.

Validation covers 262 saved Rhino workflows through both the command API and
application input flow, plus 14 workflows reopening grip displays. Full curve
and surface definitions, composite leaves, grip states, object order, history,
and native getter/results are compared. Arbitrary trimmed-surface behavior,
large-net performance and exhaustive knot layouts remain unqualified.

```sh
cargo test -p viboceros-geometry -p viboceros-document -p viboceros-oracle -p viboceros weight
cargo build --release -p viboceros-oracle
python3 -m tools.rhino_oracle.replay_grip_weight --output /tmp/weight.json
```

[Validation and evidence](../weight-validation.json) ·
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/weight.htm)
