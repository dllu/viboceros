# Typed point input

[Interface](interface.md) · [Command reference](commands/README.md)

Start a drafting command such as `Line`, then type a point at each prompt.
Mouse picks and typed coordinates can be mixed. Enter finishes collected-point
commands such as `Polyline`; Escape discards the unfinished geometry. Successful
completion uses the existing command transaction and is one undo step.
If a collected-point command fails on completion, its points, prompt, and
captured construction plane remain available for correction and retry. The
failed transaction does not leave partial geometry behind. Polyline segments
with unrepresentable lengths are rejected before a vertex is appended.

The Curve prompt rejects adjacent controls whose X, Y, and Z differences are
each at most `2^-32`, independently of model tolerance. This is a coordinate-wise
test, not a Euclidean distance test. The lower-level NURBS constructor can still
represent repeated controls. A private-Xvfb Rhino 8.32 prompt probe confirmed that exact duplicate
start/interior controls and a start-point displacement of `1e-10` were skipped:
all four sequences produced the same three-control-point quadratic curve.
See the [initial response](control-point-prompt-measurement.json) and
[30 follow-up measurements](control-point-threshold-measurement.json): the
follow-up varies model tolerance from `1e-9` to `0.01`, checks the adjacent
floating-point values below/above `2^-32`, equality, and diagonal offsets.
These probes use points near the world origin; they do not establish behavior
at very large translated coordinates or every repetition pattern.

InterpCrv's point prompt uses the same fixed coincidence test as Curve, and
open command interpolation retains distinct points below model tolerance. A
[Rhino 8.32 diagnostic](interpolation-point-prompt-measurement.json) at absolute
tolerance `0.01` instead skipped offsets through `2^-32`, failed construction
at the next float above that boundary and at `1e-9`, and successfully included
offsets of `1e-6`, `0.001`, and `0.1`. Thus point-prompt rejection and solver
failure are distinct behaviors. The six successful cases now replay through
the app and match Rhino control points within `1e-9`; the two Rhino solver
failures remain diagnostics rather than required native rejections.
Closed command interpolation also retains distinct points independently of model
tolerance. A [closed-curve follow-up](interpolation-closure-tolerance-measurement.json)
places a `0.001` offset both inside the sequence and beside the seam, for smooth
and sharp closure, at model tolerances `0.01` and `1e-9`. All eight cases match
Rhino control points within `1e-9`; Rhino's responses are identical across those
tolerances. This corrects both rejection of nearby interior points and unwanted
seam-point removal. The general-purpose interpolation helper retains its explicit
tolerance policy.
The cubic point prompt automatically finishes a closed curve after at least
two collected points when the next point is within Euclidean distance
`1.490116119385e-8` of the first (inclusive, independent of model tolerance).
This is the OpenNURBS decimal `ON_SQRT_EPSILON` constant, not the slightly
different square root of binary64 epsilon. The closing gesture is not appended
as another interpolation point. For cubics, two collected points produce a
non-periodic curve; three or more produce smooth periodic closure by default.
Sharp settings are retained; incompatible
tangent constraints leave the original draft intact on failure.
The [18-case seam audit](interpolation-auto-close-measurement.json) checks
ordinary, exact-boundary, and diagonal offsets near the world origin, and an
exact seam with no final Enter. Typed and picked completion, undo/redo, and
failed-completion recovery have separate app tests.
A [13-case translated follow-up](interpolation-translated-auto-close-measurement.json)
checks offsets `0`, `1e-8`, `2e-8`, and `1e-6` at X origins `0` and `±1e6`,
plus a return to the start after only two collected points. All control points
match within `1e-9`. A separate [no-Enter probe](interpolation-two-point-auto-close-measurement.json)
confirms that the two-point return finishes automatically, despite producing a
non-periodic result. At `±1e6`, a `2e-8` endpoint gap is also
reported as closed by Rhino's coordinate-relative topology test, but remains
non-periodic. Closed state alone must not drive automatic prompt completion.
More extreme translations and Rhino's constrained/sharp auto-close behavior
remain unmeasured; the one-line constructor does not perform this prompt gesture.

