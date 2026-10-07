# SubCrv FromMidpoint

[SubCrv](commands/subcurve.md) · [MarkEnds](subcurve-mark-ends.md)

`FromMidpoint=Yes` makes the first curve pick the center. A second location
sets the arc-length distance from that center to either end. A typed number is
that same half-length, using its magnitude, and completes immediately. It does
not require the ordinary numeric mode's direction-confirmation pick. Point and
numeric intervals retain source orientation. `FromMidpoint=Yes|No` can change
during input without editing the document.

Open sides clamp independently at their natural endpoints, including when the
center itself is an endpoint. Closed endpoints wrap through the original chart;
the result is the forward interval between them. Consequently a radius above
half the perimeter can produce the wrapped remainder rather than an extra
traversal. Half/full-perimeter radii give coincident endpoints and no result in
the recorded cases. Zero also cancels without history. Larger-than-curve radii
are rejected for retry. The same interval supports Copy, replacement and MarkEnds.

```text
SubCrv 2,3,0 3,4.5,0 FromMidpoint=Yes Copy=Yes
SubCrv Numeric=center_parameter,half_length,unused_confirmation FromMidpoint=Yes
SubCrv Numeric=center_parameter,half_length,unused_confirmation FromMidpoint=Yes Mode=MarkEnds
```

The command-layer midpoint policy reuses anchor-relative signed length queries
for both sides. Native endpoint parameters come from those queries, with seam
wrapping rather than geometric closest-point recovery. The mathematical kernel's
strict, signed, unclamped length API remains separate.

The [22 closed recipes](../tools/rhino_oracle/fixtures/subcurve_midpoint.json)
ran on private Xvfb under `VibocerosOracleSubcurveMidpointFinal20261007`.
[Raw records](../tools/rhino_oracle/observations/subcurve_midpoint.json) retain
original/result definitions and stations, marker coordinates, metadata, groups,
selection, command events and Undo/Redo. Nineteen commands succeed; zero and the
two coincident-end closed cases remain cancellations. Command and app replays
check loci and directed endpoints or marker points at `1e-6`, metadata, source
retention and history. See [provenance](subcurve-midpoint-provenance.json).

The solver inherits numerical integration and parameter-resolution limits.
Arbitrary-curve parity, live preview, direction locking, B-rep edge input,
restart preference persistence and reactive History remain outstanding.
[Session option memory](subcurve-option-memory.md) is shared by the registry
and GUI and survives cancellation and document history.

```sh
cargo test -p viboceros-command --release midpoint
cargo test --release --bin viboceros subcurve_midpoint
```
