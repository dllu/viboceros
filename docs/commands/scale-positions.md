# ScalePositions

[Command reference](README.md) · [Transforms](transforms.md)

`ScalePositions` scales the positions of individual objects' tight bounding-box
centers. Each object translates without changing its shape, size or orientation.
Grouped objects use their own centers; group bounds do not affect placement.

```text
ScalePositions 0,0,0 2 Mode=3D
ScalePositions 0,0,0 2 1,0,0 Mode=1D
ScalePositions 0,0,0 1,0,0 3,0,0 Mode=2D Copy=Yes
```

Complete invocations use world-space points. Interactive coordinates follow
normal construction-plane input rules; prefix `w` for a world point.

## Modes and input

Enter `ScalePositions`, select objects if necessary, and pick an origin. Choose
`Mode=1D`, `2D` or `3D` before supplying a factor or first reference point.
`Mode` alone opens the mode choice. A fresh application session starts in 3D.

| Mode | Position map | Reference factor |
| --- | --- | --- |
| 1D | Scale along the origin-to-direction axis | Ratio of unsigned projected distances along the first reference axis |
| 2D | Scale along the active construction plane's X and Y axes | Ratio of three-dimensional distances from the origin |
| 3D | Scale uniformly in world space | Ratio of three-dimensional distances from the origin |

A numeric factor uses its magnitude: `-2` acts like `2`. Numeric zero is rejected
and leaves the prompt active. Two reference points can instead define the factor.
A zero-length first reference is invalid; a zero-length second reference finishes
successfully without editing geometry, saves the mode and retains the previous
positive factor.

Numeric input requests a direction only when the command **started** in 1D.
The retained Rhino 8.32 capture shows that changing 3D to 1D at the origin then
entering a number finishes without requesting a direction or moving geometry.
Changing 1D to 2D or 3D still requests the direction, although the final map uses
the chosen mode. Viboceros reproduces this behavior. Starting in 2D or 3D accepts
a numeric factor immediately.

Enter at the factor prompt accepts its remembered value, initially 1. Factor and
mode defaults update on completion and survive Undo. Canceling a pending direction
or mode choice retains the previous completed defaults. Preferences belong to the
command registry and last for the application session.

## Sources, copies and previews

Displayed selected grips alone do not supply sources. A selected whole parent
translates with all its grips, retaining the selected grip indices. Ordinary
selected peers can transform while a grip-only owner remains untouched.

`Copy=Yes` keeps the original sources selected and creates unselected copies in
the **original groups**, including overlapping memberships. Geometry and attribute
UserText are retained. Copies of owners with displayed grips retain their grip
display and picks. Results follow source object order.

Numeric 1D copies repeat direction picks with the same factor. Numeric 2D and 3D
copies repeat factor input. Enter or Escape finishes accepted copies in one Undo
entry. Reference copies retain the first reference while accepting further
targets; that repeated-reference path is implemented but lacks a native capture.

Live previews cache each source center and display geometry, then apply its
individual translation. They include translated clipping bounds and do not edit
the document or defaults. Native preview appearance has not been compared.

## Verification and limits

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/scale.htm#ScalePositions)
describes bounding-center placement and the three modes. The retained
[57 native recipes](../../tools/rhino_oracle/fixtures/scale_positions.json) and
[raw observations](../../tools/rhino_oracle/observations/scale_positions.json)
come from public commands and SDK snapshots in an empty owned document on private
Xvfb. They cover world and oblique construction planes, numeric and reference
input, negative and identity factors, zero rejection and zero-length references,
mode changes, completed and canceled defaults, overlapping groups, repeated
copies, grips and selected parents. Sources include points, lines, arcs, circles,
rational and periodic curves, rational surfaces and meshes.

Application tests make 99 complete or incremental replays. Comparisons include
complete geometry definitions, weights, knots, domains, mesh records, object
order, groups, selection, metadata, displayed grips and external Undo/Redo, at
absolute epsilon `1e-9`. Independent Python equations check translations from
each native tight center. [Provenance](../scale-positions-provenance.json) records
the engine, private scheme and capture hashes.
The zero-reference macros retain separate EndCommand and post-script snapshots:
their trailing Cancel executes at idle and clears selection after completion.

These tests do not establish full parity. Real mouse picks, native preview
appearance, reference distance locks, calculator expressions, repeated reference
copies, all trimmed or subcomponent sources, cross-viewport behavior, restart
persistence, command-first pick order and history, and native performance
comparisons remain unverified.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_positions.json --scheme VibocerosOracleScalePositionsFresh --output /tmp/scale-positions.json --timeout 240
cargo test -p viboceros-command scale_positions --lib
cargo test -p viboceros --bin viboceros scale_positions
python3 -m unittest tools.rhino_oracle.test_scale_positions
```

Use a fresh private scheme beginning with `VibocerosOracle`. The bounded capture
rejects unknown fields, requires idle execution in an empty owned document and
uses the dedicated Xvfb wrapper. It does not use the shared desktop profile.
