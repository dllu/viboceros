# Divide

Select curves, then use a segment count, arc length or straight chord length:

```text
Divide 8
Divide Length 2.5 MarkEnds=Yes
Divide EqualChordLength 2.5 GroupOutput=Yes
Divide Length 2.5 Split=Yes DeleteRemainder=Yes
```

Complete argument lines also gather curve objects when no eligible source is
selected. `Div` remains an alias. Every invocation stages all results before
document mutation and forms one Undo step. Inputs remain when point objects are
created. Split replaces selected whole curves and creates pieces from temporary
portions, preserving any parent selected only through `SubCrv`.

Entering bare `Divide` gathers curves. If any are closed, drag their seam
markers or use `Flip`, `Automatic` or `Natural`, then press Enter for the
numeric options. Enter a count, or choose `Length` or `EqualChordLength` and
enter a distance. In Length mode, click a selected curve or type a point on
it to measure from the other end. These edits update viewport station markers.
Enter accepts the preview and Escape cancels it. `NumberSegments` returns to
count mode. The count and a shared Length/EqualChordLength distance are
remembered across invocations. Each invocation starts in count mode. The
initial local count and distance are 1.

At curve selection, type `SubCrv` to pick part of a curve or surface edge.
Pick the start, then pick the end or enter a length and confirm its direction.
The highlighted portion joins the Divide inputs in pick order. Repeat
`SubCrv` to add more portions, including from the same parent.
Output flags survive cancellation and Undo, and omitted flags use the remembered
values. Preferences last for the application session. See
[native preference evidence](../divide-preferences.md).
Bare boolean option names toggle their values, and `name=Yes|No` sets them.
Split previews mark piece boundaries, including any retained remainder.

Options are case-insensitive Yes/No values. Bare `MarkEnds` remains accepted.

- `MarkEnds` defaults to No. In count/arc-length point modes, open-curve ends
  are included in count mode when enabled. Length mode includes the start and
  computed stations, including the end only when a whole station lands there.
  Closed curves emit their chosen seam once. Chord mode
  includes the natural start and its computed stations; MarkEnds does not add
  an incomplete final chord.
- `Split` defaults to No. Split pieces retain source attributes and parameter
  intervals; point objects use fresh attributes on the current layer.
- `DeleteRemainder` defaults to No. In length/chord Split mode it discards the
  final incomplete segment. With Length measured backwards, Rhino omits the
  leading partial piece when interior stations exist. The pieces retain their
  original parameter direction.
- `GroupOutput` defaults to No. Point outputs form one group per input curve.
  Split outputs remain ungrouped, following the captured native behavior.

EqualChordLength chooses the first forward intersection with a sphere centered
at the previous station. Lines use direct arc-length stations; general curves
use exact dyadic Bernstein sphere coefficients and ordered root isolation,
including tangent contacts. Circular geometry has an analytic path. Polynomial
coefficient construction and output counts are bounded; singular rational traversal and invalid values fail
before output admission.

General chord traversal now keeps subdivision bounds rational, removes the
fixed-depth cutoff and checks denominator poles independently of arc length.
A bounded Sturm check distinguishes real contact from nearly touching complex
roots when the bracket reaches binary64 parameter resolution. See
[curved and extreme qualification](../divide-curved.md) for native captures,
regressions and the remaining Split differences.

The [native fixture](../../tools/rhino_oracle/fixtures/divide_command.json) covers
30 line, circle and polyline recipes, including source metadata, groups, split
domains and remainder deletion. Detailed [qualification](../divide-command.md)
records the result and limits. See [interactive input and preview](../divide-input.md)
for headless UI coverage. Preferences currently last for the application session.
