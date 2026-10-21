# Modeling comparison evidence

[Oracle overview](../oracle.md) · [Evidence index](README.md)

The `remap_cplane` operation supports 52 native recipes through both the Rust
oracle and the Python comparison API. Captures use public planes and document
APIs in an empty owned document under a private settings scheme. Source geometry
is constructed from the request; saved records constrain full definitions,
grips, groups, selection and Undo/Redo, including separate EndCommand and script
cleanup snapshots. The application replays 150 main input paths and two
Copy-cancellation workflows. See [RemapCPlane](../commands/remap-cplane.md) for
reproduction and comparison scope.

`project_curve_plane` compares complete NURBS curve images against RhinoCommon's
public ProjectAlong transform. Its four recipes and current capture limits are
described in [infinite-plane projection](../commands/project-infinite-plane.md).

The `surface_trim_geometry` operation accepts a complete `surface` definition
and increasing `u` and `v` intervals. It compares direct Rhino SDK rectangular
trimming against the selected portable or usd-brep backend, including complete
surface definitions. See [surface extraction qualification](../smlib-surface-segments.md)
for the retained whole-domain padding discrepancy and command captures.
The `surface_isocurve_geometry` operation accepts a complete `surface`, a single
varying `direction` (`u` or `v`) and a fixed `parameter`. Its native producer and
current SDK-capture status are described in [isocurve qualification](../smlib-isocurves.md).
The `curve_interpolate_geometry` operation accepts `points`, `degree`,
`knots` (`uniform`, `chord`, `sqrt_chord`), `closure` (`open`, `smooth`, `sharp`)
and optional `start_tangent`/`end_tangent` vectors. It compares complete SDK-style
curve definitions; the 30-case fixture currently has portable/native evidence
with fresh Rhino capture pending. See [point interpolation qualification](../smlib-curve-interpolation.md).
The existing `curve_rebuild_geometry` operation now selects the native backend
in `native-smlib` builds. `curve_rebuild_native.json` exercises all degrees,
odd/even closed counts and the public point limit; its portable-reference and
SDK-capture status are recorded in [Rebuild qualification](../smlib-curve-rebuild.md).
Surface Rebuild also selects the native backend. The
`surface_rebuild_native.json` fixture includes retained source definitions and
additional degree/count cases; see [tensor qualification](../smlib-surface-rebuild.md).
`surface_tween_sampled_geometry` also selects the native backend.
`surface_tween_matched_geometry` accepts `start_surface`, `end_surface`, `number`
and `refit` for direct Viboceros control/Refit comparisons. Rhino references for
these modes use retained bounded command captures; the installed RhinoCommon
API exposes only sampled tween construction. See
[native TweenSurfaces](../smlib-surface-tween.md).
The existing `curve_fit_geometry` operation selects native fixed-row fitting
with `native-smlib`. The 50-case `curve_fit_native.json` corpus compares that
producer with the saved portable implementation; fresh SDK capture remains
pending. See [native fitting qualification](../smlib-curve-fit.md).
`curve_tween_geometry` also selects native sampled-source interpolation.
`curve_tween_native.json` covers the permanent modes and the 9,999-division
boundary; see [native TweenCurves sampling](../smlib-curve-tween.md).
`piecewise_degree_elevation.json` probes exact native curve and tensor elevation
across full-order breaks. Its reference uses independent rational Bernstein
coefficients; see [piecewise qualification](../smlib-piecewise-elevation.md).
`curve_match_geometry` selects native structural preparation with `native-smlib`.
The retained 160-case corpus compares complete outputs with the pre-change
producer and Rhino captures, preserving the known boundary-tail discrepancy;
see [native Match qualification](../smlib-curve-match.md).

The [block workflow protocol](../block-workflow-oracle.md) records definition-local
members, nested placements, shared redefinition and both explosion depths using
stable numeric handles through the Python and Rust oracle APIs.

The [polyhedral Boolean commands](../polyhedral-boolean-commands.md) retain 65
native command recipes from private Xvfb. Rust command tests replay all 65 physical
and document outcomes, including history and the non-solid two-hole intersection;
seven coplanar seam differences stay explicit. The `polyhedral_boolean_command`
operation runs the closed native recipes. Mathematical SDK comparison uses the
separate [Rust/Python polyhedral API](../polyhedral-booleans.md).
Compound follow-ups retain [41 set recipes](../compound-intersections.md) and
[54 oriented-pair/common recipes](../compound-pairs.md). Command tests compare
31 and 46 complete physical/metadata records respectively; the historical native
failure-marker discrepancy is retained separately. Two additional seam/count
differences, failure classifications and native performance remain explicit limits.
The [ordinary common follow-up](../common-intersection.md) adds 80 ordered public-box
commands from a fresh private Xvfb scheme. Every record is replayed for geometry,
metadata, selection and retention, with four history recipes and no face/edge
count discrepancies. Native success can depend on selection order; finite
boundary witnesses and box recipes do not establish general geometry parity.
The [surface/UV follow-up](../surface-curve-certificates.md) retains 13 public SDK
recipes, including five native `Surface.Pushup` curves on cylinders, spheres,
and a torus. The local `surface_pushup_certified` operation constructs certified
spatial images from original UV definitions; see [pushup evidence](../certified-surface-pushups.md).
The local `surface_curve_deviation` operation proves continuous
matched-parameter error with exact rational bounds. The native sphere boundary
crossing's different parameter speed remains a recorded diagnostic.

