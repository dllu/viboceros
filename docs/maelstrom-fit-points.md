# Maelstrom FitPoints oracle workflows

[First-circle input](maelstrom-circle-input.md) · [Circle fitting](circle-fit.md) · [Plane orientation](plane-fit-basis.md)

17 completed commands were captured on Rhino **8.32.26160.13001** using an
owned private Xvfb display and settings scheme. The
[provenance record](maelstrom-fit-points-provenance.json) hashes the bounded
recipes, helper and unmodified native observations.

In Viboceros, choose `FitPoints` at the Maelstrom center prompt. Select points
or visible grips and press Enter. `SelID` accepts a point object's ID.
Three or more preselected point targets fit immediately. The circle previews
as fitting points are picked. After accepting it, the radius and angle getters
use its frame and the original deformation targets.

## Definition points and target selection

14 recipes preselect eight line targets and supply a separate set of point
objects to `FitPoints`. Three more preselect point targets: the same points
define the fitted circle and are deformed. The latter workflow immediately
accepts those points after `_FitPoints`, without another selection or Enter.
Each command uses `Copy=No`, `Rigid=No`, second radius `5` and coil angle `90`.

| Phase | Line targets | Separate definition points | Preselected point targets |
| --- | --- | --- | --- |
| Before | Selected | Unselected | Selected |
| Maelstrom ends | Unselected | Selected | Selected |
| Undo | Unselected; original geometry | Unselected; unchanged geometry | Selected; original geometry |
| Redo | Unselected; deformed geometry | Unselected; unchanged geometry | Unselected; deformed geometry |

Object snapshots record target/definition identities. Geometry replacement
changes runtime ordering, so comparisons use those identities. Separate
definition points remain unchanged through the command, Undo and Redo. Each
workflow ends with the same set of owned objects and creates no copies.

## Fitted frame and deformation

After each workflow, the original definition points are used by the separate
`Circle FitPoints` command. Its center, radius, X/Y axes and normal exactly
equal the public SDK `Circle.TryFitCircleToPoints` output. Maelstrom target
coordinates also exactly equal public `MaelstromSpaceMorph` maps built from
that SDK circle: **138 point or curve-start witnesses and 112 curve ends**.
Curve interiors and full curve approximation are outside this capture set.

A translated planar point set is repeated with CPlane normals `+Z`, `-Z` and
`(1,2,3)`. Those three commands produce identical fitted circles and maps.
The fitted frame determines the deformation; changing the CPlane does not
replace it in these captures. Reversing a fitted normal changes the signed
coil direction, so an unoriented circle-locus match is insufficient for input
integration. The Rust regression replays all 250 maps with the **captured
native frame**, independently of our unresolved plane orientation.

## Retained command-driving diagnostic

The initial 14 recipes incorrectly preselected point targets while trying to
select separate fitting points. Rhino had already accepted the target points
as the definition and advanced to the second radius. Subsequent `_SelID`
commands altered selection there; Enter ended Maelstrom before the supplied
radius and angle. `_Copy=_No` then became an unknown standalone command.

Those original helper, recipe and response files remain under
`maelstrom_fit_points_driving_diagnostic*`. All 14 EndCommand results say
`Success`, but no history contains `Morphed` and all target geometry is
unchanged. They are unsuccessful command-driving evidence and are excluded
from the 17 completed workflows and 250 map comparisons. The archived helper
uses its original schema and is not the current executable recipe factory.
Regression checks require actual deformation history in addition to the public
command result.

## Verification and remaining work

Headless app tests exercise viewport picks, preview, cancellation, numeric
placement and command-end selection, Undo and Redo for all 17 retained cases.
The fitted center and radius match within `1e-7`. The sampled deformation
matches six cases within `2e-5`. The opposite normal reverses the coil in
the other eleven cases, with witness distances up to about 19.799 model units.
Preview and commit share the local fitted orientation.

Private Rhino probes also show that pressing Enter with fewer than three
fitting points ends the command without a placement. With collinear or
coincident points, Rhino advances with a zero first radius and accepts an
identity deformation. The app covers this case, including repeated Copy
placements in one Undo entry.

```sh
cargo test --release -p viboceros app::tests::maelstrom::fit_points
cargo test --release -p viboceros-geometry maelstrom_fit_points
python3 -m unittest tools.rhino_oracle.test_maelstrom_fit_points
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/maelstrom_fit_points.json \
  --scheme VibocerosOracleMaelstromFitPoints \
  --output /tmp/maelstrom-fit-points.json
```

Live probes require one iteration, an empty owned document, a private settings
scheme and their own Xvfb display. The worker restores the construction plane
and removes its owned objects. No proprietary source was inspected.

Native signed plane orientation, the near-collinear convergence gap and
broader numerical coverage remain open.
