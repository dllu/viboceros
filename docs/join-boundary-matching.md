# Surface Join boundary matching

[Commands](commands/join.md#surfaces-and-polysurfaces) · [Kernel](brep-edge-joining.md)

The follow-up audit adds 160 actual Rhino Join/JoinCopy operations, using owned
private-Xvfb sessions and identical per-source 3DM files. Native export roundtrips
and Rhino insertion are checked before measuring the named command's EndCommand
state. All spatial/UV definitions, tables, tolerances, surfaces, face senses,
integrals, object identities, attributes, groups, and selection remain raw.

The [142-case matching fixture](../tools/rhino_oracle/fixtures/join_boundary_matching.json)
and [observations](../tools/rhino_oracle/observations/join_boundary_matching.json)
compare every field at `1e-10` absolute / `1e-12` relative epsilon. Coverage includes
88 partial-overlap cases: contained, prefix/suffix, crossing intervals, reversed
directions and source/edge order, shifted domains, and one-to-many boundaries.
Other cases cover competing planes, overlapping walls, existing polysurface
inputs, and command-first rejection of a third nearby boundary.

A [fresh live comparison](join-boundary-matching-comparison.json) passed all 142
cases with a maximum absolute difference of `2.842170943040401e-14`. The report
records fixture, archived observation, and native executable hashes. The full
workspace passed 2,798 Rust tests (20 ignored), 222 Python tests, and Clippy and
Rustdoc with warnings denied.

## Assembly policy

Original vertices from every input precede inserted cut vertices. The union
representative is therefore an original endpoint when available. New edges stay
in their source's table, independently of this vertex precedence. Complete
boundaries retain their curve ahead of newly subdivided boundaries; when both
are subdivided, the later source's curve survives.

The matcher first accepts only mutual unique candidates. A three-way boundary
does not acquire an arbitrary pair, even if one candidate is closer than another.
Other unambiguous edges can establish a connected component; a second pass then
accepts mutually unique pairs within that component, retaining the later edge.
This resolves a duplicate/overlaid wall pair without attaching the competing
sheet. Existing mated edges inside an original input are not broken or replaced.
The two selection passes are linear in candidates plus topology, apart from
disjoint-set operations and the existing broad-phase/provenance sorting.

Command-first selection reconsiders original accepted objects. Sewing each pick
permanently would hide earlier boundaries from later competition. Candidate
source provenance distinguishes an unrelated pick from a contacting pick that
ultimately becomes a separate output. Picks that would eliminate all cross-source
joins are rejected; picks after complete closure are skipped. Each resulting
piece inherits the command's initial seed attributes/groups. Batch selection
instead retains per-component source provenance and attributes.

## Numerical correction

An affine line from `-1` to `2` over parameter `[0,3]` crosses zero at `1`.
Rounding the knot-insertion fraction first previously produced a nonzero trimmed
endpoint, which then inflated a join tolerance by the model-validation floor.
Single-span polynomial-line trimming now evaluates each new endpoint with exact
rational arithmetic and one final binary64 rounding. Source weights and native
parameters are retained. Whole-domain trims remain bitwise no-ops. Tests cover
negative/common extreme weight gauges, translated domains, overflowing coordinate
differences, and subnormals. Higher-degree and genuinely rational lines retain
their existing trimming paths; this does not claim universally exact NURBS editing.

## Remaining differences

The [18-case discrepancy fixture](../tools/rhino_oracle/fixtures/join_boundary_differences.json)
and [raw observations](../tools/rhino_oracle/observations/join_boundary_differences.json)
retain twelve duplicated-wall cases differing only in the face-sense array of
the zero-volume double sheet. Every other field agrees, including sorted oriented
incidence, since the two coincident faces exchange senses. Native preserves its
first face's sense; no guessed canonical normal is imposed.

Six nearby three-way cases remain unjoined in both engines, but Rhino moves the
three boundary edges/vertices to their average location and updates incident
geometry/tolerances. With two edges at `z=0` and the third at `z=d`, the recorded
outputs use `z=d/3`. Underlying surfaces and UV trims remain unchanged. Native
retains original spatial geometry. This shows that Rhino's gap rebuilding is not
limited to successfully mated edges; it needs a distinct geometry-adjustment
policy, not tolerance suppression or output normalization.

The prior 26-case archive is retained unchanged: four partial-overlap cases now
fully match, four duplicate-wall cases now differ only by zero-volume face senses,
and eighteen gap rebuilding/threshold cases remain. Across both audits, 212 of
252 recorded surface-command cases match every recorded field; 40 remain explicit.
These finite suites do not establish general curved-overlap, cavity-solid,
non-manifold, or performance parity.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle join_command::tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_boundary_matching.json --timeout 480 --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```
