# Oriented compound pairs and common intersection

[Compound sets](compound-intersections.md) · [Polyhedral geometry](polyhedral-booleans.md)

The follow-up public SDK and command capture contains 54 closed recipes: eight
SDK signed-shell pairs and 46 command cases. All live work used a fresh private
Rhino settings scheme on private Xvfb, at idle in an empty owned document.
Original sources are built from explicit public boxes; no proprietary code is
inspected. The historical 41-recipe probe and capture remain unchanged.

## Two inward shells

| Finite enclosures | Measured command result |
| --- | --- |
| Proper boundary crossing | Union the enclosures, using first-set intermediate metadata. |
| Strict nesting | Copy the enclosing shell with its intermediate metadata. |
| Face contact | Union, retaining source face seams. |
| Edge contact | Union as one valid nonmanifold boundary; retain inward face winding. |
| Disjoint or point-only contact | No shell-pair result. |
| Equality | Fail the entire staged operation; preserve original objects and history. |

The public SDK can differ from the command: for example, its signed nested-pair
intersection is empty, while the command copies the enclosing shell if another
pair establishes an overall interaction. Closed inward union results are turned
outward by the command. An open result has no solid orientation and retains its
inward winding. The capture records `face_reversed` separately from polygon
loops so this behavior can be checked through an oriented boundary integral.

The outer-shell intersection and the inward-shell union remain separate objects
and can overlap. The pair cases also cover reversed input order, preselection,
retained inputs, and Undo/Redo.

## Common intersection

Two-input and multiple-input common intersection construct the odd/even material
intersection, then export its participating connected boundaries. Untouched
cavity shells can disappear. A participating inward solid cavity is inserted as
an outward object. A nonmanifold cavity boundary remains non-solid and retains
its inward winding.

Participation is resolved among original finite shells, including positive-length
boundary contact. Equality and isolated points do not establish participation.
Enclosing inputs with no participating boundary are ignored when choosing result
metadata. Attributes normally come from the first participating original; a
complete unchanged boundary takes attributes from its matching original input.
For one output, geometry user text comes from the last participating original.
Multiple outputs clear that text. Coplanar faces are merged across source owners
for common intersection, while two-set pairs retain their source seams.

Captured examples include:

- Crossing cavities: separate outward boundaries of volumes 15.625 and 1.875.
- Nested cavities: the untouched cavity disappears, leaving volume 15.625.
- Edge-contacting cavities: volume 15.625 plus a valid non-solid inward boundary.
- An enclosing third input: it contributes neither an active boundary nor result
  metadata, even when selected first.
- A cavity intersecting a positive box: one material result of volume 2, whereas
  the two-set command produces overlapping objects of volumes 2.25 and 3.75.

Common outputs remain unselected. Captured input retention, groups, user text,
selection and four Undo/Redo recipes are replayed independently of geometry.

## Boundary export and verification

`BrepPolyhedralBooleanPlan::export_boundary` exports separate connected
boundaries without promising a manifold material solid. It keeps inward
orientation and intentional nonmanifold edge contacts. The ordinary `export`
API retains its closed-solid and singularity certificates. Both paths reject
rounded point collapse and validate face trims, edge incidence and supporting
surface correspondence after the sole rounding step.

`export_boundary_with_faces` enforces complete source-face coverage. Boundary
components report canonical face owners, equivalent original boundary aliases,
and inputs with exactly the same complete unoriented boundary. Command policy
uses these exact reports without re-extracting rounded intermediate geometry.
The plan retains its cumulative work and output limits.
Original-input shells use certified unsplit polygons for exact classification;
material-side agreement is checked against every arrangement cell. This avoids
redundant classification of global fragments without increasing the work limit.
Generic edge coalescing still requires manifold input, so nonmanifold command
boundaries retain their validated edges after coplanar face merging.

Raw source geometry, SDK unions used only as diagnostics, SDK null/empty
responses, Native EndCommand and post-script records, failure status, history,
face winding and non-solid output records are retained in
[the capture](../tools/rhino_oracle/observations/compound_pairs.json).
See [provenance](compound-pairs-provenance.json) and
[recorded face partitions](compound-pairs-partitions.json).
All 46 command records replay physical geometry, metadata, selection and winding.
One partition difference remains explicit: `second_multi` has 12 faces and
30 edges locally, versus native 13 faces and 34 edges. The original 65-command
capture now replays all 65 records, including its valid non-solid two-hole
intersection. That path retains first-set geometry text and native topology counts.

Finite bidirectional boundary witnesses and boundary integrals do not establish
continuous error bounds, arbitrary curved Boolean parity, all nonmanifold input
operations, native component ordering or native kernel performance. Local error
messages do not reproduce every native Failure/Nothing classification. The
previous failure-marker discrepancy remains recorded separately.

```sh
cargo test --release -p viboceros-command boolean_solids::tests -- --nocapture
python3 -m unittest tools.rhino_oracle.test_compound_pairs

# Fresh private scheme and display for every live run.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/compound_pairs.json --scheme VibocerosOracleCompoundPairsExample --timeout 900 --output /tmp/compound-pairs-native.json
```
