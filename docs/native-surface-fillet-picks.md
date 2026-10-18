# Fillet UV picks and construction accuracy

`SurfaceFilletOptions::try_from_picks` prepares normal offset sides from UV
picks in each original surface domain:

```rust
let options = SurfaceFilletOptions::try_from_picks(
    &first, &second, [[u0, v0], [u1, v1]], radius, false, tolerance,
)?;
let result = fillet_surfaces(&first, &second, options, tolerance)?;
```

Both surfaces reuse the producer's conditioned spatial frame, weight gauge and
unit parameter charts. For each surface, the other surface's picked point is
projected onto its bounded domain. The normal there determines the signed
offset. This matters near a curved surface boundary: a point at negative X
beside a first-quadrant cylinder selects its inner side. Extending an imaginary
complete cylinder would choose a different side. Reversing either parameter
direction or swapping U/V reverses the signed normal consistently.

Invalid UVs, singular projection normals and signed distances within four local
query tolerances are errors. The ambiguity filter has a normalized `1e-12`
floor; it does not enlarge the construction or comparison tolerances. The API
selects signed offsets. Choosing among disconnected fillet components, trimming
parents, joining and the complete FilletSrf command workflow remain in progress.

The native oracle previously retained picks in its request but always used
positive normal offsets unless an explicit diagnostic override was present.
It now calls this API by default. Explicit `sides: [-1, 1]` remains available;
an explicit null is rejected by both request validators.

An independent analytic torus projection exposed a second issue. Although the
fillet's positional residual was small, its fitted normals moved some
off-surface closest points by about `4.19e-8` at a `1e-8` model tolerance.
Tightening only the later closest-point query did not remove the error. The
private native producer now constructs at one sixteenth of the requested
accuracy, bounded just above the existing native accuracy floor. Input
acceptance, exported correspondence checks and Rhino comparisons keep their
original tolerances. The controlled four-query diagnostic improves to about
`6.37e-9` against the analytic torus. This reserve is empirical and does not
provide a global normal-error certificate.

Sixteen fresh public SDK recipes exercise plane quadrants, inner/outer cylinder
picks, bounded boundary projections, reversed U/V, swapped U/V, rational
parameter changes and a reversed first plane. The complete raw SDK definitions,
topology, areas and query results are retained. Qualification checks 81 points
from each surface against the other surface, all returned closest points against
the other surface, query distances and tight bounds at `1e-8`. Literal
closest-point coordinates can differ by more than that epsilon: one saved SDK
answer is `3.1e-8` from the independent analytic torus projection even though the
sampled surface images agree within `5e-10`. This tangential discrepancy does not
establish whether the SDK's fitted normals or its query stopping rule caused it.
All sixteen recipes pass the sampled image checks; the largest bidirectional
surface gap is `4.63e-10`. Literal closest-point coordinates pass in eleven of
sixteen recipes, with a maximum difference of `2.71e-8`. These strict coordinate
differences remain in the raw audit. Full NURBS protocol equality and continuous
global Rhino agreement are not claimed. The [validation record](native-surface-fillet-picks-validation.json)
contains the old executable comparison, final test logs, ownership checks,
precision diagnostic and unnormalized audit reports.

Curved `Extend=Yes` still has a known extent mismatch. Separate fresh SDK probes
show that degree elevation preserves the observed endpoints, while knot
insertion changes them even when an independent coefficient bound establishes
the same input parameterization within `6.4e-14` in the decimal JSON recipes.
An affine chart gives the same result; a chart starting at `1e12` returns no
fillet. Changing the requested tolerance can also change the extension shape.
All these outcomes, including the empty result, are retained. A native G2
continuation experiment returned successful statuses but failed independent
radius identities, so it was rejected. No new curved extension policy is
enabled by this change.

The direct C ABI probe now evaluates its stored endpoints exactly. Its previous
interpolation formula rounded one upper endpoint one ULP outside the valid
domain. The native rejection was correct; no parameter clamping was added.
The normal and AddressSanitizer probes retain the original `1e-8` geometry
checks. Leak detection is disabled. The existing producer's conditioning,
resource and general-performance limitations continue to apply.

```sh
cargo test --release -p viboceros-smlib --features native --test surface_fillet_picks
cargo test --release -p viboceros-oracle --features native-smlib --lib surface_fillet
python3 -m unittest tools.rhino_oracle.test_surface_fillet_probe
```
