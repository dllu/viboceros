# Volume display units

[Volume](commands/volume.md) · [Earlier scalar audit](volume-command-audit.md)

`Volume` now offers all fourteen choices observed in Rhino's unit menu, including
`ModelUnits` and `Liter`. Choices are accepted while selecting, independently of
the later open-boundary warning. The submenu returns to selection; it never
implicitly answers that warning. Enter with no eligible selection ends the
query, retaining its accepted display choice. Preselection uses the remembered
choice without opening the unit menu. McNeel's [command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/volume.htm)
documents the postselection-only option; its exact menu and persistence behavior
below were observed through real commands, not inferred from a mass-properties API.

## Evidence

The [source generator](../tools/rhino_oracle/references/volume_display_units.py)
produces thirty operations, captured in one fresh owned private-Xvfb Rhino
8.32.26160.13001 session. Existing user sessions were untouched. Each operation
sets an explicit initial display choice through a real `_Volume _Units ... _Enter`
with no selected objects. Its EndCommand event reports failure and its history
records the accepted choice. The measured command then runs with preselection
or actual `_SelID` postselection. Source vertices/faces are checked unchanged
after insertion and after measurement; scalar commands must create no objects.

The [complete capture](../tools/rhino_oracle/observations/volume_display_units.json)
covers every menu choice, millimeter/meter/inch/unitless model metadata, signed
volume, last-choice persistence, and all Yes/No/Escape open-boundary answers.
The ordinary histories retain printed uncertainty and exact unit labels. API
volumes are separate diagnostics, never substituted for command results.

The [native replay](volume-display-units-comparison.json) matches **30/30**
at fixed absolute `1e-9`, relative zero. The maximum difference is `3.71e-11`,
from Rhino printing one cubic foot as `0.037037037` cubic yards. No tolerance was
changed, no unit label is ignored when parsing observations, and no observed
number is passed to the native runner. The [independent rational witnesses](../tools/rhino_oracle/fixtures/volume_display_units_reference.json)
derive mesh volumes from exact determinants and nominal SI/imperial definitions
using Python `Fraction`; native results equal their rounded binary64 values.
See [capture hashes](volume-display-units-provenance.json).

## Numerical and state boundaries

`VolumeMassProperties::signed_volume_from_boundaries_in_units` evaluates only
the scalar boundary integral. It multiplies the accumulated exact scalar by
the cube of an exact unit ratio before its first binary64 conversion. Standard
unit definitions are nominal decimal/rational values (inch = 127/5000 meter),
not an already-rounded binary64 conversion factor. Custom source/target unit
scales preserve their stored binary64 values. Surface quadrature remains
numerical; dimensional conversion does not make that integral exact.

This avoids both premature volume range loss and an overflowing/underflowing
conversion cube. Independent tests use tetrahedra at powers of two from
`2^-1000` through `2^1000`, whose converted volumes are exactly ±10. Tiny facets
are constructed with numerical mesh validation, not a minimum modeling feature
size. Final overflow still errors; final underflow may round to zero, as with the
unconverted scalar. Unitless sources use factor one, matching the capture rather
than assigning them an invented physical scale. `ModelUnits` needs no physical
conversion.

Preferences live in a command registry, outside model undo/redo; independent
registries are isolated. Captures establish persistence in one Rhino document,
including after changes to its model-unit metadata. Persistence across new Rhino
documents or application restarts was not tested or claimed. Native UI tests
separately check submenu phases, empty Enter, selection cleanup, unchanged
geometry/units/tolerances, and real Undo/Redo. Explicit headless `Volume Units=...`
is supported even on a preselected set; this is not a claim that Rhino offers
that interactive option during preselection.

The earlier three scalar discrepancies remain: two curved printed-volume
residuals and inward B-rep document-insertion reversal. This work does not change
those observations, the default geometry integral, centroid conventions, SubD
support, or Rhino performance-parity status.
Regression replays remain 36/36 surface-centroid, 42/42 warning-centroid, 26/26
closed-centroid, and 77/80 scalar/centroid commands, with no new differences.

```sh
python3 -m tools.rhino_oracle.volume_command_replay \
  tools/rhino_oracle/fixtures/volume_display_units.json \
  tools/rhino_oracle/observations/volume_display_units.json
python3 -m unittest tools.rhino_oracle.test_volume_display_units
```