Degree-one InterpCrv distinguishes exact and nearby seam inputs. Its
[six-case audit](interpolation-degree-one-auto-close-measurement.json) checks
two or three collected points followed by offsets `0`, `1e-8`, and `2e-8`.
On Enter, two-point returns retain the actual endpoint; with three collected
points, offsets within the same fixed seam threshold reconcile to the start.
Preview and completion share this rule without modifying the raw draft points.
No-Enter probes at `1e-8` for both point counts remained in Rhino's next-point
prompt (visually checked in the private Xvfb) until the 100-second client timeout.
[Exact returns](interpolation-degree-one-exact-close-measurement.json), however,
finish without Enter for both point counts. The cubic near-seam automatic
completion rule must not be applied unchanged to degree-one drafts.
The [nine-case reconciliation boundary audit](interpolation-degree-one-seam-boundary-measurement.json)
confirms equality at `1.490116119385e-8` is included, the next float is excluded,
and a longer diagonal is excluded at model tolerances `0.01` and `1e-9`.
Three additional cases at X=`1e6` agree within `1e-9`; closed topology there
still need not imply an exactly reconciled endpoint. Preview and completion
both replay against these references. Smaller nonzero automatic-completion
offsets remain incompletely audited.
An [ordinary three-point closure baseline](interpolation-closure-prompt-measurement.json)
now matches Rhino's smooth and sharp control points within `1e-9` through app
completion.

