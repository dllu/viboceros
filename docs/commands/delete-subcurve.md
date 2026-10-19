# DeleteSubCrv

Select one curve and run `DeleteSubCrv` (alias `Break`), or start the command and
pick a curve. Pick the beginning and end of the portion to remove. Open curves
retain their original orientation regardless of pick order. Closed point input
removes the forward interval, including across the seam.

After the start pick, enter a length and then confirm its direction with a point.
The magnitude is used for negative lengths. Open lengths clamp at the chosen
endpoint. A new number replaces the pending length; zero, empty Enter and Escape
cancel without editing geometry or consuming redo history. Numeric direction
uses the observed curve-specific getter rules: closed polyline/polycurve inputs
choose a shorter route toward confirmation, while the captured smooth closed
inputs retain their natural direction. The shared `Direction=Locked|Free`
getter can constrain a side. Copy, MarkEnds and FromMidpoint options belong to
`SubCrv`; starting this command leaves those preferences intact.

Every successful deletion replaces the source identity. Retained pieces inherit
the source's layer, attributes, attribute user text and groups. An ordinary trim
also retains geometry user text. Joining portions across a closed seam creates
a fresh polycurve and clears geometry-root text. Outputs finish unselected;
Undo restores an unselected source, and Redo restores unselected results. One
successful edit makes one history step. All geometry preparation precedes any
source, selection or group edits. Stale, hidden or deleted source geometry must
be picked again.

Direct scripting accepts these forms after selecting one curve:

```text
DeleteSubCrv 2,0,0 7,0,0
Break Parameter=2,7
DeleteSubCrv Numeric=2,3,8
DeleteSubCrv Numeric=7,3,2 Locked=Backward
```

`Parameter` uses native parameters. `Numeric` supplies an anchor parameter,
length and confirmation parameter. Numeric and point input have different
closed-seam semantics; full-length inputs retain the captured native behavior,
including numerical near-full remainders on rational curves. These near-full
outputs are compared at the model tolerance rather than treated as exact
cancellation or certified empty geometry.

Both builds share the command policy. NURBS trimming uses the existing Rust
splitter or usd-brep with `native-smlib`. The shared curve getter provides source
picking, point constraints, numeric units, direction locking and portion previews.
The current source getter accepts standalone curves.

The 82 retained native commands cover points, lengths, opposite confirmations,
closed charts, custom domains, direction reversal, seam endpoints, full and
partial deletions, cancellation, metadata and history. Seventy-eight complete
invocations are replayed through the command API; all 82 are exercised through
the UI. Replays compare classes, complete NURBS definitions, domains, polyline
parameters, nested segments, 33 sampled positions, object identity, metadata and
selection/history at `1e-6`. This matrix does not establish arbitrary-curve
numeric direction, pixel-level preview parity or a continuous approximation
certificate. See the [validation record](../delete-subcurve-validation.json).

Top-level NURBS file transfer now retains an explicit rational representation
with unit weights. `has_rational_representation` reports that representation;
`is_rational` continues to report varying weights. Basic trims, direction/domain
edits and affine transforms preserve the flag. Nested geometry codecs still
infer their representation from their controls.

```sh
cargo test -p viboceros-command --test delete_subcurve
cargo test -p viboceros-command --features native-smlib --test delete_subcurve
cargo test -p viboceros app::tests::delete_subcurve
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/deletesubcrv.htm)
