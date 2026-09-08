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
to finish using its original settings. These prompt shortcuts are not yet
available for `InterpCrv`.

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
