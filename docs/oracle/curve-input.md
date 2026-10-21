# Curve input and interpolation comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

`control_point_prompt_rhino_only.json` is a diagnostic exception: it exercises
Rhino's Curve point prompt and records resulting control points, rather than
running the geometry constructor. Run it with `run_headless.sh rhino
tools/rhino_oracle/fixtures/control_point_prompt_rhino_only.json --timeout 300`;
the native oracle does not implement this operation. Its [recorded response](../control-point-prompt-measurement.json)
checks adjacent-point rejection; see [typed point input](../point-input.md).
The [threshold follow-up](../control-point-threshold-measurement.json) embeds three
additional requests and their responses (30 cases), establishing a fixed,
coordinate-wise `2^-32` comparison for the tested near-origin controls.
The app test `recorded_rhino_curve_threshold_sequences_match_interactive_completion`
replays all 30 stored requests with their document tolerances and checks accepted
controls, output degree, closure, and control locations against Rhino's responses.
JSON uses round-trip float parsing so adjacent binary64 boundary values remain distinct.

`interpolation_point_prompt_rhino_only.json` is a separate eight-case diagnostic
using the same coordinate whitelist and private construction-plane wrapper for
InterpCrv (open cubic, chord knots). Explicit command rejections become
`command_succeeded: false` records; unexpected failures still fail the batch.
Its [recorded response](../interpolation-point-prompt-measurement.json) exposed an
open-curve point-tolerance discrepancy that is now corrected. An app test replays
the six successful cases and compares control points within `1e-9`; two Rhino
solver rejections remain diagnostic. Run it with `run_headless.sh rhino`, not
the native comparison runner.

`interpolation_closure_prompt_rhino_only.json` checks ordinary three-point
closure using `_Close` and `_Sharp _Close`, in that order, in a fresh private
Rhino session. The [recorded response](../interpolation-closure-prompt-measurement.json)
replays through app completion and matches both sets of control points within
`1e-9`. This is a baseline, not a near-seam tolerance measurement or a test of
arbitrary persistent option state. In this Rhino build, `_Sharp` alone toggles
the option and leaves the point prompt active. Exploratory `_Sharp=_No _Close`
and `_Sharp=_Yes _Close` both produced the sharp curve; those spellings are not
used by the retained fixture. Closed prompt probes are restricted to InterpCrv.

The [closure tolerance follow-up](../interpolation-closure-tolerance-measurement.json)
embeds two requests and their responses: a `0.001` interior or seam-adjacent
offset, smooth or sharp closure, at absolute tolerances `0.01` and `1e-9`.
Both batches use the same operation order in fresh private Rhino sessions.
All eight results replay through app completion with control-point error at
most `1e-9`. Command interpolation now retains these distinct points instead
of rejecting or merging them using model tolerance. This does not measure
the exact duplicate/near-zero seam threshold or automatic closing behavior.
The separate [auto-close audit](../interpolation-auto-close-measurement.json)
embeds four requests and responses (18 cases) that establish an inclusive
Euclidean `1.490116119385e-8` prompt-closing threshold near the world origin.
The `PointOnly` diagnostic ending sends no Enter or closure option: its exact
seam case confirms completion from the point itself. Use that ending only to
probe suspected automatic completion; non-closing input may wait until the
client timeout. App regressions replay every measured geometry and distinguish
automatic completion from the cases that still require Enter.

The [translated follow-up](../interpolation-translated-auto-close-measurement.json)
records `periodic` separately from `closed` for 13 cases. At X=`±1e6`, curves
with endpoint gaps of `2e-8` are closed under Rhino's coordinate-relative
topology test but are not periodic; a two-point return also produces a closed,
non-periodic result. App replay must not infer periodicity from closed state.
New InterpCrv prompt measurements include both properties; older responses
still check their recorded control geometry without inventing periodicity data.
A [two-point no-Enter probe](../interpolation-two-point-auto-close-measurement.json)
confirms automatic completion with non-periodic output. Consequently neither
`closed` nor `periodic` alone establishes when the point prompt finished; the
`PointOnly` ending directly tests completion. The app now auto-closes two-point
returns with a non-periodic cubic instead of requiring a third collected point.

The [degree-one follow-up](../interpolation-degree-one-auto-close-measurement.json)
records six Enter-completed sequences, all replayed through preview/completion
with control errors at most `1e-9`. Separate `PointOnly` probes for two and three
collected points followed by `w1e-8,0,0` both waited until the 100-second client
timeout. The owned private windows were inspected while live and still showed
the next-point prompt after accepting that input; these are prompt observations,
not successful geometry responses. Separate [exact-return probes](../interpolation-degree-one-exact-close-measurement.json)
do complete without Enter for both point counts. Degree-one completion at a
nearby endpoint therefore differs from completion at exact equality, even when
its eventual endpoint is reconciled to the start.
A further no-Enter batch timed out during its first `1e-10` offset; its subsequent
`2^-32` case was not reached and must not be treated as measured.

The [degree-one reconciliation boundary audit](../interpolation-degree-one-seam-boundary-measurement.json)
contains nine successful Enter-completed cases: equality and the next float
around `1.490116119385e-8`, a longer diagonal, two model tolerances, and three
translated endpoints at X=`1e6`. It confirms the current distance-based
reconciliation rule within control-point error `1e-9`. Replay also compares
the cached preview to completion for every still-active open prompt; this
does not establish a tighter automatic-completion threshold.
