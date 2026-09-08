# Model units

[Command reference](README.md) · [File formats](../file-formats.md)

```text
Units
Units Meters Scale=Yes
Units Inches Scale=No
Units Custom MetersPerUnit=0.25 Scale=Yes Name=quarter metre
Undo
```

`Units` reports model units and numeric absolute, relative, and angular
tolerances. Changing units requires an explicit `Scale=Yes` or `Scale=No`:

- `Yes` rescales all geometry about the world origin to preserve physical size,
  including hidden and locked objects and objects on hidden or locked layers.
- `No` changes unit metadata without changing coordinates.

Numeric tolerances remain unchanged in both cases. Object IDs, attributes,
selection, groups, and object order are preserved. A change is one undo step;
queries, unchanged settings, invalid input, and failed rescaling preserve redo.
Unrepresentable geometry causes the entire change to fail atomically.

Unit names are case-insensitive. Supported names are `None` (also `Unitless`),
`Microns`, `Millimeters`, `Centimeters`, `Meters`, `Kilometers`, `Microinches`,
`Mils`, `Inches`, `Feet`, `Miles`, `Angstroms`, `Nanometers`, `Decimeters`,
`Dekameters`, `Hectometers`, `Megameters`, `Gigameters`, `Yards`, `PrinterPoints`,
`PrinterPicas`, `NauticalMiles`, `AstronomicalUnits`, `LightYears`, `Parsecs`, and
`Unset`. British metric spellings such as `Millimetres` work too. Short forms
are `mm`, `cm`, `m`, `km`, `in`, `ft`, and `yd`.

Conversions involving `None` retain coordinates even with `Scale=Yes`.
Rescaling to or from `Unset` fails unless the unit setting is unchanged.
Custom units use `Units Custom MetersPerUnit=value Scale=Yes|No Name=name`.
The factor is the physical length of **one custom unit in meters**, not units
per meter, and must be finite and positive. The options appear in the order
shown. `Name=` must be last: its value is the remaining command text, including
internal spaces, Unicode, and `=` characters. No quoting is needed; quote
characters are literal parts of the name. Trailing command whitespace is
discarded. Empty/whitespace-only names and embedded NULs are rejected.

Custom settings are undoable and support the same explicit scale choice as
standard units. Reports identify them as custom and include meters per unit.
3DM export preserves the name and factor; custom-to-standard conversions use
that factor. Layout units and custom-unit aliases are not changed.

This is Viboceros's explicit CLI workflow, not Rhino dialog or macro-syntax
parity. Rhino's [unit settings](https://docs.mcneel.com/rhino/8/help/en-us/documentproperties/units.htm)
offer a rescale choice when changing unit systems. The underlying document
conversion behavior is checked against retained Rhino 8 API fixtures.
This command does not edit layout units, tolerances, or viewport/grid settings.
In the application, changing the physical unit scale clears the remembered
relative-point reference, including changes made by Undo/Redo and `Scale=No`.
Enter a new absolute point before using relative coordinates again. Unit
queries, no-ops, failed changes, and custom-name-only changes retain that
reference. Like other model commands, `Units` cancels an in-progress drawing
command; its uncommitted points are not rescaled into a new command.
