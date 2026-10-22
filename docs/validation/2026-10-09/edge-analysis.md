# Edge analysis follow-up checkpoints

[Historical checkpoint](../../validation-checkpoint.md)

## Edge analysis checkpoint

[ShowEdges, ShowEdgesOff, ZoomNaked and ZoomNonManifold](../../commands/show-edges.md)
add transient edge display, mode/color controls, source management, zoom
navigation and transactional endpoint marking. Eight regressions cover B-rep
and mesh classifications, geometry edits and Undo, cache reuse, application
selection handoff and hidden-object drawing.

The complete release run passes 5,610 Rust tests with zero failures and 38
ignored tests. Clippy with warnings denied, formatting, diff checks and all
historical source pins pass. The Python implementation is unchanged since the
preceding 1,070-test run. No new live Rhino edge-analysis capture was performed;
native UI, ordering, curved drawing and framing parity remain open. Exact
source and validation-log hashes are in [provenance](../../edge-analysis-provenance.json).

## Native edge-analysis checkpoint

Ten [licensed Rhino SDK recipes](../../edge-analysis-native.md) now qualify the actual
ShowEdges session's edge indices, ordering, classifications, domains and samples.
All 2,268 numeric fields match at 1e-9 absolute plus 1e-12 relative epsilon;
the largest difference is 1.1102230246251565e-16. No fields are excluded.
This adds a permanent native replay and an `edge_analysis` Python oracle
operation. Native dialog behavior and camera framing remain unqualified.

The existing oracle package suite passes 509 tests, and the new ten-case
integration replay is validated separately. The complete Python suite passes
1,073 tests. Clippy with warnings denied, formatting, diff checks and historical
source pins pass. Capture ran under private Xvfb; an initial license-refresh
failure with native-first Wine DLLs produced no observations, while a subsequent
process-local built-in security-DLL setting succeeded. Neither the launcher nor
Wine configuration was changed. Detailed hashes are in
[provenance](../../edge-analysis-native-provenance.json).

## Native marking and navigation checkpoint

Fifty [native command workflows](../../edge-mark.md) match all 654 numeric fields
exactly at 1e-9 absolute plus 1e-12 relative epsilon, without exclusions. They
cover coincident endpoints, shared corners, repeated Mark, navigation and
wraparound, mixed source types, and eight Undo/Redo pairs. The resulting command
keeps every endpoint object, executes actions sequentially and resets fresh
Zoom commands to their first eligible edge.

The initial in-script history capture was insufficient: Undo succeeded but did
not remove marks. Running the final workflows at Rhino idle establishes their
actual history behavior. A reversed-trim implementation hypothesis also failed
the native replay because the surface's U direction followed world Y. The
rejected reports remain diagnostic evidence; the final implementation retains
the original edge order and directions. Exact records and final validation are
in [provenance](../../edge-mark-provenance.json). Dialog controls, camera framing and
broader curved/multi-face geometry remain open.

The corrected-source full release run passes 5,615 Rust tests with zero failures
and 38 ignored tests. All 1,075 Python tests pass; Clippy with warnings denied,
formatting, diff checks and all 1,602 historical/current source pins are clean.

## Edge-analysis input checkpoint

[Command input and session lifetime](../../edge-analysis-input.md) adds independent
persistent ShowEdges and temporary Zoom sessions, command-first selection with
argument retention, navigation prompts, retained marks on Escape and one Undo
group across answers. Shared viewport sampling retains both active edge sets.
Five licensed Rhino window/geometry observations establish the panel lifetime;
sixteen new cancellation records match exactly, including false command results,
accepted points and Undo/Redo. The existing 10 topology and 50 marking captures
still pass the permanent replay suite.

The complete release run passes 5,623 Rust tests with zero failures and 38
ignored tests. All 1,075 Python tests, Clippy with warnings denied, formatting,
diff checks and 1,619 source pins pass. An earlier full run exited 143 without
failure markers; its terminal handle was verified before the successful rerun.
Detailed evidence is in [provenance](../../edge-input-provenance.json). Native widget
details, highlight styling, camera framing and broader geometry remain open.
