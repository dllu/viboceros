# File formats

[Project overview](../README.md) · [Command reference](commands/README.md)

File commands preserve internal filename whitespace. Use double quotes around
a filename to preserve leading/trailing whitespace or make the path explicit,
for example `ExportStl Binary "parts/two  spaces.stl"`. Backslashes are literal,
not escape sequences. An unterminated quote or text after the closing quote is
an error before document edits; quotes are not a general modelling-command syntax.

Path-based STL, STEP, and 3DM exports write to a temporary file beside the
destination, flush/synchronize the completed contents, then replace the destination.
Validation, writing, or commit failures leave the existing destination intact
and remove the temporary file. Stream-based writers cannot roll back bytes
already accepted by a caller's stream. This is not a power-loss durability
guarantee for the containing directory.

STL/STEP commands export all visible meshable objects, including locked objects
and objects on locked layers. Object hiding, hidden layers, and `Isolate` affect
that set; selection alone does not restrict it. `Export3dm` instead retains the
whole model and its object/layer visibility and lock attributes. A mesh export
with no visible meshable objects fails without replacing an existing destination.

STL imports retain the file's unitless coordinates, including finite triangles
below the document's modelling tolerance. `read_stl` and `read_stl_file` perform
numerical validation without a tolerance argument. Neither import changes the
document's units or tolerances; malformed, non-finite, and degenerate facets are
still rejected. Binary export additionally checks for loss at 32-bit precision.
Facet validation does not impose the default angular modelling tolerance:
very thin non-collinear triangles are retained. Cross-product determinants
compensate product rounding so exactly parallel edges do not gain artificial area.
The ASCII reader accepts one solid block, case-insensitive keywords, whitespace,
and free-form solid names. It rejects incomplete facets, extra geometry fields,
and nonblank content after `endsolid`. Record tokenization uses bounded stack
storage; this does not impose a file-size or line-length limit.

Both ASCII and binary STL are supported. 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, point-cloud locations, lines, circular arcs, NURBS
curves, parameterized polylines, exact piecewise polycurves, untrimmed NURBS surfaces, mixed triangle/quad meshes, and editable
rational NURBS B-reps. Mesh faces retain their arity in 3DM round trips. B-rep
interchange retains shared vertices and edges, exact edge and
parameter-space trim curves, face surfaces and orientation, outer and inner
loops, boundary/mated/seam/singular trims, and modelling tolerances. Layer and
object state are also preserved, including the raw RGB display color, its
layer/object/material/parent source, and surface wire density. Named group
definitions and ordered membership survive round trips, including overlapping
and empty groups.
Unnamed groups receive deterministic `GroupNN` names on export without changing
the document. All existing names are reserved before allocation; a single
candidate sequence avoids repeated scans when many unnamed groups are exported.
3DM import resolves name collisions with ` (Imported N)` suffixes, preserving
ASCII-case-insensitive layer matching and case-sensitive group matching. A
per-import name index and per-base suffix cursors avoid scanning the document
for every collision candidate or restarting suffix allocation for repeated names.
File-level regression tests cover repeated imports across undo/redo, including
renamed layer assignments, ordered memberships, and hidden/locked layer state.
Standalone circle and ellipse objects are exported without approximation as
rational NURBS curves. Arc objects retain their analytic type and native domain.
Polylines retain their native object type and every vertex parameter;
degree-one NURBS remain NURBS instead of being classified by a knot-vector heuristic.
Unsupported object types and specialized B-rep trim forms are
counted and reported during import.
Eight [morphed B-rep cross-reader cases](brep-3dm-interchange.md) check actual
native exports in Rhino, including holes, seams, singular trims and usable meshes.

Polycurves remain composite objects with native line/arc/polyline/NURBS leaves,
independent parameter intervals, and rational control structure. Nested source
composites are flattened on a private copy. Circular segments retain angular
evaluation and endpoint-editing behavior; they are not silently converted to NURBS.
The bridge shares a versioned, validated binary codec
for B-reps and polycurves, checks payload sizes before allocation, and rejects
malformed or trailing data. The typed polycurve payload is version 2; the reader
also accepts version 1 NURBS-only payloads. It does not fit curves or average endpoints.
Free NURBS curves with internal full-order knots are decomposed into valid native
pieces before export. Connected pieces become PolyCurves; positional gaps produce
separate objects with the original attributes. Export reports the actual object
count without editing the document. See [full-order curve interchange](curve-3dm-interchange.md)
for parameter preservation, cross-reader tests, and remaining limits.
The [rational range adapter](rational-3dm-range.md) prevents silent homogeneous
coordinate underflow, chooses safe common weight scales when needed, and imports
subnormal-weight curves and surfaces through direct homogeneous division.
An independently generated Rhino 8 nested line/arc reference is retained in
`crates/viboceros-io/tests/fixtures/`, with its generator and provenance documented
alongside it. Tests check the analytic locus and subsequent round trip.

