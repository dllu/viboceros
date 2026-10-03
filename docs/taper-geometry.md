# Taper deformation kernel

[Architecture](architecture.md) · [Rhino oracle](oracle.md)

`TaperPointMorph` is an independent Rust implementation of the public
[TaperSpaceMorph constructor](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.morphs.taperspacemorph/taperspacemorph).
It accepts an axis, start/end radii, Flat and Infinite choices. The
[Taper command](commands/taper.md) adds distance picks, construction-plane Flat
direction, atomic document edits and interactive repeated copies.

## Point map

Let `t` be the axial coordinate divided by the axis length. The radius scale is
`1 + (end_radius / start_radius - 1) * blend(t)`.

* Infinite uses `blend(t) = t`, including outside the axis endpoints.
* Finite uses zero before the start, one after the end, and cubic smoothstep
  `t²(3 - 2t)` between them.
* Radial mode scales both perpendicular coordinates and retains axial position.
* Flat scales only the perpendicular plane's X coordinate. SDK construction
  uses the deterministic OpenNURBS-style plane normal basis already implemented
  by `Frame3`. An explicit frame constructor supplies a different flat direction.

An Infinite taper can reach zero scale and then reverse the perpendicular
coordinates. It does not clamp the extrapolated scale to positive values.
Consequently, fitted geometry or meshes crossing a singular plane may fail
their own validation even when the point map is defined.

Public SDK construction requires both radii and axis length to exceed `2^-32`.
Signed/zero radii and shorter axes are invalid. The kernel reports construction
errors; the matching oracle protocol returns unchanged points for an invalid
definition, as the measured SDK does. `try_from_frame` instead accepts positive
finite mathematical lengths and radii below the SDK cutoff. The command frame
constructor accepts signed distances above the native absolute zero cutoff;
this is a separate contract from the positive-only public SDK constructor.

The ordinary map caches the radius change and uses floating-point projections.
Overflowing ratios/displacements, underflowing blend products and severe
cancellation use exact rational evaluation with one final rounding per world
coordinate. This preserves representable images across extreme scales; an
unrepresentable image returns an error. Equal radii preserve points exactly.

## Geometry and rigid placement

The morph implements the existing `PointMorph` contract. Curve and surface
fitting uses the shared bounded, tolerance-driven fitters. PreserveStructure
moves existing Euclidean controls while retaining degrees, knots and weights;
it is an approximation, not a pointwise error certificate. Multi-face B-reps
use fitting regardless of PreserveStructure.

Rigid placement maps a supplied object/group bounds center and builds a rotation
from the measured forward world-axis derivative policy shared with Twist and
attenuated Bend. One grouped native point command constrains this pose at `1e-7`.
Follow-on [command captures](commands/taper.md) constrain rigid curves, surfaces,
boxes and meshes, along with group and history rules. Singular rigid frames
remain limited by the shared pose validation.

## Evidence and limits

Licensed Rhino **8.32.26160.13001** ran entirely on private Xvfb. There are 51
public SDK cases and six owned actual point commands, covering radial/flat maps,
finite/infinite axes, translated and reversed spatial axes, small and signed
radii, exact validity boundaries, scale crossings, Copy, grouped Rigid and
Undo/Redo. Raw records and [hashes](taper-provenance.json) retain this evidence.
Ordinary point coordinates use component epsilon `1e-11`; large radius ratios
also use relative epsilon `1e-12`. Rigid points use `1e-7`.

An initial six-command capture is retained as a diagnostic: it ran inside
`RunPythonScript`, so Undo did not restore the models. Corrected commands use an
idle callback and omit trailing commands after completion. Their terminal
geometry matches the diagnostic, and Undo/Redo now restores the expected states.
Two earlier helper compatibility failures produced no successful captures.

The fitted-line and preserved-control tests check kernel contracts separately
from native fitting. Follow-on [command evidence](taper-command-provenance.json)
adds native curve/surface/B-rep samples, flat point picks, construction planes,
preferences and repeated targets. Live previews remain pending. These witnesses
do not establish exhaustive Taper or Rhino parity.

A release Rust point-batch run (1,000 iterations) measured 4.35–319.88 ns per
point across 20 valid primary cases, with median 36.16 ns. Timings include result
collection and exclude construction, serialization, fitting and UI work. They
are not a native Rhino kernel comparison; the licensed oracle runs through
Python, Wine and FEX.

Verification: five geometry tests and the bounded protocol test passed, along
with release protocol replay of all 51 SDK cases and 26 related Python tests.
Formatting and geometry/oracle all-target Clippy passed with existing warnings.

```sh
cargo test -p viboceros-geometry taper
cargo test -p viboceros-oracle taper
python3 -m unittest tools.rhino_oracle.test_taper
```
