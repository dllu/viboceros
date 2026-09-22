# Volume

[Command index](README.md) · [VolumeCentroid](volume-centroid.md) · [Mass integration](../mass-properties.md)

Select meshes, NURBS surfaces or B-reps and enter `Volume`, or enter it first,
pick objects and press Enter. The result is cumulative signed volume, initially
in cubed document-coordinate units. Reversed orientation subtracts; exact cancellation
reports zero. Mixed preselection skips curves and points. Directly selected
group subsets contribute only the selected members.

If any participating boundary is open, the same conditional warning as
`VolumeCentroid` asks whether to continue. Yes, Enter and Escape **at the warning**
continue; No declines. Escape during object picking and command replacement
cancel without calculating. Headless callers must explicitly pass
`Volume Continue=Yes` for open input. A closed selection needs no warning and
ignores a supplied continuation answer. The answer is not remembered.

The query creates no geometry and never adds or discards model undo/redo history.
Preselection remains selected; successful command-first queries and declined
postselection warnings clear selection. Unsupported-only preselection fails and
retains selection (unlike VolumeCentroid). Numerical errors produce no partial result.

Open collections describe volume only when they jointly enclose a consistently
oriented region. Neither the warning nor its acceptance certifies that condition.
For other open inputs the result is reference-dependent cone flux. Mesh warning
policy checks topological closure, separately from consistent winding.

## Display units

During command-first selection, enter `Units` to choose a display unit or type
`Units=Liter`, `Units=Meter`, etc. Choices are `ModelUnits`, `Micron`, `Millimeter`,
`Centimeter`, `Liter`, `Decimeter`, `Meter`, `Kilometer`, `Microinch`, `Mil`, `Inch`,
`Foot`, `Yard`, and `Mile`. A liter equals one cubic decimeter. `ModelUnits` follows
the document's current unit metadata. This does not rescale geometry, change
document units/tolerances, or add an undo entry.

An accepted choice persists in the command registry, including after Enter with
no eligible selection or cancellation. It applies to later preselected queries;
the interactive unit menu is only offered during postselection. Headless callers
may explicitly use `Volume Units=Meter [Continue=Yes]` on a selected set.
`VolumeCentroid` has no display-unit option: its marker stays in model coordinates.
See [the 30-case unit audit](../volume-display-units.md) for evidence and scope.

## Integration and limits

`VolumeMassProperties::signed_volume_from_boundaries` shares the centroid
reference policy, but does not evaluate quartic mesh first moments or the three
surface first-moment quadratures. Exact cubic accumulation preserves binary64
mesh contributions and cross-object cancellation before the final rounding,
including individually overflowing volumes. B-rep/surface integrals remain
numerical, on exact NURBS and trims rather than viewport tessellation.

The common base is the center of participating bounds, excluding unused mesh
vertices. Closed oriented meshes use the equivalent exact origin path. Each
closed B-rep retains its own conditioned frame; open pieces share one frame.
An unrepresentable final scalar is an error; a tiny nonzero scalar can round to
zero. The centroid query can still use the unrounded volume internally.

Display conversion uses exact nominal length-unit ratios, cubed before the final
scalar rounding. This can report a finite converted volume even when the volume
in model units, or the conversion cube alone, overflows or underflows. Custom
source units retain their exact stored binary64 scale. Unitless sources use
factor one even with an explicit display choice, as observed in Rhino; this does
not establish a physical size for a unitless model. Unset conversion is an error.

SubD geometry is **not implemented**.
Native output round-trips its computed binary64 result; it does not reproduce
Rhino's printed uncertainty or exact history text. The [official command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/volume.htm)
documents unjoined enclosing collections and display-unit selection.

## Evidence

See the [scalar command audit](../volume-command-audit.md), complete retained
observations and source-only generator. It includes real `_Volume` executions,
warning input, source geometry, selection, printed results, and separate public
single-object/collection API diagnostics. Scalar results are parsed from actual
command history, never substituted from API values. All native replay tolerances
remain fixed at absolute `1e-9`, relative zero, including printed-value residuals.

```sh
python3 -m tools.rhino_oracle.volume_command_replay \
  tools/rhino_oracle/fixtures/volume_command.json \
  tools/rhino_oracle/observations/volume_command.json
python3 -m unittest tools.rhino_oracle.test_volume_command
```
