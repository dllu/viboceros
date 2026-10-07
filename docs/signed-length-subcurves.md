# Signed arc-length subcurves

[UV command inputs](uv-subcurve-input.md) · [Curve parameters](curve-parameters.md)

`Curve3::try_subcurve_at_arc_length(anchor, length, tolerance)` extracts a piece
starting at an original native parameter. Positive lengths follow increasing
parameters; negative lengths reverse direction. Closed curves can cross their
seam for one complete traversal. Unavailable lengths return `None`; zero lengths,
invalid anchors and numerical failures return errors. Lines and circular arcs
retain their analytic forms. Other curves use their existing exact trim/seam paths.

The solver trims or relocates the seam before integrating, rather than adding a
small offset to a potentially enormous cumulative-length prefix. A regression
retains a quarter-unit interval following a `1e16` prefix. Independent tests also
check analytic circle endpoints, signed line intervals, full closed traversals
and unavailable lengths. This remains an accuracy-controlled numerical length
query, with inherited parameter-resolution, domain and integration limits.

Both UV adapters accept scripted lengths in model units:

```text
ApplyCrv Surface=surface-uuid SubCrvLength=curve-uuid,anchor,signed-length
CreateUVCrv Surface=surface-uuid [Face=index] SubCrvLength=curve-uuid,anchor,signed-length
```

Options may repeat and mix with `SubCrv=curve,start,end` and selected whole inputs.
Temporary outputs use default attributes and the current layer. Original sources
remain unchanged; failed length inputs roll back without selection or history
changes. The Python `apply_uv_curves` / `create_uv_curves` input definitions accept
`subcurve_lengths: [[anchor, signed_length], ...]` on curves.

The independent Python operation `curve_subcurve_arc_length` accepts a complete
original NURBS `curve` definition, `start`, and `length`. It reports `available`,
the returned definition and 33 stations, and measured source purity. This geometry
probe reports zero as unavailable; the kernel and document adapters reject zero.

The [20 SDK fixtures](../tools/rhino_oracle/fixtures/signed_length_subcurves.json)
ran on private Xvfb under `VibocerosOracleSignedLengthFinal20261007`.
[Raw native records](../tools/rhino_oracle/observations/signed_length_subcurves.json)
and [local results](signed-length-subcurves-local.json) retain untouched inputs
and outputs. They cover line, quadratic, cubic, stationary-endpoint, piecewise,
polyline, rational circle, ellipse and arc definitions, with 16 returned curves
and four unavailable cases. Replay checks every native station and directed
endpoint within `1e-6`; these finite cases do not certify arbitrary-curve length
inversion. See [provenance](signed-length-subcurves-provenance.json).

Numeric input in Rhino's inline getter remains unresolved. Historical
[16 scripted attempts](../tools/rhino_oracle/observations/subcurve_length_scripted_diagnostic.json)
and [three cursor-assisted attempts](../tools/rhino_oracle/observations/subcurve_length_cursor_diagnostic.json)
did not produce temporary curves for the tested positive lengths. Some parent
commands succeeded with only their other inputs; others failed. These are raw
diagnostics, not native length-entry parity evidence. Their original producer
snapshots were not retained. The current inline UI continues to use endpoint picks.

```sh
cargo test -p viboceros-geometry --release curve_length_subcurve
cargo test -p viboceros-command --release uv_inputs
cargo test -p viboceros-oracle --release curve_length_subcurve
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/signed_length_subcurves.json --timeout 600 --output docs/signed-length-subcurves-local.json
```
