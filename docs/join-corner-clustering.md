# Isolating surface Join corner clustering

[Selection-distance audit](join-selection-distance.md) ·
[Diagnostic replay API](oracle-replay.md) ·
[Complete replay report](join-corner-clustering-comparison.json)

This audit isolates endpoint rebuilding from edge mating. The
[84 requests](../tools/rhino_oracle/fixtures/join_corner_clustering.json) and
[raw Rhino records](../tools/rhino_oracle/observations/join_corner_clustering.json)
use planar sheets whose origin corners are near each other but whose boundary
directions differ. Every Rhino output remains a separate single-face sheet with
four unmated edges. Thus the observed movement is not caused by choosing a
full-edge mate or adding short partial edges.

All observations are actual preselected `Join` commands in Rhino
8.32.26160.13001, on identical per-source 3DM files in owned private-Xvfb sessions.
The three successful batches record 24 ordered/unequal chains, 32 isolated pairs,
and 28 asymmetric/exact-binary cases. Every discovery case is archived, including
failures; no kernel clustering policy was changed on the strength of an
incomplete explanation.

## What the probes distinguish

Isolated pairs average normally through gaps just below `1.8 * 0.001`, at both
tested translations and in both orders. Yet adding other nearby corners changes
both grouping and position. For four sheets at `z = [0, 0.0015, 0.003, 0.0045]`
in that order, the first pair averages to `0.00075`; the third sheet's corner
snaps to the fourth's original position instead of their midpoint. Its original
corner vertex is removed and the moved corner appears last in its vertex table.
The same third/fourth pair averages normally when tested alone.

Asymmetric gaps rule out simply grouping the first seed's neighbors. For
`z = [0, 0.0014, 0.0019]`, all six orders leave the first geometric position
unchanged and average the closer pair to `0.00165`. Reversing the gap sizes to
`[0, 0.0005, 0.0019]` always averages the closer pair to `0.00025` and leaves the
last position unchanged. In contrast, when all three points are mutually near,
the entire group averages in every order.

Exactly representable `1/1024` gaps show that order dependence is not merely a
decimal-rounding boundary. For the three original positions `[0, 1/1024, 2/1024]`:

| Input order | Final corner z, by original sheet 0/1/2 |
| --- | --- |
| 0, 1, 2 | 1/2048, 1/2048, 2/1024 |
| 1, 0, 2 | 1/1024, 1/1024, 1/1024 |
| 2, 1, 0 | 0, 3/2048, 3/2048 |

The middle-first result spans twice the neighbor spacing, ruling out a universal
complete-link diameter cutoff as well. Four-point exact-binary cases retain the
one-sided snapping effect. A nearest-pair preference alone therefore also fails
to explain the data. The current transitive-mean implementation and proposed
simple greedy replacements are both contradicted by these observations.

## Replay and remaining work

44 new cases fully match every raw geometry/document field at `1e-10` absolute /
`1e-12` relative epsilon. Thirty have corner-geometry differences and ten reject
an over-wide native transitive cluster. Tests separately retain those errors and
disagreements; they are not normalized matches. The new diagnostic API completes
the whole batch in one native invocation, retaining successful records after
the first rejection.

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_corner_clustering.json --observations tools/rhino_oracle/observations/join_corner_clustering.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
cargo test --release -p viboceros-oracle corner_discovery
```

The replay command deliberately exits 1: 44 matched, 30 mismatched, 10 native
execution failures. The full surface-command replay is now **471 of 594 full
matches**, with 32 classified native failures and 91 other differences. The
previous 510 cases retain exactly their earlier results. More coverage reveals
more limitations; it does not establish general Join parity.

The next kernel revision still needs an evidence-supported grouping rule and
raw vertex-ownership policy. Source surfaces, UV trims, conservative uncertainty,
whole-curve certificates, and the excessive-movement guard remain intact.

Verification passed 2,817 Rust tests (20 ignored), 232 Python tests, formatting,
and Clippy/Rustdoc with warnings denied. Fixture, raw observation, executable,
and replay implementation hashes are recorded in the report. Replaying with
fresh owned exports preserves every previous pass/fail state, numeric residual,
and native error message.
