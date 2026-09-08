# Document units

New documents use millimetres. `Document::with_units` creates an empty document
with explicit `LengthUnitSystem` metadata. Custom scales must be finite and
positive and custom names cannot contain NUL bytes.

The toolbar shows the current model units. Hover over the indicator for absolute,
relative, and angular tolerances, plus the custom scale or an unknown/unitless
warning where applicable. The indicator is read-only and remains available
during drafting. Long custom names are abbreviated to keep the toolbar compact.

`Document::set_units(units, rescale)` changes an existing document as one
undoable setting edit:

- `rescale = false` changes metadata only, retaining coordinates and tolerance.
- `rescale = true` scales all geometry about the world origin. Numeric absolute,
  relative, and angular tolerances stay unchanged. Hidden and locked objects
  are included. Existing curves and meshes use numerical validation rather
  than the document's minimum modelling threshold; small features must survive
  successive conversions. Brep reconstruction retains a scaled topology-matching
  allowance and scales stored vertex/edge tolerances.

Attributes, groups, object order, selection, and selection-recall memories are
preserved. Undo and redo exchange stored geometry rather than applying an
inverse scale, avoiding accumulated numerical drift. The operation also joins
an active transaction and rolls back with other document edits.

Invalid units, unrepresentable factors, and geometry conversion errors are
rejected before document mutation. Identical unit settings are a no-op and
preserve redo history. Conversions involving unitless metadata retain
coordinates; rescaling involving unset units is rejected unless nothing changes.

The [Units command](commands/units.md) exposes this API for standard units with
an explicit scale choice. Its CLI syntax is not Rhino dialog or macro parity.
Eight retained Rhino 8.32 public-API measurements cover millimetres/metres,
millimetres/inches, and unitless conversions, including hidden/locked objects
and selection. The fixture in `crates/viboceros-document/src/units/fixtures/`
was generated with `tools/rhino_oracle/generate_document_units_reference.py`
in an isolated instance using headless documents. It measures
`RhinoDoc.AdjustModelUnitSystem`, not the interactive Units command. Rhino
dialog behavior and automatic UI/view-setting adjustments are not emulated.
Interchange behavior is documented in
[file formats](file-formats.md).

The same measurement is available through the Python oracle client's normal
`run_viboceros`, `run_rhino`, and comparison workflow. Use
`tools/rhino_oracle/fixtures/document_units.json`; each `document_units` operation
takes numeric `source` and `target` codes (0 unitless, 2 millimetres, 4 metres,
8 inches) and a boolean `rescale`. It records three fixed normal/hidden/locked
points before and after the change, with the first selected. Initial tolerances
are fixed at 0.001 absolute, 0.0001 relative, and 0.00001 angular, independently
of the request's global tolerance. This is a correctness probe, not a benchmark
(elapsed time is zero). Each Rhino operation disposes its headless document.
