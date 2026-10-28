# Sweep1 section rebuilding

[Command guide](../sweep1.md) · [Construction](construction.md)

`Sweep1 Rebuild=5` rebuilds real cross-sections before common-basis preparation
and point-pole creation. Original objects, rail stations, directions and seams
are retained. `RebuildCount` and `RebuildPointCount` are aliases. The interactive
picker accepts these value getters; local Undo revises the option.
`Simplify=Rebuild` chooses five points when rebuilding is disabled;
`Simplify=None` or `Rebuild=0` disables it. Rail refitting remains independent.

Commands accept counts from 3 through 512. The SDK path (`command=false` in JSON)
additionally accepts one and two: zero/one preserve the source, and two rebuild open sections to lines
while retaining closed sections. JSON requests use `rebuild_point_count`;
`command=true` applies the command count rules. This distinction follows the observed SDK and
scripted command behavior. Closed sections otherwise get one extra control for
the repeated seam. Degree is `min(3, count - 1)`; rebuilt weights are one.
The 512-control limit includes seams and subsequent common-basis matching.

`Sweep1::rebuild_sections` preserves supplied stations. Its backend variant
`rebuild_sections_with` delegates the clamped arc-length rebuild, sharing
`try_rebuild_nurbs_sections_with` with Loft. Native builds use usd-brep/SMLib
basis evaluation and solving. Layout preflight borrows profiles and checks every
requested layout before calling the numerical backend.

## Validation

[Recorded checks](rebuild-validation.json) bind sixteen SDK outputs, twelve
actual commands, and four command holdouts covering point ends and a retained
curved rail. Portable and native SMLib API/command paths compare all 32 complete
surface images at epsilon `1e-7`, equal to construction tolerance. Extra knots
and U reversal are permitted when they preserve the image.

The certificate decomposes positive surface weights into separable ideal tensors,
bounds the complete weight perturbation with exact rational arithmetic, and
certifies every full profile row under one shared monotone V correspondence.
Common U bases are checked exactly from stored binary knots and domains. Searches
only propose the map; exact curve bounds validate all intervals and knot limits.
This bounds symmetric image distance, including collapsed pole rows. Separate
regressions check interior-row changes, weight coupling, refinement and gauge.

Rebuilding intentionally approximates the input profiles according to point
count. The comparison above measures agreement with Rhino's rebuilt output, not
error against the original curves. Existing Global blending, general curved-rail
refit, rail miters and seam/alignment limitations remain. Section refitting by
tolerance is still pending.
