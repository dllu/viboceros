# Point coordinate precision

[Coordinate input](point-input.md) · [Point commands](commands/points.md) · [Oracle](oracle.md)

Complete point arguments accept an optional `w`/`W` world prefix and Cartesian
calculator components. For example:

```text
Point w4503599627370497/37778931862957161709568,0,0
Point pow(2,3),sqrt(16),sin(0)
Point 1+2 4-1 5/16
```

The parser retains its direct numeric path and shares the interactive prompt's
bounded calculator for expressions. Complete point arguments have world
coordinates and no previous-point or model-unit context. Relative input and
physical length suffixes require the interactive prompt. Invalid inputs leave
the document and history unchanged.

## Native measurements

The [64 recipes](../tools/rhino_oracle/fixtures/point_input_precision.json)
and [raw Rhino 8.32 observations](../tools/rhino_oracle/observations/point_input_precision.json)
cover zero, neighboring doubles around `2^-23`, negative X, positive Y/Z,
four equivalent numeric formats and an optional public ScalePositions reference
seed. Captures run at idle in empty owned documents on private Xvfb, using four
separate settings schemes. The public Point command receives the prescribed
world token; SDK point-object coordinates are read at EndCommand and afterward.

All formats preserve exact binary64 coordinate bits. Equality at `2^-23` has
bits `3e80000000000000`; the next larger double is `3e80000000000001`.
Rhino's public [Vector3d.IsTiny](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.vector3d/istiny)
includes the former and excludes the latter at tolerance `2^-23`.
Independent Python checks use integer bit expectations and verify format and
seed agreement. Rust tests replay 64 complete commands and 128 complete or
incremental app placements. App Undo/Redo checks establish application history
invariants; native document history is not compared here.

## Remaining diagnostics

A custom public SDK GetPoint harness timed out before publishing a response
after acknowledged input of the negative neighboring value. Its
[raw timeout](../tools/rhino_oracle/observations/point_input_precision_getpoint_timeout.txt),
[probe](../tools/rhino_oracle/capture_sources/point_input_precision_getpoint_probe.py),
[driver](../tools/rhino_oracle/capture_sources/point_input_precision_getpoint_input.py)
and [request](../tools/rhino_oracle/capture_sources/point_input_precision_getpoint_request.json)
are retained as historical diagnostics. The successful Point command matrix
does not establish SDK getter behavior or identify the cause of that stall.

These Point results also do not establish the origin consumed internally by
ScalePositions. Its [two unstable boundary recipes](scale-positions-cursor.md)
remain excluded from parity counts. Oblique planes, other expression families,
extreme coordinates, performance and full native parity remain unmeasured.
The [manifest](point-input-precision-provenance.json) records source/artifact
hashes, schemes, counts and comparison scope.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.point_input_precision_capture tools/rhino_oracle/fixtures/point_input_precision.json --scheme VibocerosOraclePointPrecisionFresh --output /tmp/point-precision.json --timeout 180
python3 -m unittest tools.rhino_oracle.test_point_input_precision
cargo test -p viboceros-command --lib point_precision
cargo test -p viboceros --bin viboceros native_point_command_precision
```

The capture retains completed batches beside the output in an `.batches`
directory. Repeating it reuses a batch only when inputs, settings scheme and
source hashes agree; a failed batch has no successful checkpoint.
