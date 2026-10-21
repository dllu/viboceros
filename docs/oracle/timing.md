# Interpreting comparison timings

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Timing interpretation

The comparison report's `rhino_to_viboceros_ratio` is a ratio of raw harness
elapsed times, **not a kernel speedup**. Its `timing_note` is included in both
the Python report and the CLI's JSON output to keep that qualification with
exported comparisons. Correctness checks do not depend on timing ratios.

The generic measurement helpers run one warm-up before timing repeated calls.
Process startup and transport JSON I/O are outside those loops, but the work
inside a call varies by probe and engine. Rhino's `mesh_unweld_edge`, for
example, still extracts result geometry into Python containers and disposes
the edited duplicate on every timed iteration. Excluding JSON encoding does
**not** necessarily exclude result extraction.

For angle-based `mesh_unweld`, both current implementations time creation of
the edited result and cleanup of the previous result (including the warm-up
result on the first iteration). Rhino duplicates its source for each in-place
edit; native Unweld returns an owned mesh. Both extract only the final result
outside the loop, then release that result. Source mesh construction is also
outside both loops. Worker lifecycle tests check event order and cleanup when
editing, timer reads, or result extraction fail. Historical angle-Unweld
recordings predate this timing-boundary change and included per-iteration
result extraction and disposal; their geometry values remain valid.
A fresh private-session run on Rhino 8.32.26160.13001 with the revised worker
matched all six `mesh_unweld.json` recorded geometry values exactly (100 timed
iterations per case). This verifies unchanged output, not equivalent kernel
performance.

Rhino measurements also include its Python/RhinoCommon bridge and, on this
development host, FEX/Wine overhead. Existing recorded responses retain their
original timings; geometry replay tests do not compare those elapsed times.
Before making a kernel-performance claim, audit the specific operation's
timing boundaries, match copying/extraction/cleanup work, use release-mode
native builds and sufficient iterations, and report the host/emulation setup.
