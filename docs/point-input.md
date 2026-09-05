# Typed point input

[Interface](interface.md) · [Command reference](commands/README.md)

Start a drafting command such as `Line`, then type a point at each prompt.
Mouse picks and typed coordinates can be mixed. Enter finishes collected-point
commands such as `Polyline`; Escape discards the unfinished geometry. Successful
completion uses the existing command transaction and is one undo step.

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

The current fixed planes are XY for Top/Perspective, XZ for Front (normal -Y),
and YZ for Right (normal +X). Camera navigation does not rotate these planes.
One-line commands such as `Line 0,0,0 4,5,0` still use their documented world
coordinate arguments. See [construction-plane primitives](construction-planes.md)
for Circle/Polygon orientation, rectangle projection, and signed box heights.

Not yet implemented: custom construction planes, scalar distance/angle
constraints, unit expressions, surveyor/DMS notation, `x,y<elevation`, and
editing command options inside an active prompt. Nonzero scalar input is
explicitly rejected rather than interpreted as a point.

`viboceros-drafting/point_input` owns parsing and frame resolution;
`app/point_input` routes typed and picked points through one validation path.
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
