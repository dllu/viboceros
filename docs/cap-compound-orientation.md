# Compound Cap orientation

[`Cap`](commands/cap.md) now uses the conservative
[spatial orientation query](solid-orientation.md) when normalizing a newly
closed B-rep. Total signed volume is not a reliable orientation test for a
compound solid. Nor should every connected shell be independently turned
outward: that would erase inward cavities and observed mixed-shell senses.

For example, capping an outward box of volume 8 at X = −10 and an inward box
of volume 64 at X = +10 leaves the whole result spatially outward in Rhino,
with signed volume **−56**. Reversing both source shells produces the same
normalized result. With equal-sized opposing boxes, total volume is zero,
but an inward result still requires a global reversal.

## Command policy and limits

- Exact `Inward`: reverse the whole newly capped B-rep.
- Exact `Outward`, or a result that is still open: preserve face senses.
- `Unknown`, one edge-connected shell: use numerical signed-volume sign.
  This retains support for curved and higher-degree trim representations not
  yet covered by the exact classifier. It has no rigorous integration error
  certificate and does not validate an embedded, non-self-intersecting solid.
- `Unknown`, multiple shells: preserve senses and report
  `orientation unresolved for N compound solid(s)` in the command message.
  Cancellation or domination of component volumes cannot choose their sense.

The kernel capping operation remains orientation-preserving. The approximate
single-shell fallback is command policy, not part of the exact orientation API.
Already closed inputs remain no-ops. Geometry replacements still form one
atomic undo step with IDs, attributes, groups, and selection behavior retained.

Native regressions cover 24 negative-volume command workflows, both senses of
zero-volume compounds, and unresolved compounds with geometrically equivalent
quadratic UV trims. The latter deliberately exercise unsupported representation
handling; they are not additional Rhino observations.

## Retained Rhino evidence

The [source-only generator](../tools/rhino_oracle/references/cap_compounds.py)
produces the [100-case request](../tools/rhino_oracle/fixtures/cap_compounds.json).
The [unaltered Rhino 8.32 observations](../tools/rhino_oracle/observations/cap_compounds.json)
come from one completed owned private-Xvfb session. The matrix varies separation
along X/Y/Z, relative shell sizes, global reversal, component order, pre/postselection,
and whether one or both components are open. It also includes equal-volume
opposing shells, nested cavities, and coincident opposing shells.

Both engines consume the same native-exported 3DM source. Export verifies its
native round trip; the Rhino probe verifies that document insertion did not
change the recorded input before invoking the public command. EndCommand
snapshots retain output topology and document state. Probes delete only owned
objects and restore selection, layers, and groups.

At absolute epsilon `1e-9`, relative epsilon `1e-10`:

| Stage | Full matches | Orientation differences | Coincident topology differences |
| --- | ---: | ---: | ---: |
| Former total-volume policy | 68 | 28 | 4 |
| Spatial policy | 96 | 0 | 4 |

The 28 resolved records comprise 24 negative-volume X-separated cases and four
reversed zero-volume cases. All 140 earlier ordinary Cap records still match.
Independent Python box-area/volume witnesses check the 96 noncoincident Rhino
outputs and the complete matrix's input vertices, input areas, and document
attributes; they do not obtain their expected integrals from either engine.

The four coincident-opposed cases remain genuine topology differences:
native produces 12 faces, 24 edges, and a topologically solid zero-volume result;
Rhino produces 11 faces, 32 edges, and an open result. Neither a global flip nor
a guessed coincident-shell orientation rule resolves these discrepancies.
Regression tests retain both outcomes rather than filtering them into matches.
The complete [before](../tools/rhino_oracle/observations/cap_compounds_before_report.json)
and [after](../tools/rhino_oracle/observations/cap_compounds_after_report.json)
comparison reports retain every discrepancy and each case's numeric residual.

Comparison records include physical vertices, full spatial edge definitions,
domains and samples, oriented loop-to-edge incidence, face areas, signed volume,
solid/manifold flags, and identity/attributes/groups/selection. They deliberately
do **not** establish parity of generated cap UV frames, surface coefficients or
domains, loop starts, or face insertion order. See
[provenance and hashes](cap-compound-provenance.json). JSON compaction preserved
every field and IEEE-754 numeric bit pattern, including negative zero. Timing
fields are zero; this audit makes no performance claim.

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/cap_compounds.json \
  --observations tools/rhino_oracle/observations/cap_compounds.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-10
cargo test --release -p viboceros-command cap::
cargo test --release -p viboceros-oracle cap_
python3 -m unittest tools.rhino_oracle.test_cap_probe
```

Replay correctly exits nonzero for the four retained topology differences.
