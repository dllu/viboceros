# TweenSurfaces

[Command reference](README.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tweensurfaces.htm)

`TweenSurfaces` adds intermediate surfaces between two inputs. The current
implementation handles control matching (`MatchMethod=None`) for equal-sized
polynomial nets and compatible positive rational nets. Sources remain unchanged.
Select two single-face surfaces and confirm, or start the command and select
sources with Enter confirmation.

```text
TweenSurfaces NumberOfSurfaces=3 MatchMethod=None
TweenSurfaces Sources=<first-id>,<second-id> OutputLayer=StartSrf
TweenSurfaces Sources=<first-id>,<second-id> FlipEndU=Yes SwapEndUV=Yes
```

Explicit `Sources=` fixes script order. `FlipStartU`, `FlipStartV`, `SwapStartUV`,
and their `End` counterparts adjust correspondence without changing originals.
Swap applies before flips. The viewport controller collects two surfaces;
native ordered single picks and corner-preview editing remain unfinished.

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
retained without normalizing these differences. Unequal-net common-chart fitting,
`Refit`, `SamplePoints`, native default matching, option memory, previews and
performance parity remain unfinished. The registered command currently defaults
to control matching and rejects unsupported methods. Trimmed single-face input
uses its underlying surface; trim correspondence is not implemented.

```sh
cargo test --release -p viboceros-geometry surface_tween
cargo test --release -p viboceros-command tween_surfaces
cargo test --release --bin viboceros tween_surfaces
python3 -m unittest tools.rhino_oracle.test_tween_surfaces
```
