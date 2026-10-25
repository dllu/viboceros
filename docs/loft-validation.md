# Loft validation

[Loft usage and architecture](loft.md)

## Section preparation

The ordered section-preparation regression corpus compares 46 native Rhino 8.32
lofts, including shifted domains, relocated seams, aspect changes, altered inner
knots, and open smooth splines. Each case checks control counts, all control
coordinates and weights, both full knot vectors, and 289 unrounded surface points.
45 cases match within `1e-9`; one symmetric case remains an explicit ignored test:
`loft-domain-seam1-1-compatible-preparation-matching-affine-0-g2`. Its coarse arc
lengths differ by one binary64 bit, selecting another boundary at a near tie and
producing a substantial surface difference. This is an incomplete native
correspondence policy, not a claim of complete Rhino parity.

```sh
cargo test -p viboceros-geometry --test loft_domains_native
cargo test -p viboceros-geometry --test loft_domains_native -- --ignored --nocapture
```

The second command currently reproduces that known difference. Portable geometry
tests separately check extreme model units, source purity, the common corner
chart, backend errors, and the integration budget across spans.

[Source and capture bindings](loft-section-preparation.json) retain the exact
comparison scope and known difference.

## Established fixtures

Permanent fixtures contain 34 API loft cases, 33 command cases, and four
same-file 3DM interchange cases. All 67 API/command fixtures now also request
289 unrounded surface points on a 17-by-17 normalized-UV grid per output face,
in addition to coefficients and command state. Exploratory comparisons cover 90 API cases:
two through six sections, all five styles, closed loops, rational circles,
mixed degrees and knots, polylines, common weight scales, and unequal endpoint
weights. On the tested Rhino 8.32 build, all 90 API and 33 command cases match at
`1e-9 + 1e-12 * max(|a|, |b|)`; largest numeric differences are respectively
`1.25e-13` and `7.11e-15`. Command records include face directions, validity,
edge/vertex counts, and document state. Faces are compared in UV-domain order,
not implementation-specific allocation order.

`loft_end_weights.json` adds equal-weight large-coordinate and ordinary
unequal-weight ruled profiles. `loft_end_weights_diagnostics.json` retains three
near-equal-weight cases, **not passing references** at the ordinary `1e-9`
absolute / `1e-12` relative bound. In the public Rhino 8 probe, changing an end
weight from `1` to `1 ± 1e-8` moves a supplied endpoint from `6e8` to
`6e8 ± 3`, and changes the second profile's endpoint heights by `1.5`.
The unit-scale case has the same `3e-8` endpoint displacement. Native tests
independently compare all 289 ruled-surface points to the original profiles
under the analytic Möbius parameter map and retain their exact corner locations.
The diagnostic is not merely a difference in surface parameter speed.

The `sample_geometry=true` oracle field enables these additional points for both
API and command operations, in U-major/V-minor order. Geometry-API sampling stays
outside the construction timer; command timings still include result recording.
A fresh 72-case run of the expanded fixtures passes all 67 existing cases and the
two new passing cases at `1e-9 + 1e-12 * max(|a|, |b|)`; the three deliberate
diagnostics remain visible. Grid samples are bounded checks, not continuous
surface-error certificates.
The 67 existing API/command cases also pass a separate fresh run at their default
document tolerances, with maximum absolute error `1.25e-13`.

A combined live run of the 34 permanent API cases, all 33 commands, four loft
3DM cases, and eight existing morphed 3DM cases also passes the stricter
`2e-12 + 1e-14 * max(|a|, |b|)` bound. Both readers validate and mesh the same
exported loft B-reps. Periodic 3DM records canonicalize only the two outer knot
entries that OpenNURBS does not store; all stored coefficients remain checked.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft_3dm_interchange.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
```
