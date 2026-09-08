# Document units

New documents use millimetres. `Document::with_units` creates an empty document
with explicit `LengthUnitSystem` metadata. Custom scales must be finite and
positive and custom names cannot contain NUL bytes.

`Document::set_units(units, rescale)` changes an existing document as one
undoable setting edit:

- `rescale = false` changes metadata only, retaining coordinates and tolerance.
- `rescale = true` scales all geometry about the world origin and scales the
  absolute tolerance by the same unit factor. Relative and angular tolerances
  stay unchanged. Hidden and locked objects are included.

Attributes, groups, object order, selection, and selection-recall memories are
preserved. Undo and redo exchange stored geometry rather than applying an
inverse scale, avoiding accumulated numerical drift. The operation also joins
an active transaction and rolls back with other document edits.

Invalid units, unrepresentable factors, and geometry conversion errors are
rejected before document mutation. Identical unit settings are a no-op and
preserve redo history. Conversions involving unitless metadata retain
coordinates; rescaling involving unset units is rejected unless nothing changes.

This is currently a Rust document API, not a Rhino Units command implementation.
Its document invariants are tested locally; Rhino command behavior and UI/view
settings have not yet been integrated. Interchange behavior is documented in
[file formats](file-formats.md).