When a distinct, finite line is below OpenNURBS's `LineCurve` coincidence
threshold, 3DM export uses an exact degree-one NURBS instead. Endpoints and the
native parameter interval are preserved, but the imported representation is a
NURBS curve. The same fallback applies to individual line segments in polycurves;
ordinary line segments retain their analytic representation.

The low-level 3DM I/O model preserves standard, unitless, unset, and custom
length-unit metadata (`ThreeDmModel::units`). Custom names and finite,
positive metres-per-unit scales round-trip without rescaling coordinates;
new I/O models explicitly default to millimetres. The shared
`LengthUnitSystem` provides validated, checked conversion factors;
`Document::with_units` initializes explicit document units, and 3DM export
retains them. Default documents use millimetres. `Import3dm` converts file
coordinates to document units before editing the document. Defined primitives
use numerical validation rather than a document-dependent minimum feature size,
both during decoding and conversion; short lines are not reported as unsupported
merely because they are below the modelling tolerance. Source-space B-rep
topology-matching tolerance is converted to the source units.
That conversion is deferred until a B-rep is decoded: a point-only file is
not rejected because an unused matching tolerance would over/underflow.
If a B-rep needs an unrepresentable source tolerance, the import fails with
an explicit error rather than silently skipping the B-rep.
Native-file command regressions cover both extremes, including attribute/group
preservation, exact Undo/Redo restoration, unchanged target settings, and failed
mixed-geometry imports preserving the complete document and redo history.
Unitless files retain coordinates; unset units and unrepresentable conversion
factors or transformed coordinates are errors. The low-level
`read_3dm_file` still reads raw file coordinates, while
`read_3dm_file_in_units` performs the conversion. The document API supports
undoable unit changes through `Document::set_units`; see [document units](units.md).
The [Units command](commands/units.md) exposes standard and custom model-unit changes;
the toolbar reports the current units but has no settings editor.
3DM export also preserves the document's absolute, relative, and angular model
tolerances. Raw reads expose file tolerance metadata separately from geometry
decoding tolerance. Unit-aware reads and import commands retain destination
tolerances; see [tolerance settings and encoding limits](tolerances.md).

