# Model units

[Command reference](README.md) · [File formats](../file-formats.md)

```text
Units
Units Meters Scale=Yes
Units Inches Scale=No
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
Imported custom units can be reported and converted to a standard unit;
creating custom units through this command is not implemented yet.

This is Viboceros's explicit CLI workflow, not Rhino dialog or macro-syntax
parity. Rhino's [unit settings](https://docs.mcneel.com/rhino/8/help/en-us/documentproperties/units.htm)
offer a rescale choice when changing unit systems. The underlying document
conversion behavior is checked against retained Rhino 8 API fixtures.
This command does not edit layout units, tolerances, or viewport/grid settings.
