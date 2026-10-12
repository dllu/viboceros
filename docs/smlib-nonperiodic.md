# Native nonperiodic conversion

Linux builds with `--features native-smlib` now use usd-brep for MakeNonPeriodic
on curves and tensor surfaces. The command retains its mixed-selection policy,
source identities, attributes, groups, selection, error handling and atomic
Undo/Redo. Nonperiodic objects are skipped; a wholly ineligible selection still
fails without changing the document.

The converter shares the existing native endpoint-split operation from
[knot refinement](smlib-knot-refinement.md). A periodic curve is clamped at both
active endpoints. A tensor clamps only periodic directions selected by U, V or
Both; both original flags are captured before editing either direction.
Nonperiodic inputs and already-completed directions return the original
definition unchanged. No new control blending, endpoint extraction or fitting
routine is added. Native command and oracle paths exclude the portable endpoint
control solver for this conversion.

Complete definitions enter the native adapter, including periodic padding,
unclamped outer knots and their stored weight gauges. The native split/reversal
workarounds and binary weight scaling from the shared adapter remain in effect.
The [limitations ledger](usd-brep-limitations.md) records those upstream issues;
this integration adds no claim that arbitrary native end splitting is safe.

Fresh Rhino 8.32 observations retain two curve and two surface conversions,
including cubic, rational, single-direction and doubly periodic cases. All four
complete raw definitions match at absolute `1e-10`, relative `1e-12`: degree,
domains, controls, weights, knots, closed state and periodic flags remain in the
comparison. The [retained report](smlib-nonperiodic-comparison.json) includes
all outcomes and input hashes without comparison-time normalization.

Independent bridge tests check source purity, idempotence, selected U/V/Both
directions, unchanged active domains, 65 curve points and 17×17 surface grids.
Curve gauges include `1e-300`, `1`, `1e300` and `-1e300`. Existing command
regressions check replacement in place, attributes, groups, selection, Undo and
atomic invalid-input rejection. These qualify the recorded inputs; they are
not continuous Rhino-parity certificates for arbitrary definitions.

The shared adapter's degree-17 and one-million-control limits apply to native
edits. No-op nonperiodic definitions are returned before entering the kernel.
Pathological weight ranges and broader chart/coordinate extremes remain
unqualified. Other builds retain the existing portable implementation.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test knot_refinement nonperiodic
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib make_non_periodic
```

The first isolated live capture timed out. Its retry completed on an isolated
display using private profile `VibocerosOracleSmlibBridgeFinal20261010`.
Harness timings do not establish kernel speedups against Rhino.

Verification passed 1,313 native command tests (two ignored), 79 native bridge
tests (one ignored), all 1,095 Python oracle tests, portable command and oracle
conversion checks, and native/portable workspace clippy with warnings denied.
Workspace formatting also passed.