Initial STEP interchange uses the Apache-2.0 Monstertruck kernel to read
solid/shell B-reps and assemblies. The independent `step/export` module owns
mesh-to-shell construction, source-unit conversion, and staged destination
writes; it shares the geometry-record adapters but not the importer's parsing
or tessellation machinery. Imports apply instance transforms and robustly
tessellate exact trimmed surfaces into validated display meshes. Tessellation
extent samples use range-safe quarter stations with exact parameter
endpoints. Relative extent sizing scales axis spans before computing the diagonal,
avoiding overflow when finite endpoints span more than the binary64 range.
Unit tests cover extreme opposite-sign parameter domains and finite coordinates
up to `f64::MAX`; these validate tolerance setup, not the downstream kernel's
ability to tessellate arbitrary geometry at those scales. Extent
sampling rejects non-finite vertex, curve, or surface points before tessellation;
the extent accumulator validates every coordinate before changing its bounds,
so NaNs cannot silently disappear through floating-point min/max. Repeated
assembly instances share source-space tessellation during each import, but
each transformed mesh is validated independently. Cached tessellations are
released after their last instance; shell-conversion losses are reported
once per source shape, not once per instance. Parser,
topology, and unsupported-representation losses are reported instead of being
silent. STL and STEP export tessellate visible NURBS surfaces and B-rep faces;
exact outer and inner p-curves are sampled into a constrained UV triangulation
so holes remain open, with interior knot-span samples refining nonplanar
trimmed surfaces. STEP writes the results as faceted shells with shared
topology and planar faces. `ExportStep` converts physical document units to
millimetres. Edge sharing follows raw vertex indices: coincident but unwelded
seams remain separate. Sixteen endpoint-sharing/winding/order cases check every
directed face boundary against source triangle indices; two quad cases check
the shared triangulation diagonal and opposite edge-use orientations. The
generated STEP files are also parsed and checked for face/edge record counts.
These establish the exporter's topology policy, not Rhino seam-conversion parity.
Before serialization, the exporter partitions faces by shared raw edges into
separate shells, in first-face order, retaining face order within each piece.
This follows the [STEP connected-face-set shell hierarchy](https://steptools.com/docs/stp_aim/html/t_connected_face_set.html).
Coordinate-only seam contacts and lone shared vertices do not join components.
Edges and faces are moved into component-local tables; shared point-only vertices
are copied and references remapped. Tests cover 257 interleaved panels and a
two-component export/import round trip. One disconnected mesh can therefore
import back as multiple objects. Non-manifold connected shells are not repaired
or certified by this partitioning.
Connectivity uses ranked unions with path compression over edge incidences,
without allocating per-face neighbor lists. An independent graph traversal
checks all 1,100 graphs on zero through five faces, including deterministic
component/face order, original curve identities, vertex remapping, and edge-use
orientation. These abstract incidence tests complement the geometric fixtures.
The `step/export_plane` adapter writes plane placements using
the native kernel's scale-safe facet normals and reference directions. Parsed
STEP regression records check finite unit directions and reversed winding from
mesh scales `1e-200` through `1e200`, including translated origins. Parsed line
magnitudes also match independent triangle edge lengths. This tests
serialization, not downstream tessellation or Rhino parity at those scales.
The `step/export_geometry` adapter writes coordinates, directions, and line
lengths with an explicit decimal point and uppercase exponent. This fixes
unparseable records such as a bare `1e21` coordinate. A 12,282-value binary64
matrix checks exact numeric round trips, including subnormals and signed zero.
Geometry-number formatting uses a checked 32-byte stack buffer rather than a
temporary heap string per value. Buffer overflow returns a formatting error
without truncation or partial append; output-stream buffering is separate.
Path-based STEP exports buffer record writes and explicitly flush before syncing
and committing the staged file. Fault-injection tests check buffered write and
flush errors; staged-file tests check callback failure, cleanup, and successful
flush-before-commit. A counting sink receives one write for 1,000 four-byte
fragments. This is a write-count regression, not a wall-clock benchmark.
Low-level stream exports leave buffering and flushing to their caller.
The line adapter precomputes finite lengths and unit directions with the native
kernel's scale-safe norm and normalization, once per unique exported edge.
Formatting does not repeat geometric arithmetic. Some
valid extreme-scale native meshes have edge differences or lengths beyond binary64;
these return an explicit error instead of emitting invalid STEP directions.
Regression tests cover huge meshes, including failure after a valid
earlier mesh, and verify unchanged output streams and existing destinations.
`ExportStep` writes the correspondingly converted absolute tolerance as
the file's distance accuracy. Full-file parsing tests check exact declared
accuracy from `f64::MIN_POSITIVE`
through `f64::MAX`, independently of coordinate magnitude.
Unitless and unset documents are rejected; conversion failures leave an
existing destination unchanged. The low-level
`write_step`/`write_step_file` APIs interpret coordinates as millimetres;
their `_in_units` counterparts accept explicit source units and tolerance.
General editable STEP B-rep interchange is not implemented yet. The low-level
`read_step_planar_shells` API converts supported planar source shell definitions
to validated native B-reps without tessellation; it does not yet provide assembly
placement, unit conversion, or document integration. `ImportStep` still imports meshes.
The [native B-rep conversion boundary](step-brep-boundary.md) records the retained
source topology/trim evidence and the representation work still required.

Assembly regression tests include repeated parts beneath a translated,
rotated parent, with expected corner coordinates checked independently of
the importer's matrix arithmetic. These are generated STEP fixtures, not
Rhino parity measurements. `ImportStep` resolves SI prefixes and
conversion-based length units (including nested conversion factors), then
converts coordinates into document units. Tessellation accuracy uses the
modelling tolerance in source coordinates, while resulting triangles and unit
transformations use numerical validity checks. Small finite faces are retained;
genuine collapse remains an error. Export likewise preserves valid small meshes
while converting the declared file accuracy separately. This
currently requires a single data section with uniform length units across
contexts. Missing, mixed, cyclic, or unsupported unit definitions and
non-radian angular contexts are rejected before document edits. Mixed-unit
assembly conversion remains unimplemented. The low-level `read_step` and
`read_step_file` APIs retain raw file coordinates; their `_in_units`
counterparts perform checked conversion.
Both reader paths reject zero or multiple data sections explicitly rather
than panicking or silently ignoring later sections. UTF-8 decoding retains
the Latin-1 fallback for legacy raw header bytes.
The unit-aware reader also rejects duplicate complex-entity components and
checks explicit dimensional exponents against length dimensions. Conversion
units must reference dimensions; SI units may use their derived dimensions.
