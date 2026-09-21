# Surface Join spatial rebuilding

[Command policy](commands/join.md#surfaces-and-polysurfaces) · [Kernel](brep-edge-joining.md)

Automatic `Join`/`JoinCopy` now adjusts nearby spatial boundaries separately from
topological mating. Underlying surfaces and UV trims remain unchanged. Explicit
`Brep::try_join_edge_pairs` remains a geometry-preserving assembly primitive.

## Adjustment and numerical bounds

The naked-edge broad phase also finds nearby endpoints across original sources.
Each endpoint cluster moves to an incident-edge-weighted mean. Thus two ordinary
sheet corners use a midpoint, while a three-edge polysurface corner and a two-edge
sheet corner use weights 3:2. Repeated candidate pairs do not add weight. Means
use exact rational accumulation and one final binary64 rounding: input order,
overflowing intermediate sums, and cancellation do not change the answer.
Vertices remain topologically distinct unless accepted edge pairs merge them.
Corner-only and ambiguous three-way contacts can therefore move without joining.

Every incident clamped curve adjusts its control positions along its original
endpoint chord, including existing mated edges. Chord projection, not a control
index or Greville parameter, interpolates the two endpoint displacements. Weights
and native knots remain unchanged. Short/poorly conditioned chords use end-only
editing; equal-endpoint closed curves translate. The attributed Rust adaptation
of the public OpenNURBS routines is in
[`third_party/opennurbs_rust`](../third_party/opennurbs_rust/README.md), with
exact rational projection/displacement and retained licensing. No proprietary
Rhino source was used.

Source uncertainty is never discarded. A complete natural isoparametric trim
can be certified against an exact control row/column of its clamped surface;
general rounded isocurve extraction is not treated as exact. Changed vertex
tolerances include a conservatively rounded 1.001 margin. Edge uncertainty
uses whole-curve bounds. For common positive rational Bernstein bases through
degree 16, exact homogeneous midpoint subdivision tightens the difference hull,
with depth limited to 16 and work charged to the assembly budget. Evaluations
only guide refinement; every returned upper bound covers the whole interval.
This handles interior-only curved gaps with coincident endpoints.

Other lifted-boundary representations retain conservative propagation, including
the source's validation-tolerance floor. This is not universal trim composition
or an exact Hausdorff-distance solver. Unsupported nonclamped incident curves
retain the unadjusted assembly policy for the whole operation. A transitive
cluster or curve adjustment exceeding the join distance fails atomically.

## Evidence

The audit records 108 actual Rhino commands on identical per-source 3DM files,
in owned private-Xvfb sessions. Records retain raw geometry, table order, face
senses, integrals, tolerances, object identity, selection, attributes, and groups.
Comparisons use `1e-10` absolute / `1e-12` relative epsilon without geometry or
parameter normalization.

The [fresh 88-case comparison](join-gap-rebuilding-comparison.json) passed with
maximum absolute difference `5.648814749292796e-13`. The report records fixture,
observation, executable, and independent-witness hashes. Verification passed
2,809 Rust tests (20 ignored), 222 Python tests, and Clippy/Rustdoc with warnings
denied.

- [88 matching cases](../tools/rhino_oracle/fixtures/join_gap_matching.json) and
  [raw observations](../tools/rhino_oracle/observations/join_gap_matching.json):
  axial/diagonal gaps, tapering, reversed and rational curves, interior-only gaps,
  all three-sheet permutations, corner-only contacts, and prejoined polysurfaces.
- [Four transitive-chain cases](../tools/rhino_oracle/fixtures/join_gap_chains.json)
  and [observations](../tools/rhino_oracle/observations/join_gap_chains.json):
  command-first Join/JoinCopy fully match. Batch native rejects an over-wide
  cluster; Rhino instead builds two smaller joins. This selection policy remains
  unresolved; the guard is not relaxed to allow excessive movement.
- [16 chord-profile cases](../tools/rhino_oracle/fixtures/join_chord_adjustment.json)
  and [observations](../tools/rhino_oracle/observations/join_chord_adjustment.json):
  eight fully match. Eight differ only in face area, including untouched copied
  inputs. All other fields pass in all 16, including cubic/quadratic control
  deformation, rational weights, and component uncertainty.

Independent integration of `|C'(t) × V|` for the two differing extruded profiles
agrees at 60 and 100 decimal digits:

| Profile | Independent area | Rhino area |
| --- | --- | --- |
| Quadratic, middle weight 0.5, first input | 12.2804014516790736166510513012207713138964002899675665078617 | 12.280401451981588 |
| Cubic, polynomial, second input | 20.0000012295410609183878968712561905159663045485941581203529 | 20.000001229907621 |

The native values agree with the independent result within `2e-12`; reference
residuals are about `3.03e-10` and `3.67e-10`. The optional mpmath witness computes
the Bernstein profile and derivative directly, without either geometry engine:

```sh
python3 -m tools.rhino_oracle.join_chord_witness --precision 100
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle join_command::tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_gap_matching.json --timeout 480 --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Twenty older gap discrepancies now fully match; their archived observations stay
unchanged and replay every field. Across all three surface-command audits, 330
of 360 cases fully match. Thirty explicit differences remain: 16 zero-volume
face-sense cases, four threshold cases, two transitive-chain cases, and eight
area cases above. These suites do not establish general curved partial-overlap,
nonclamped rebuilding, solid-classification, or performance parity.

The initial large discovery run lost its owned Rhino process while entering an
opposite-sign taper case; no numerical result was recorded from that run. The
case subsequently completed both alone and in a four-command batch, with all
fields matching. A supplemental launch also timed out before worker progress;
a later sequential launch completed all 16 supplemental cases. Neither failed
session contributes numerical evidence.
