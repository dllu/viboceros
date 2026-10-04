# Maelstrom

[Command reference](README.md) · [Point map](../maelstrom-geometry.md)

Complete command invocations deform selected objects around a center and the
resolved first circle normal:

```text
Point 3,1,5
SelAll
Maelstrom 0,0,0 2 5 90
Undo
Maelstrom 0,0,0 0,2,7 -5,0,9 -90 Copy=Yes
```

The two radii can be numbers or comma-separated world points. A numeric first
radius retains the active construction plane. A first radius point uses its full
distance from the center and orients the circle through that point, projecting
the preferred construction-plane normal perpendicular to its radial direction.
A direction parallel to that normal uses construction-plane Y as the circle
plane's second direction. The first radius must exceed `2^-32`, matching the native Circle size getter. The second
radius accepts signed numbers by magnitude; a second radius point uses radial
projection onto the resolved circle plane. A point on the circle axis is
rejected, while a numeric second radius at or below that
cutoff leaves points unchanged, matching the command's invalid SDK morph path.
Coil angles are degrees and can be negative, zero, or multiple turns.

| Option | Behavior |
| --- | --- |
| `Copy=Yes/No` | Copy geometry or replace the sources. Copies preserve names, colors and corresponding cloned group memberships. Sources stay selected and copies are unselected. |
| `Rigid=Yes/No` | Place each object using its bounds center without changing its shape. Selected peers in a top group share the placement about their combined bounds. |

Maelstrom does not expose PreserveStructure. Curves and surfaces use the shared
tolerance-driven fitters. Fitted commands use an absolute tolerance floor of
`1e-5`; point and mesh maps retain the document tolerance. B-reps fit each shared
edge once, retain UV trims and validate the resulting topology. Zero angles and
zero/tiny second radii retain the existing point, curve, surface and mesh shapes;
line results use an equivalent degree-one NURBS representation.

Geometry is staged before replacement or copying. Invalid first radii, malformed
arguments, invalid mesh images and exhausted fitting limits fail without changing
document geometry, attributes, selection, groups or history. Replacements record
an Undo entry even when geometry is unchanged. Undoing copied groups leaves their
empty definitions, matching the native captures.

## Preferences and continuation APIs

Rigid starts at No for each command. Copy starts at No and follows
[RememberCopyOptions](remember-copy-options.md). Native Copy getter edits save
immediately, including later cancellation; the command-specific
`remember_maelstrom_copy_option` API provides this policy for an input session.
The first radius initially defaults to 1 and saves when accepted, even if a
later prompt is cancelled. `maelstrom_radius_default` and
`remember_maelstrom_radius` store that value outside document history. Undo,
Redo and a new document retain preferences.

Complete invocations validate their syntax before accepting the first radius.
Once that radius and the Copy choice are accepted, a later radius, angle or
fitting error rolls back the document while retaining those getter preferences.

`deformed_geometries` supports prepared sources, Circle radius points and construction
planes. Repeated native Copy placements use the original sources and both radii,
vary only the coil angle, and form one Undo batch; command adapters can replay
them through `execute_in_history_group` with explicit `Sources`.

The application currently supports complete typed invocations. Interactive source
selection, staged circle/radius/angle getters, repeated Copy input, Circle's
alternative construction modes and live previews remain to implement. The
preference and history APIs above are foundations for those workflows.

## Retained verification

Six fixture sets retain 85 owned commands and 29 preference/history steps from
Rhino **8.32.26160.13001**, collected on private Xvfb displays. They cover points,
lines, NURBS curves and surfaces, solid boxes, colored quad meshes, Copy and Rigid,
grouped/repeated placements, equal/reversed radii, spatial/parallel first-radius plane construction and projected second-radius picks,
construction-plane normals, signed and multi-turn angles, rejected first radii,
zero/tiny second radii, unchanged-result history and a fitting tolerance sweep.
The [provenance record](../maelstrom-command-provenance.json) hashes recipes,
observations and helpers.

Point comparisons use epsilon `1e-11`, rigid placements `1e-7`, mesh coordinates
at least `1e-6` for the native single-precision vertex buffer, and fitted
curve/surface samples `2 * max(document absolute tolerance, 1e-5)`. B-rep
patches use evaluated witnesses in both directions, constraining boundary
samples to matching edges. A 486-point native SDK/ClosestPoint diagnostic
measures tangential parameter differences up to `8.74e-5` while physical
distances remain below `1.89e-6`; these checks therefore compare geometric
patches rather than assuming identical UV fitting correspondence. Names, colors,
group membership and command selection/history are checked separately. Radius
diagnostics append Cancel to end a potentially rejected getter; when it already
succeeded, that extra command owns the later selection cleanup.

These are sampled witnesses, not a continuous error certificate or exhaustive
Rhino parity. Complex trimmed shapes, other Circle construction choices,
interactive point-driven coil angles and original native performance comparisons
remain unverified.

```sh
cargo test --release -p viboceros-command maelstrom::tests
python3 -m unittest tools.rhino_oracle.test_maelstrom_command
```

Public reference: [McNeel Maelstrom help](https://docs.mcneel.com/rhino/8/help/en-us/commands/maelstrom.htm).
No proprietary code was inspected.
