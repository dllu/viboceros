# Automatic curved image fitting validation

[Fitting guide](geometry/surface-fitting-images.md) · [Machine-readable record](surface-image-auto-validation.json)

Automatic image fitting now tries monotone U/V correspondences on curved
candidates after ordinary parameter admission declines. Every returned fit
retains a certificate that rechecks the complete original surface image.
The separate UV fitter keeps its previous results and accuracy contract.

Both builds replay all 112 archived public Rhino fits, with 90 native surfaces
and 22 native nulls. Source-image accuracy uses the requested tolerance;
native output agreement and complete 3DM support round-trips retain `1e-9`.

| Outcome | Earlier quad-only fitter | With curved mappings |
| --- | ---: | ---: |
| Certified fits | 93 | 94 |
| Complete native image matches | 54 | 59 |
| Unproved native image comparisons | 27 | 22 |
| Local fit with native null | 12 | 13 |
| Native surface without local fit | 9 | 9 |
| Both decline | 10 | 9 |

The new results use 48 parameter, 19 planar quad and 27 mapped certificates.
All 94 source certificates recheck, all support round-trips pass, and all 30
planar fixtures still fit one patch. Twenty-six earlier fits use fewer controls.
The UV regression remains 87 certified fits and 35 native parameter matches.

Seven native comparisons gain complete image bounds. Two earlier quadratic
strip matches become unproved: geometric admission accepts three controls
before the earlier six-control UV fit. Their complete source-image bounds are
below `0.01`; agreement with the native outputs at `1e-9` remains unproved.
The record retains both gains and losses by case. Full native fitting policy
and the physical `FitSrf` command remain unfinished.

Knot-aware floating preflight preserves every output from the completed uniform
preflight replay. Samples only reject proposals; a rational peak missed by all
preflight stations is still rejected by the complete proof. Prepared curve
queries, bounded grids and streamed evaluation limit avoidable work, while
exact certification can remain expensive. No performance parity is claimed.

All work here is offline replay: no new Rhino process or desktop window.
The proof receipts retain frozen sources, final logs, historical producers,
and superseded build, schema and assertion failures separately.