The [fixed-endpoint follow-up](../constrained-surface-pullbacks.md) retains eight
public `Surface.Pullback` and closed-isocurve records. The local
`surface_pullback_certified` operation accepts optional UV endpoint constraints
and proves its complete spatial error at absolute tolerance. Native planar
pullbacks have different parameter speeds; their geometric locus samples and
the independent endpoint image witnesses remain explicit in the records.

The [automatic branch follow-up](../automatic-surface-pullbacks.md) adds 24 SDK
recipes for changed charts, closed isocurves, collapsed boundaries, and a
singular diagonal. Local requests omit endpoint constraints and return complete
certificates. Four native failures and the swapped two-pole surface's different
parameter speed remain in the raw records. Already certified bounds are reused
by the local Python API.

The [derivative-free follow-up](../derivative-free-surface-pullbacks.md) captures ten
nonlinear singular-endpoint recipes on private Xvfb. All ten previously failed
locally and now return complete correspondence certificates. Eight native
pullbacks succeed with different parameter speeds; two cubic native failures
remain explicit in the raw evidence.

The [ScalePositions guide](../commands/scale-positions.md) covers 249 bounded native
recipes from private Xvfb and 289 application replays. Complete geometry, tight
center translations, overlapping groups, metadata, grip display, numeric getter
phases, defaults and external Undo/Redo are compared at `1e-9`. Cursor captures
cover reference axes, origin planes and the numeric 1D cutoff. Two inputs just
above the boundary remain diagnostics; preview appearance and performance are unverified.

The [ScaleNU guide](../commands/scale-nu.md) links numeric, cursor/reference and
option captures from private Xvfb. Options add 36 recipes replayed through both
complete and incremental application input, plus three follow-up Move cases for
temporary grip display. Geometry, groups, preferences, selection and Undo/Redo
are compared; scripted cursor dependencies and preview appearance remain open.

The [Bend oracle](../bend-geometry.md) retains 76 public SDK point maps, the original
48 point-command recipes, 76 geometry commands, and two preference workflows
from private Xvfb sessions. SDK maps replay through the Rust/Python protocol;
command and UI tests compare geometry, attributes, groups, selection, Undo/Redo
and 41 option/history steps. The [command guide](../commands/bend.md) documents
numeric angle memory and fitting bounds. Another 35 [cursor captures](../bend-preview.md)
constrain live previews, display modes and the spine-start mouse plane.

The [Taper foundation](../taper-geometry.md) retains 51 SDK point maps and six owned
native point commands on private Xvfb. The Python/Rust `taper_points` protocol
replays validity and coordinates; command snapshots constrain Copy, grouped
Rigid and Undo/Redo. Follow-on [Taper command tests](../commands/taper.md) use 90
geometry recipes and a 33-step preference/history workflow, including signed
distances, radial point picks, construction planes and a fitting sweep. Native
precision diagnostics are retained in the [provenance](../taper-command-provenance.json).
Another 30 [Taper cursor captures](../taper-preview.md) constrain quick previews,
display modes and axis-normal mouse planes, with six earlier plane diagnostics
retained separately. All were captured using private Xvfb settings schemes.

The [Maelstrom Circle input captures](../maelstrom-circle-input.md) compare public
Circle definitions, command point maps, size memory, selection and history in
private Xvfb sessions.
The [Maelstrom FitPoints workflows](../maelstrom-fit-points.md) add 17 completed
commands and 250 point/curve-end maps. The retained
[plane-basis experiments](../plane-fit-basis.md) leave native orientation unresolved.
The [Circle fit foundation](../circle-fit.md) adds spatial-distance fitting on a
PCA plane with retained native command/SDK evidence. Native basis conventions
and near-collinear convergence remain unresolved. Circle's
[point-object input](../circle-fit-input.md) adds eight native selection workflows
and replays the earlier 22 native command loci through the command adapter.
Sixteen [control-point workflows](../control-points.md) add curve, surface and
mesh grips, mixed point objects, display commands and history. Both input
families retain EndCommand and final script snapshots to distinguish getter
cleanup from Undo/Redo; completed fits receive no idle cancellation tokens.
Thirty-two [grip-transform workflows](../grip-transforms.md) record partial control
edits, copies, seam aliases, display and history. Thirty-one workflows replay
through application input; the retained collapsed mesh-face case is incompatible.
The [Maelstrom foundation](../maelstrom-geometry.md) retains 108 public SDK point
maps and eight native point commands from private Xvfb. The Python/Rust
`maelstrom_points` protocol replays validity, radial profiles and coordinates;
actual commands constrain Copy, grouped Rigid, selection and Undo/Redo. This is
geometry evidence. Its [command captures](../commands/maelstrom.md) add 85 geometry
recipes and 29 preference/history steps, including the fitting floor, unchanged
Undo entries, first-radius memory, per-command Rigid defaults and immediate Copy
getter persistence. Its [input captures](../maelstrom-input-provenance.json) add
44 commands for angle quadrants, tiny radial picks, empty getters and Copy
batches; application tests replay these and the 29 preference/history steps.
Its [live previews](../maelstrom-preview.md) add 32 pending captures, six earlier
diagnostics, three typed-input captures and three axis-input captures, retaining
owned framebuffer PNGs and public SDK quick-preview witnesses.
