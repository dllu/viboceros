# Document tolerances

[Architecture](architecture.md) · [Document units](units.md)

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

This is currently a Rust document API. A tolerance-settings command/editor and
full tolerance persistence in 3DM are not implemented by this change. `Units`
reports these settings but continues to leave their numeric values unchanged.
