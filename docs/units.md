# Document units

New documents use millimetres. `Document::with_units` creates an empty document
with explicit `LengthUnitSystem` metadata. Custom scales must be finite and
positive and custom names cannot contain NUL bytes.

`Document::set_units(units, rescale)` changes an existing document as one
undoable setting edit:

- `rescale = false` changes metadata only, retaining coordinates and tolerance.
- `rescale = true` scales all geometry about the world origin. Numeric absolute,
  relative, and angular tolerances stay unchanged. Hidden and locked objects
  are included. Geometry conversion uses a proportionally scaled validation
  tolerance internally so shrinking existing geometry does not collapse it.

Attributes, groups, object order, selection, and selection-recall memories are
preserved. Undo and redo exchange stored geometry rather than applying an
inverse scale, avoiding accumulated numerical drift. The operation also joins
an active transaction and rolls back with other document edits.

Invalid units, unrepresentable factors, and geometry conversion errors are
rejected before document mutation. Identical unit settings are a no-op and
preserve redo history. Conversions involving unitless metadata retain
coordinates; rescaling involving unset units is rejected unless nothing changes.

This is currently a Rust document API, not a Rhino Units command implementation.
Eight retained Rhino 8.32 public-API measurements cover millimetres/metres,
millimetres/inches, and unitless conversions, including hidden/locked objects
and selection. The fixture in `crates/viboceros-document/src/units/fixtures/`
was generated with `tools/rhino_oracle/generate_document_units_reference.py`
in an isolated instance using headless documents. It measures
`RhinoDoc.AdjustModelUnitSystem`, not the interactive Units command. Rhino
command behavior and UI/view settings have not yet been integrated.
Interchange behavior is documented in
[file formats](file-formats.md).
