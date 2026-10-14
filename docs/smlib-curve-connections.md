# Native Connect, Fillet and Chamfer preparation

Linux `native-smlib` builds reuse the
[native boundary-extension producers](smlib-boundary-extension.md) to prepare
smooth NURBS ends for `Connect`, `Fillet` and `Chamfer`. The shared curve-pair
policy delegates extrapolation and interval extraction through
`CurveExtensionOperations`; native builds select usd-brep's
`N_CrvExtendByParamDist` and the existing checked segment extractor.
NURBS-to-line connections reuse the boundary-hit adapter. NURBS pairs extend
both terminal spans, independently check intersection parameters and meeting
positions, and extract the retained intervals with the same native producers.

Chamfer also uses native extraction for partial retained NURBS leaves after
measuring its two arc-length setbacks. Fillet's corner/contact solver and its
final contact trimming remain shared portable geometry. Picking, analytic
arc/line policies, source directions, composite leaves, attributes, groups and
Undo remain shared command behavior. Native geometry errors retain their
command error type; preparation finishes before document mutation.
Degree-18 and higher NURBS retain portable extrapolation and extraction.

The [native regression](../crates/viboceros-smlib/tests/curve_connections.rs)
checks 96 combinations of NURBS-to-line/pair connections, reversed ends,
shifted parameter charts, composite leaves and common signed weights
`1`, `1e-280`, `-1e280`. Complete definitions and 33-point grids agree with
the portable outputs within `1e-9`. The independent parabola
`C(t)=(2t,t²,0)` checks connection endpoints; unselected composite leaves and
source definitions remain exact. The same cases check retained chamfer
pieces. Fillet checks positive-radius tangency and zero-radius joined/separate
outputs at all three gauges. Degree-18 fallback is exercised separately.
Injected producer failures check error propagation through all three policies,
including Chamfer's final extraction, and meeting Connect sources avoid edits.

The Python replay uses the command-selected backend for four saved scripted
Connect cases and three public SDK pair-filleting fixtures. Connect checks
ends and controls at `1e-10`; the existing NURBS fillet sample/length ceiling
is `1e-7`, and analytic fillets use `1e-10`. The
[retained comparison](smlib-curve-connections-comparison.json) includes raw
native responses, per-case results, engine identities and source hashes.
All 24 cases pass. Connect's maximum coordinate difference is `7.07e-14`;
the largest fillet length/sample difference is `2.87e-8`. The complete native
suite passes 236 tests, including its doc test; targeted command/geometry
regressions and native/portable Clippy checks also pass.
These observations come from the saved licensed Rhino outputs. Fresh Rhino
capture is still blocked by the
[recorded Wine startup failure](smlib-extension-rhino-capture.json).

[Rational smooth Connect parity](commands/connect.md) remains unresolved.
Its eight retained Rhino probes reject some operations or extend only the
line; mathematical continuation here can produce different document geometry.
Chamfer has native/portable and command regression coverage, but a live Rhino
comparison is pending. Whole-model compatibility and performance against
native Rhino remain further work.

```sh
cargo test --release -p viboceros-smlib --features native --test curve_connections
cargo test --release -p viboceros-geometry --lib curve_connect_pair
cargo test --release -p viboceros-command -p viboceros-oracle \
  --features viboceros-command/native-smlib,viboceros-oracle/native-smlib fillet
python3 -m tools.numerics.compare_native_curve_connections /tmp/connections.json
```
