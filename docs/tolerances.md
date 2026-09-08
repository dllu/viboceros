# Document tolerances

[Architecture](architecture.md) · [Document units](units.md)

## Command line

```text
Tolerance
Tolerance Absolute=0.01 Relative=0.0001 AngleDegrees=1
Tolerance AngleRadians=0.001
Undo
```

`Tolerance` reports the current settings. Named options may appear in any order;
omitted settings retain their values. Option names are case-insensitive.
`AngleDegrees` and `AngleRadians` are mutually exclusive, and duplicate or
unknown options are errors. Every value must be finite and positive, including
the stored radians after converting degrees. A complete proposal is validated
before any change is recorded. Reports always label the stored angle as radians.

This is an explicit Viboceros CLI workflow, not Rhino macro-syntax parity.
Relative tolerance remains available as part of the geometry kernel's policy.

## Document API

`Document::set_tolerance(tolerance)` changes the validated absolute, relative,
and angular tolerance policy as one undoable settings edit. Construct the value
with `Tolerance::try_new(absolute, relative, angular_radians)`; all three values
must be finite and positive. Absolute tolerance is in model units, relative
tolerance is dimensionless, and angular tolerance is in radians.

The setter returns whether anything changed. An identical value is a no-op
that preserves redo. Changes join an active transaction, so tolerance edits,
unit conversions, and geometry edits can be committed or rolled back together.
Undo/Redo restore the exact stored values.

Existing geometry is neither refitted nor revalidated against the new policy.
Object attributes, groups, selection, model units, and last-changed-object
tracking are unchanged. In particular, increasing absolute tolerance does not
delete existing features smaller than the new tolerance or rewrite stored
B-rep vertex/edge tolerances. Subsequent operations use the new document policy.

`Export3dm` stores all three model tolerances in the archive's native settings.
Raw `read_3dm_file` retains those numeric values independently of its geometry
decoding tolerance. Unit-aware reads and `Import3dm` use the destination policy;
importing geometry never replaces an existing document's tolerance settings.
Invalid nonpositive/nonfinite archived tolerances are reported as invalid model
metadata. Export requires relative tolerance below 1 and angular tolerance at
most π radians, matching OpenNURBS's valid ranges; unencodable settings fail
without replacing an existing file. Layout/page tolerances are not preserved.

There is no graphical tolerance-settings editor yet. `Units` reports these
settings but continues to leave their numeric values unchanged.
