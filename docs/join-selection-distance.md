# Surface Join selection distances and cluster order

[Command policy](commands/join.md#surfaces-and-polysurfaces) ·
[Spatial rebuilding](join-gap-rebuilding.md) ·
[Complete replay report](join-selection-distance-comparison.json)

Actual `Join` and `JoinCopy` commands in Rhino 8.32.26160.13001 distinguish
preselected batches from command-first picks. The adapter now requests a
certified distance strictly below `1.8 * document_absolute_tolerance` for the
former and `2.1 * document_absolute_tolerance` for the latter. It uses the
preceding binary64 value of the scaled radius; multiplication overflow remains
an error, not a saturated distance. Explicit kernel callers still supply their
own distance. No geometry certificate, source-uncertainty bound, or
excessive-movement guard was weakened.

These are measured command policies, not a universal statement about every
Rhino Join API or boundary representation. The near-cutoff behavior below and
ordered cluster selection remain unresolved.

## Threshold probes

The [90 requests](../tools/rhino_oracle/fixtures/join_surface_thresholds.json)
and [raw observations](../tools/rhino_oracle/observations/join_surface_thresholds.json)
cover coplanar box sheets at origins 0 and 3, document tolerances `0.0001`,
`0.001`, and `0.01`, both commands, both selection modes, and adjacent binary64
values at the cutoffs. They are actual commands on shared per-source 3DM inputs,
not RhinoCommon `JoinBreps` calls. All runs use owned private-Xvfb sessions.

At document tolerance `0.001`, ordinary preselected gaps of `0.00175` join while
`0.00185` does not. Command-first gaps of `0.00201` join while `0.00211` does not.
Immediately below the scaled radius at origin 0, both commands join. At the
exact `2.1 * 0.001` binary64 value they fail unchanged. At the exact
`1.8 * 0.001` value, however, Rhino returns two unjoined sheets with only the
top corner pair moved to its midpoint. The bottom pair remains separated;
the second sheet's vertex table starts at its moved top corner.

83 records fully match every geometry and document field at `1e-10` absolute /
`1e-12` relative epsilon. Seven retain the one-endpoint behavior: four scaled
preselection thresholds, two exact-cutoff preselected Join/JoinCopy cases, and
one translated command-first threshold. Native may leave both endpoints
unchanged or join both, depending on their certified distance. Tests check the
raw Rhino corner positions, unchanged underlying surfaces and copied originals;
these seven cases are not counted as matches. No special-case endpoint rule is
implemented to imitate an unexplained numerical outcome.

Command-level tests additionally exercise 96 interior/exterior combinations of
scale, translation, command, and selection mode, checking attributes, groups,
copied-source immutability, selection, failure rollback/redo, and two undo/redo
cycles.

## Ordered cluster discovery

The [60 requests](../tools/rhino_oracle/fixtures/join_cluster_order.json) and
[raw observations](../tools/rhino_oracle/observations/join_cluster_order.json)
retain all discovery cases: three- through five-sheet chains, source-order
permutations, unequal gaps, and a five-way star. Angled sheets expose additional
short partial edges near corners. A second set uses parallel flat sheets with
long common boundaries to separate that effect from clustering.

The 12 command-first records fully match. All 48 preselected records differ:
20 hit the native transitive-cluster excessive-movement guard, and 28 return
different topology. Merely splitting oversized clusters, picking the closest
pair, or using an order-independent mean does not explain the reference.
For example, flat sheets initially at `z = [0, 0.0015, 0.003]` give:

| Input order | Joined original sheets | Final bottom z, by original sheet 0/1/2 |
| --- | --- | --- |
| 0, 1, 2 | 0 and 1 | 0.00075, 0.00075, 0.003 |
| 1, 0, 2 | 1 and 2 | 0.0015, 0.0015, 0.0015 |
| 1, 2, 0 | 1 and 0 | 0.0015, 0.0015, 0.0015 |
| 2, 1, 0 | 2 and 1 | 0, 0.00225, 0.00225 |

Thus even an unjoined sheet can move, and a middle-first ordering chooses a
different mate. Four- and five-sheet cases further vary the number of outputs
and means. The current kernel still forms transitive endpoint clusters and
selects mutual unique edge candidates; this audit does not claim to solve that
policy. Excessive adjustments continue to fail atomically.

## Aggregate evidence and reproduction

The report replays every saved surface-command observation, including prior
discrepancy archives: **427 of 510 fully match**. The 83 differences comprise
16 zero-volume face-sense cases, nine cutoff-endpoint cases, 50 ordered-cluster
cases (22 native guard rejections), and eight area-integration cases supported
by the independent witness in the spatial-rebuilding audit. Raw data are not
normalized or filtered to improve the count. Fixture, observation, adapter, and
release-executable hashes identify the replay. Harness timings do not establish
kernel performance parity.

```sh
cargo test --release -p viboceros-command selection_mode_changes_gap_acceptance
cargo test --release -p viboceros-oracle join_command::tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_surface_thresholds.json --timeout 480 --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

The final command deliberately exits unsuccessfully for the seven archived
differences; it must not be interpreted as an all-passing suite.

A fresh private-Xvfb recheck reproduced exactly those seven differences; the
other 83 cases had maximum numeric residual `1.2434497875801753e-14`.
Verification passed 2,813 Rust tests (20 ignored), 222 Python tests, formatting,
and Clippy/Rustdoc with warnings denied. The report also records the fresh
recheck summary and its original timed comparison hash.
