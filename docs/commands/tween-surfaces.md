# TweenSurfaces

[Command reference](README.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tweensurfaces.htm)

`TweenSurfaces` adds intermediate surfaces between two inputs. The current
implementation supports sampled matching (`MatchMethod=SamplePoints`) for
polynomial/rational surfaces with different degrees, counts and parameter domains.
Control matching (`MatchMethod=None`) handles equal-sized polynomial nets and
compatible positive rational nets. Sources remain unchanged.
Select two single-face surfaces and confirm, or start the command and select
sources with Enter confirmation.

```text
TweenSurfaces NumberOfSurfaces=3 MatchMethod=SamplePoints SampleNumber=6
TweenSurfaces NumberOfSurfaces=3 MatchMethod=None
TweenSurfaces Sources=<first-id>,<second-id> OutputLayer=StartSrf
TweenSurfaces Sources=<first-id>,<second-id> FlipEndU=Yes SwapEndUV=Yes
```

Explicit `Sources=` fixes script order. `FlipStartU`, `FlipStartV`, `SwapStartUV`,
and their `End` counterparts adjust correspondence without changing originals.
Swap applies before flips. The viewport controller collects two surfaces;
native ordered single picks and corner-preview editing remain unfinished.

The initial matching default is `SamplePoints` with `SampleNumber=10`, following
the owned fresh native settings. `SampleNumber` specifies divisions per UV axis;
there are one more stations than divisions. Both sources are evaluated at matching
normalized UV positions, blended, then interpolated as a non-rational tensor
surface with degree up to three. Knot placement uses mean chord distances on the
blended grid. Domains follow those physical chord sums and can differ between
outputs. This interpolates the grid; it does not certify a continuous exact
blend of the source surfaces. Sampling is bounded to `2..=255` divisions per axis,
matching the existing 256-station dense tensor solve limit. Source trims are not
sampled. Native sample counts above this local limit remain unsupported.

Counts default to one and are bounded at 4,096, with an aggregate million-control
limit. Outputs exclude sources, at fractions `i / (number + 1)`. Polynomial
preparation elevates degrees and unifies knots; output domains count common
spans. Compatible rational matching retains the first weights and scales each
control displacement by `sqrt(end_weight / start_weight)`, following measured
native data. This can extrapolate control locations. Unequal rational degrees
or counts remain rejected.

`OutputLayer=CurrentLayer` uses fresh default attributes and no groups.
`StartSrf` and `EndSrf` copy the corresponding attributes and group memberships.
Geometry-root text is not copied. Sources and outputs finish unselected.
One Undo removes every output; Redo restores the accepted result. Construction
is staged before insertion, and failures preserve geometry and history.

The [complete capture](../../tools/rhino_oracle/observations/tween_surfaces_command.json)
contains 22 successful owned Rhino recipes on private Xvfb under
`VibocerosOracleTweenSurfacesVerified20261008`. It retains full NURBS definitions,
81 stations per surface, properties/groups, source identity, command events
and independent Undo/Redo, plus public sampling SDK results. Eight compatible
control recipes replay degree/counts, knots, controls and weights at `1e-6` for
positions and `1e-12` for knots/weights: planes, quadratic/rational nets, different
domains, swapped rational order and output layers. Independent regressions
check extreme finite coordinates, source purity, resource rejection and history.
See [provenance](../tween-surfaces-provenance.json).

Captured `Refit` commands leave four additional source-shaped copies, and
`SamplePoints` two, alongside requested tweens. The public sampling SDK returns
just the requested outputs. Both raw records and the initial investigation are
retained without normalizing these differences. Unequal-net control matching,
`Refit`, option memory, corner previews and performance parity remain unfinished.
The registered command rejects unsupported methods. Trimmed single-face input
uses its underlying surface; trim correspondence is not implemented.

The [sampling follow-up](../../tools/rhino_oracle/observations/tween_surfaces_sampling.json)
adds 20 successful owned commands under `VibocerosOracleTweenSampling20261008`
with sample counts 2, 3 and 6. All requested command surfaces exactly equal the
public sampling SDK definitions; this run keeps the matching mode active and
produces no extra source-shaped copies. Kernel replay checks all 20 output nets
at `1e-7` for controls and `1e-8` for knots, plus all 22 earlier SDK outputs.
Command replay compares 81 UV-normalized witnesses per source/output at `1e-7`,
identity, metadata/groups and independent history. Cases cover planes, warped
and rational tensors, unequal counts/degrees/domains and source output layers.
The app regression checks that method, sample count and output count survive
command-first selection and Undo/Redo. Independent kernel checks require
analytic plane agreement at `1e-12` and reject excessive work before allocation.
See [sampling provenance](../tween-sampling-provenance.json).

The JSON/Python oracle accepts `surface_tween_sampled_geometry` with
`start_surface`, `end_surface`, `number` and `sample_number`. It returns full
`surfaces` definitions in both engines and shares the same resource limits.

```sh
cargo test --release -p viboceros-geometry surface_tween
cargo test --release -p viboceros-command tween_surfaces
cargo test --release --bin viboceros tween_surfaces
python3 -m unittest tools.rhino_oracle.test_tween_surfaces tools.rhino_oracle.test_tween_surfaces_sampling
```
