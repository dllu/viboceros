# Large-tolerance section fitting

[Section fitting](section-fitting-api.md) · [Validation](refit-large-tolerances-validation.json)

Refit now admits certified endpoint-tangent cubics when tolerance reaches or
exceeds the endpoint chord. A failed whole-profile proposal subdivides under the
existing budgets; common-handle correction retains its smaller-tolerance path.
Every fitting pass still requires a complete source-image certificate.

Backward-pointing endpoint tangents use handle length
`chord * (2 - max(0, cosine)²) / 3`. Nearly opposite endpoint directions require
subdivision even when a whole cubic meets a large positional tolerance. Captures
bracket the opposition cutoff around cosine `-0.9999`. Ordered floating products
retain the observed batch boundary; one single-profile boundary case still has
a certified geometric gap. The comparison epsilon remains `1e-7`.

For closed profiles whose control-box diagonal fits within tolerance, proposals
retain natural spans and their global length lookup. Joining those spans avoids
collapsing a closed loop during subsequent Hermite reconstruction. Fine closed
fitting retains its existing subdivision policy.

Nine immutable public Rhino captures contain 196 contexts: six smooth source
families, circles, asymmetric closed curves, spatial and tall profiles, mixed
companions, affine domains, weight gauges, and endpoint-frame boundaries. Inputs
and authored stations are preserved within each engine. Tests compare complete XYZ curve images;
exact projected minimum-distance bounds certify disagreements in either direction.
Separate complete-curve comparisons cover the twelve newly admitted open Loft
pairs without changing their historical baseline fixture.

| Captured contexts | Constructed | Complete native images proved |
| --- | ---: | ---: |
| Large tolerances and holdouts | 66/66 | 66/66 |
| Endpoint frames and boundaries | 58/58 | 57/58 |
| Independent fine and coarse profiles | 16/16 | 4/16 |
| Public-frame candidate re-entry | 42/42 | 23/42 |
| No-fit preparation controls | 14/14 | 14/14 |

The [re-entry checkpoint](refit-proposal-reentry-validation.json) compares original
profiles and independently authored cubics using public endpoint frames. These
are independent inputs, not established native fitting intermediates. The 42
re-entry cases retain 18 certified gaps and one inconclusive comparison; their
no-fit controls retain the authored parameter domains. Both backends agree on
all 56 complete output definitions. Production fitting is unchanged by this checkpoint.

Geometry agreement does not establish control-net or parameter-domain parity.
Some large paired native fits have 18 controls while an equivalent whole cubic
has four. The required fine-profile and opposition-boundary failures remain
active. General Rhino fitting compatibility is unfinished.

```sh
cargo test --frozen -p viboceros-geometry --test loft_refit_large_tolerances \
  --test sweep_refit_large_tolerances --test sweep_refit_independent_profiles \
  --test sweep_refit_proposal_reentry --no-fail-fast
cargo test --frozen -p viboceros-oracle --features native-smlib \
  --test sweep_refit_extended_sections
python3 -m unittest tools.rhino_oracle.test_sweep1_refit_extended_captures
```
