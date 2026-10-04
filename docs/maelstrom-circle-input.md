# Maelstrom first-circle input

[Maelstrom](commands/maelstrom.md) · [Previews](maelstrom-preview.md) · [Architecture](architecture.md)

The first circle is a command input definition. It is never inserted into the
model. `viboceros_command::circle_input::CircleInput` returns its frame and physical
radius; the application uses that frame for the second radius, coil angle,
drafting plane and deformation. Pending picks and guides create no Undo entry.

## Construction and size

| Input | Meaning |
| --- | --- |
| Center, size | Numeric size uses the CPlane. A radius point sets the seam and measures full spatial distance. |
| `Diameter` / `Radius` | Select the numeric size convention. Diameter numbers divide by two; radius points still measure the radius. |
| `Circumference` / `Area` | Convert the next number or measured distance to a radius. |
| `Vertical` | Pick a center, then a radius point; alternatively enter a radius and pick its direction. The circle normal is perpendicular to CPlane Z and the picked radial direction. |
| `Orientation` | After the center, pick the new normal direction. A radius point projects onto that fixed plane. |
| `2Point` | Pick diameter endpoints. The normal follows the CPlane as closely as possible, with deterministic plane axes. |
| `3Point` | Pick three points on the circle; their order sets its normal and the first point sets its seam. |
| `3Point`, two picks, `Radius` | Enter a radius, then pick the center direction; a radius point measures from the first picked point. |

Radius/Diameter mode is remembered independently for Maelstrom. It also controls
the numeric **second radius or diameter** prompt. Circumference and Area are
single size inputs and retain the preceding Radius/Diameter preference. An
accepted circle updates the remembered physical radius, including later Cancel.
Enter accepts that radius. Undo, Redo and a new document retain preferences.
Circle command preferences do not change Maelstrom preferences.

Circumference and Area point inputs measure full spatial distance and keep the
chosen plane axes. They do not follow the radius point seam. For Vertical they
complete immediately in the CPlane X/Z plane, including numeric size inputs.

The getter supports typed world/CPlane coordinates, relative inputs and the
shared drafting path. Orientation size picks use the chosen plane for mouse
projection. Completed pending circle definitions supply the preview guide.
Invalid picks retain their current getter without inserting geometry.

## Evidence and scope

Owned Rhino **8.32.26160.13001** sessions run exclusively in private Xvfb with
separate settings schemes. The main set has 42 command recipes; six additional
recipes verify remembered radius acceptance after cancellation, and four prove
that Circle and Maelstrom have independent preferences. Eight further recipes
compare Circumference/Area point measurements, fixed planes and translated
centers. Four spatial two-point recipes use coordinate coil angles and retain
the actual Circle curve seam, making the frame comparison sensitive to rotation
around its normal. Each completed main or point conversion recipe pairs public Circle
output with public SDK Maelstrom point maps.
Three direct Circle captures verify that rotated CPlane X/Y axes do not change
the two-point seam; command tests also replay the six earlier Circle captures.
Application tests replay input, selection, Undo and Redo. Circle frames, radii
and point maps are compared within `1e-11`.

The [provenance record](maelstrom-circle-provenance.json) hashes fixtures,
observations and helpers. An earlier 42-case diagnostic is retained separately:
repeating an already current Radius/Diameter keyword invokes Rhino's transparent
measurement command. Verified recipes omit that unavailable option; their actual
input tokens are retained as `resolved_inputs`.

The [FitPoints workflows](maelstrom-fit-points.md) now retain 17 completed
native commands, point selection/history behavior and 250 deformation maps.
Interactive integration still needs a compatible fitted-plane orientation.

Tangent, AroundCurve, transparent measurement commands, ProjectOsnap
behavior on actual snapped picks and native guide pixels for these construction
modes remain to verify. The two ProjectOsnap recipes establish only that typed
world points produce the same circle. Pending three-point construction aids,
complex trims, extreme numerical inputs and original native performance also
need more work. These sampled witnesses do not establish full Rhino parity.

```sh
cargo test --release -p viboceros-command circle_input
cargo test --release -p viboceros app::tests::maelstrom
python3 -m unittest tools.rhino_oracle.test_maelstrom_circle
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/maelstrom_circle.json \
  --output /tmp/maelstrom-circle.json --scheme VibocerosOracleMaelstromCircle
```

Only public command/API outputs were used; no proprietary source was inspected.