During `Polyline`, `Curve`, or `InterpCrv`, type `Undo` to remove the last
draft point without changing document history or its redo stack. Relative input
then starts at the last remaining point. Removing every point clears the relative
origin and captured plane; the next absolute point or mouse pick starts afresh.
Undo on an empty draft leaves the prompt active. After completion, `Undo` again
operates on document history. This implements the point-removal part of Rhino's
[curve Undo option](https://docs.mcneel.com/rhino/8/help/en-us/commands/curve.htm),
not general undo of all prompt options.

In `Polyline`, `Close` finishes with a segment back to the first vertex after at
least three points. An exactly repeated first vertex is not appended twice.
An invalid closing segment leaves the draft and input available for correction;
near-but-not-identical endpoints are not silently moved. Closing does not change
the remembered last explicitly entered point. The whole polyline is one document
undo step.

In `Curve`, `Close` finishes with smooth closure and `Sharp` finishes with a
kink, using the same geometry as `Curve ... Close=Smooth|Sharp`. Both require
at least three control points. Failure retains the points, construction plane,
degree, and previous closure setting; you can correct the draft or press Enter
to finish using its original settings. `InterpCrv` also supports `Close` and
`Sharp`, preserving its degree and knot-spacing settings. Its interpolation
constructor determines the required point count. Explicit endpoint tangents
cannot be combined with closure: a rejected attempt preserves those tangents
and the open draft so Enter can still finish it.

While collecting `Curve` control points, `Degree=n` and
`Close=Open|Smooth|Sharp` update the draft settings without finishing or
discarding points. Degree uses the one-line command's integer range policy
(clamped to 1–11); the resulting degree can be lower when there are too few
control points. Enter finishes using the updated settings. Invalid values leave
both the draft and typed text intact. These single-entry settings use Viboceros's
one-line syntax, not Rhino's multi-step option prompts. Draft `Undo` still removes
points, not setting changes.

`Curve` and `InterpCrv` display the curve from their accepted points in
all four viewports, alongside the accepted-point polygon and point markers.
Polyline drafts and accepted construction-plane prompt points also remain visible
without hovering, including while the command field has focus. Cursor tracking
and snap indicators remain local to the hovered viewport.
The preview uses the completion constructor, including degree reduction and
closure. Invalid or insufficient controls produce no curve preview. The cursor's
unaccepted point is still a straight tracking guide, not a prospective control
point. `InterpCrv` uses the command's default open cubic chord-spaced interpolation.
Start with options such as `InterpCrv Degree=3 Knots=Uniform Close=Smooth`
to change that construction. `Degree=1|3`, `Knots=Uniform|Chord|SqrtChrd`,
`Close=Open|Smooth|Sharp`, and world-space `StartTangent=x,y,z` /
`EndTangent=x,y,z` use the one-line parser; tangents require an open cubic.
Explicit directions must be finite and nonzero. Zero vectors (including inputs
that underflow to zero) are rejected at entry; valid nonzero subnormal and large
vectors are retained, without applying model tolerance to their magnitude.
Preview and completion retain all startup settings, including endpoint directions.
These options can also be entered during an active InterpCrv prompt, alone or
together (for example `Degree=1 Knots=Uniform`). Unspecified settings and points
are retained. The entire update is rejected if an option is invalid, duplicated,
or incompatible with another setting; document history is untouched. Enter
finishes with the updated settings. `StartTangent=None` and `EndTangent=None`
restore automatic endpoint directions; these explicit reset values are Viboceros
syntax, not a claim of Rhino macro compatibility. For example,
`StartTangent=None EndTangent=None Close=Smooth` clears both constraints and
changes closure in one atomic update. The same reset values work in one-line
commands and at draft startup.
InterpCrv currently has a 256-point solver limit. A 257th typed or picked point
is rejected without changing the draft, preview, or relative origin; use Undo
to replace a point or Enter to finish. A sharp closure may require one extra
repeated seam point within that same limit. This is a Viboceros implementation
limit, not a measured Rhino restriction.
Open cubic interpolation uses a linear-memory tridiagonal solve, with a pivoted
dense fallback for ordinary-size systems that the fast path cannot solve.
Both open-curve paths solve offsets from a local origin before restoring world
coordinates, reducing cancellation for clustered points far from the origin.
Fixed endpoint and tangent-handle coordinates are retained exactly.
Periodic cubic interpolation also centers its right-hand side before the
pivoted solve. This preserves constant coordinate planes instead of introducing
translation-dependent control noise; regressions cover three knot spacings and
ordinates from `±1e6` through `±f64::MAX`, including 33 evaluated stations per
curve. If translating to the first input would overflow a coordinate difference,
the solver retains world coordinates; a wide uniform periodic curve checks this
fallback. These are native numerical invariants, not Rhino measurements at
extreme coordinates or a guarantee about GPU rendering at those scales.
Two-point uniform cubics do not require a representable chord length: their
domain is `[0,1]` and control points use overflow-safe convex interpolation.
Chord-based two-point domains still require a finite endpoint distance.
Uniform knot-interval construction likewise avoids unused chord-length
calculations. A periodic four-point regression has unrepresentable adjacent
chords but finite controls and evaluated points; it now completes through typed
input, cached preview, and undo/redo. Chord-based spacing still rejects that
fixture. This does not remove genuine chord requirements elsewhere, such as
automatic open-cubic endpoint handles or degree-one arc-length domains.
Regression tests check evaluated points across these extreme uniform curves,
plus typed input, preview creation, completion, and undo/redo. This does not
establish GPU display accuracy at extreme world-coordinate magnitudes.
Model-space preview geometry is cached by points, construction mode, settings,
and interpolation tolerance, including
failed constructions. Camera movement reprojects it without rebuilding the curve;
completion or cancellation clears the cache on the next preview update.

The supported forms follow [Rhino's coordinate-entry documentation](https://docs.mcneel.com/rhino/8mac/help/en-us/user_interface/accurate_modeling.htm):

| Input | Interpretation |
| --- | --- |
| `1,2` or `1,2,3` | Construction-plane Cartesian coordinates; omitted Z is zero |
| `0` | Construction-plane origin |
| `w1,2,3` or `w0` | World coordinates or world origin |
| `r1,2,3` or `@1,2,3` | Construction-plane displacement from the previous point |
| `wr1,2,3`, `rw1,2,3`, `@w1,2,3` | World displacement |
| `5<30` or `5<30,2` | Polar distance/angle, with optional height |
| `5<30<45` | Spherical distance/azimuth/elevation |

Prefixes are case-insensitive and also apply to polar/spherical inputs. Angles
are decimal degrees. Coordinates contain no internal whitespace. For example,
enter `Polyline`, `0`, `r4,0`, `@3<90`, then Enter.
Negative spherical distances reverse the horizontal bearing; elevation retains
its own above/below-plane sign, matching the measured Rhino prompt behavior.
Spherical elevation must lie between -90° and +90° after full-turn reduction
(450° is +90°); out-of-range entries remain editable errors.

Typed input bypasses Osnap, SmartTrack, and Grid Snap. Invalid or overflowing
coordinates leave the prompt and text intact for correction; a subsequent mouse
pick can replace them. Geometrically rejected picks do not change the relative
origin. The application remembers its most recent accepted interactive point,
including after Escape or completion; it does not infer one from a fully
specified one-line command or imported geometry. A relative entry with no
remembered point is rejected.
Line endpoints and sphere radius points must have a finite distance greater
than the current absolute tolerance. This check applies to both typed and
picked points, including finite coordinates whose difference overflows; rejected
endpoints leave the draft and last accepted point intact.

The initial planes are XY for Top/Perspective, XZ for Front (normal -Y),
and YZ for Right (normal +X). [CPlane](cplane.md) can translate or reorient each
viewport's plane independently; camera navigation does not rotate these planes.
One-line commands such as `Line 0,0,0 4,5,0` still use their documented world
coordinate arguments. See [construction-plane primitives](construction-planes.md)
for Circle/Polygon orientation, rectangle projection, and signed box heights.

Not yet implemented: general scalar distance/angle
constraints, unit expressions, surveyor/DMS notation, `x,y<elevation`, and
editing other command options inside an active prompt. Nonzero scalar input is
explicitly rejected rather than interpreted as a point.

`viboceros-drafting/point_input` owns parsing and frame resolution;
`app/point_input` routes typed and picked points through one validation path;
`app/curve_prompt` handles draft-only options separately from document commands.
`viboceros-command/curve_options` owns Curve degree and closure value parsing
for one-line execution, draft startup, and in-prompt setting changes.
`viboceros-command/interpolation_options` provides the corresponding InterpCrv
parser and atomic updates, validating retained directions even for Rust callers
that constructed options without parsing text.
Its canonical formatter is shared by draft completion and settings feedback;
it preserves tangent components and explicitly writes absent constraints as `None`.
Angle reduction preserves tiny negative angles and exact quadrants. Tests cover
large magnitudes, full floating-point precision, invalid input, relative origins,
construction planes, command replacement, cancellation, and undo.

The `point_input.json` oracle fixture compares 19 sequences against Rhino's
actual Polyline coordinate prompt on five planes, including translated and
oblique planes. All agree within absolute/relative `1e-12`; maximum observed
coordinate difference is `3.56e-15`. These untimed probes check coordinate
resolution; application tests separately exercise UI state and undo behavior.
`point_input_diagnostics.json` retains an extreme-scale Rhino trigonometric
discrepancy: `w2e16<90` gives Rhino X approximately `1.22465`, while the native
exact-quadrant result is X=0. It is not a passing reference at `1e-12`.
Additional probes confirmed negative-distance, negative-elevation input. At
elevations ±120°, however, Rhino returned unexpected points (including Z=323
for radius 5), and a multi-point sequence lost vertices. Those results are not
used as a geometry reference; this elevation range is explicitly unsupported.
