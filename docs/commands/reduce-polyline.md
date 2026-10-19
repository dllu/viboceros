# ReducePolyline

Select polylines and enter `ReducePolyline`, or start the command, select the
curves, and press Enter. Enter a positive reduction tolerance; `Copy=Yes` keeps
the sources. Enter accepts the displayed tolerance, initially the document's
absolute tolerance. Accepted values persist across commands and Undo/Redo in
the same command registry. `RememberCopyOptions` governs the Copy default.

```text
ReducePolyline 0.01
ReducePolyline 0.1 Copy=Yes
ReducePolyline Tolerance=0.05 Copy=No
```

`Tolerance=value` is also accepted by scripts. The native numeric prompt's
option is `Copy`; the reduction tolerance is entered as a number.

The kernel follows the public [Rhino3dm C# reduction](../../third_party/rhino3dm-polyline/README.md)
with an explicit stack and cached chord directions. Douglas-Peucker reduction
uses distance to finite chords and keeps the first furthest vertex on ties.
Vertices at the tolerance threshold are removed. Endpoints and the existing
closed seam remain; native-recognized closed triangles are unchanged at any
tolerance. Working memory
is linear; the usual Douglas-Peucker worst case remains quadratic.

Polyline form is recognized using public OpenNURBS policy, including rational
degree-one NURBS, elevated linear Bezier spans, and composite/nested polylines.
Other selected curves are ignored. Recognition retains the native thresholds;
it does not fit arbitrary curved inputs. A curve with no removed vertices keeps
its representation, parameters, metadata and redo history, even with Copy enabled.

Changed replacements retain object identity, attributes and groups, with unit
vertex parameters starting at zero. Geometry user text is cleared because the
polyline is reconstructed. Copies use chord-length parameters, fresh attributes
on the current layer, and no groups or user text. Command-first replacements
finish unselected. With Copy enabled the sources remain selected at completion
and are released on Undo/Redo. Each successful edit makes one undo step.

Rhino can collapse a sufficiently narrow or over-reduced closed polyline into
an invalid curve. Viboceros rejects these results before changing any source,
selection, history or remembered tolerance. This is a deliberate validity guard;
the retained invalid native results are tested as atomic failures, separately
from matching valid results.

The retained 58 actual Rhino 8 commands cover 56 valid results and two invalid
collapses. Replays compare vertices, parameters, curve definitions, 33 evaluated
points, identity, metadata, selection, copies, and Undo/Redo at `1e-8` absolute
epsilon. Source files and capture-time helper hashes are retained. Additional
tests cover malformed native buffers, cancellation, defaults, redo retention,
large flat inputs and extreme coordinate scales. These are bounded command
comparisons, not a general continuous-error certificate. The
[validation record](../reduce-polyline-validation.json) contains results, logs,
and the measured 1,024-vertex SDK/kernel timing comparison.

For that near-straight input, median reduction time was 17.5 µs for release Rust
on ARM and 47.2 µs for Rhino 8's SDK under FEX. Calls exclude input construction,
document updates and file I/O, with two warmups followed by seven Rust samples
and three SDK samples. Other inputs and hardware require separate measurements.

```sh
cargo test -p viboceros-command --test reduce_polyline
cargo test -p viboceros app::tests::reduce_polyline
cargo test -p viboceros-geometry --release --test polyline_reduction -- --nocapture
python3 -m unittest tools.rhino_oracle.test_reduce_polyline_probe
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_reduce_polyline
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/reducepolyline.htm)
