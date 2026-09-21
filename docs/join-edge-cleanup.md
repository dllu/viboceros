# Certified redundant-edge cleanup

[Kernel](brep-edge-joining.md) · [Join command](commands/join.md) ·
[Full raw replay report](join-edge-cleanup-comparison.json)

Automatic Join now coalesces smooth redundant edges in successfully joined
components. The independent `Brep::try_merge_all_edges(angle, tolerance)` kernel
operation is also available. Explicit `try_join_edge_pairs` remains a
geometry-preserving assembly primitive; untouched Join components are not cleaned.
This does not yet add an interactive MergeAllEdges command.

## Geometry and topology guarantees

A removable vertex must have exactly two incident edge ends, with distinct,
non-closed edges. Every use of the first edge must have a unique consecutive
partner in the same trim loop. Branches, incompatible incidence and vertices
used by singular trims remain intact. Limiting tangents, including stationary
endpoints, use a stable `atan2(cross_length, dot)` angle test. Genuine tiny kinks
are not erased by a cosine that rounds to one.

Concatenation may clamp, raise degree, reconcile weight gauges, or shift parameter
domains. Such floating-point operations only propose a curve. Exact rational
Bernstein certificates compare each original piece with its oriented restriction
of the proposal. Accumulated spatial displacement cannot exceed the modeling
absolute tolerance. UV pieces require **zero** displacement, even when their
domains, degrees or parameter speeds differ: a small UV error cannot silently
be treated as a small model-space error on an arbitrary surface. Surfaces and
vertex positions are unchanged.

Exact surface-isocurve certificates recompute the new edge's uncertainty where
possible, retaining the maximum original edge tolerance. Otherwise uncertainty
propagates outward, including the source modeling floor for nonzero changes and
the removed vertex's uncertainty. No sampled comparison accepts a curve change.
The result still passes ordinary B-rep validation, whose general trim-image
correspondence checks retain their existing sampling limitations.

Linked trim rings update both uses of seams, loop heads, reversed uses, and
two-edge closed loops without scanning every face at each merge. Surviving
source edges precede appended merged edges; remaining vertices keep source
order. Curve copying and certificates share Join's sixteen-million-unit budget;
dense degree conversion is charged cubically before allocation. Budget or final
validation failures leave the source unchanged. This is not a linear-time curve
concatenation algorithm or a performance-parity claim.

Uncertifiable merges remain separate. Current limits include degree 16,
sign-coherent rational certificates, exactly preservable UV concatenation, and
representable parameter/weight ranges. New full-order joins that the current
OpenNURBS B-rep codec cannot encode are not introduced. A richer composite-edge
representation and general surface-image certificates remain future work.

## Angular-tolerance evidence

Four eight-case batches use the **same sixteen exported and roundtrip-checked
3DM sources** in private-Xvfb Rhino 8.32.26160.13001 sessions. Each source pair
has a `0.0005` normal gap, document absolute tolerance `0.001`, and pre-existing
boundary cuts at `0.125`, `0.375`, and `0.875`. Only the document angular tolerance
changes. Polynomial, rational, unclamped and multispan profiles are each tested
in both UV orientations.

| Profile / UV orientation | Rhino edges at `1e-10` and `1e-8` rad | Rhino edges at `1e-6` rad and 1° | Native edges at all four angles |
| --- | ---: | ---: | ---: |
| Quadratic / either | 7 | 7 | 7 |
| Rational cubic / either | 9 | 7 | 7 |
| Unclamped / 0 | 10 | 7 | 7 |
| Unclamped / 1 | 8 | 7 | 7 |
| Multispan / 0 | 8 | 7 | 7 |
| Multispan / 1 | 9 | 7 | 7 |

Requests and raw observations are saved for
[tight](../tools/rhino_oracle/fixtures/join_edge_cleanup_tight.json),
[roundoff-scale](../tools/rhino_oracle/fixtures/join_edge_cleanup_roundoff.json),
[resolved](../tools/rhino_oracle/fixtures/join_edge_cleanup_resolved.json), and
[one-degree](../tools/rhino_oracle/fixtures/join_edge_cleanup_degree.json) angles.
The public OpenNURBS
[`CombineContiguousEdges` implementation](https://github.com/mcneel/opennurbs/blob/23fc677ba06e49212296ca75fab7fb6c2851b4ce/opennurbs_brep.cpp#L10744)
uses unit-tangent dot products and cosine. Both tiny angles have binary64 cosine
one. The observed transition is consistent with that numerical mechanism; it
does **not** establish which private implementation Rhino Join invokes. Native
does not reproduce representation-dependent floating-point threshold artifacts.
An [independent 100-digit evaluation](../tools/rhino_oracle/references/join_edge_cleanup_angles.py)
of the stored clamped source pieces finds
a maximum junction angle below `1.90e-15` radians and endpoint gap below
`7.12e-15` model units. These numerical reference values, included in the report,
are not formal enclosures; acceptance still uses the kernel's curve certificates.

Six of eight records fully match at each resolved angle. The two unclamped
records retain only the known redundant outer-knot interchange differences.
At each tiny angle only the two quadratic records fully match; the other six
retain explicit topology differences. No coalescing, reordering, area correction
or outer-knot substitution is hidden in the full comparison.

Across **949 comparable surface-command cases**, **679 fully match**, **32 retain
native errors**, and **238 have other differences**. The earlier 917 cases gain
twelve full matches with none lost. Every raw field of the other 837 earlier
native outcomes is unchanged, excluding timing; only the eighty pre-split cases
change. All eighty now have one mated seam, seven edges and six vertices, while
their independent surface-area and half-gap uncertainty checks remain valid.
The report records source, fixture, executable and artifact hashes and all
pass/fail transitions. The 31 previously unobserved translated-isocurve cases
remain native-only evidence, excluded from comparison totals.

## Verification

Kernel tests cover box branches, cylindrical seams and caps, closed trim rings,
all sixteen reversal combinations of four pieces, tiny genuine kinks, singular
trim vertices, mixed degrees, extreme/signed weight gauges, unrepresentable
shifted domains, original uncertainty floors, exact UV rejection and work limits.
A separate 3DM test roundtrips every coalesced cylindrical edge and trim. Existing
command tests retain independent high-precision area references and raw Rhino
records; the new angular fixtures additionally check full records where they match.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle join_command
cargo test --release -p viboceros-io coalesced_rational_brep
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_edge_cleanup_resolved.json --observations tools/rhino_oracle/observations/join_edge_cleanup_resolved.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

Replay intentionally exits 1 for the two recorded outer-knot differences.
Generic curved partial overlap, ordered corner clustering, nonisoparametric
surface-image uncertainty and complete Rhino topology policy remain unfinished.

Verification passed 2,892 Rust tests (22 ignored in the regular run), all seven
opt-in offscreen GPU tests, 232 Python tests, formatting, and Clippy/Rustdoc with
warnings denied. README remains 43 lines.
