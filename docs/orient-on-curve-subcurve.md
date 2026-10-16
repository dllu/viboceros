# OrientOnCrv temporary guides

[Command usage](commands/orient-on-curve.md) · [Original placement validation](orient-on-curve-validation.json)

`OrientOnCrv` now accepts inline SubCrv curve guides and the complete-command
`SubCrv=start,end` option. Original model geometry stays intact. The controller
keeps an original guide snapshot, a private directed portion, and a cursor curve;
only accepted object placements enter its shared history group.

The command and preview reuse `curve_segments::directed`, the existing SubCrv
length/direction helpers, and the snapshot-based subcurve preview cache. With
`native-smlib`, NURBS trimming uses usd-brep through the existing bridge. A point
interval can reverse an open guide, while numeric shortening retains its source
orientation. Closed decreasing intervals wrap across the original seam.

Spatial transport begins at the private portion's start. OnCurve source frames,
Flip, cursor constraints, and typed clamping all use that same private guide.
The endpoint preview now preserves directed input for the existing UV commands
as well. Viewport curve getters report their actual 3D hover point, allowing
SubCrv direction locking to work away from the construction plane.

The 26-case portion capture covers lines, cubic spatial NURBS, circles, reversed
open ranges, closed seam crossings, numeric lengths and backward clamping,
OnCurve, Flip, perpendicular frames, and Copy. Four additional cases check typed
projection. Two copy batches distinguish a relative point measured from a
clamped endpoint from one measured from the original typed coordinate. That
experiment exposed and corrected the previous interactive point-history error.
The complete-command oracle replays all 81 whole/partial-guide cases from their
original source recipes. UI tests independently replay all 26 portion workflows
and both relative copy batches against the captured witness points and selection.
Local tests also cover one-batch undo/redo, source/group retention, directed
preview, numeric direction locking, invalid lengths, and cancellation.

The projected/relative raw records contain an unsuccessful diagnostic attempt
to read `rhinoscriptsyntax.LastPoint`; that API is unavailable in this runtime.
The helper no longer calls it. Relative-anchor behavior is established by the
observed copy positions, and no unavailable diagnostic is used as an input or
expected result. SDK frames in these records describe the original full guide,
not the temporary portion. Helper fingerprints describe the final reproducer;
the helper evolved during capture, so they do not establish byte identity of
every earlier staged helper.

[Surface-edge guides](orient-on-curve-edge.md) now have separate validation.
Using a guide among the sources, all singular and
self-intersecting cases, native reference-point rotation, native undo/redo,
and general performance remain unqualified. Native direction locking was not
captured specifically for OrientOnCrv; it reuses the previously tested inline
SubCrv policy and has local interaction coverage here. These captures establish
bounded fixture agreement rather than complete command parity.

[Validation record](orient-on-curve-subcurve-validation.json) records the executed
checks, hashes, and comparison counts. Native captures and source-only fixtures
are `tools/rhino_oracle/{observations,fixtures}/orient_on_curve_{subcurve,projected_input,relative_input}*`.
The three successful discovery rows are retained separately and are redundant
with the expanded fixture rather than additional qualification cases.

```sh
cargo test -p viboceros --bin viboceros orient_curve
cargo test -p viboceros --bin viboceros subcurve
cargo test -p viboceros-command -p viboceros-oracle --lib orient_on_curve
cargo test -p viboceros-oracle --features native-smlib --lib orient_on_curve
python3 -m unittest tools.rhino_oracle.test_orient_on_curve
```

Public reference: [McNeel's inline SubCrv documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/subcrv.htm).
