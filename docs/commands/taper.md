# Taper

[Command reference](README.md) · [Geometry and oracle](../taper-geometry.md)

Select objects, run `Taper`, pick the axis start and end, then enter or pick the
start and end distances. Starting without selection prompts for objects first;
Enter accepts the selection. Coordinates, snaps and point filters use the
common drafting input path. Complete scripted invocations use:

```text
Point 2,1,5
SelAll
Taper 0,0,0 0,0,10 2 1
Undo
Taper 0,0,0 0,0,10 0,2,0 1 Flat=Yes Copy=Yes
```

A distance can be a signed number or a comma-separated world point. Point picks
use distance from the axis, ignoring their axial coordinate; their distances
are positive. Zero and distances within the native zero cutoff are rejected.
Interactive distance prompts also accept explicit unit suffixes, such as `2cm`.

| Option | Behavior |
| --- | --- |
| `Copy=Yes/No` | Create copies or replace sources. Interactive copies always use the original sources and initial distance; enter more end distances, then Enter or Escape. One Undo reverses the batch. |
| `Rigid=Yes/No` | Place objects without deforming them, using their bounds centers. Selected peers in a top group share one placement about their combined bounds. |
| `Flat=Yes/No` | Scale one radial direction or both. A picked start distance supplies the flat direction. With a numeric start distance, use the intersection of the axis-normal plane and active construction plane, or construction-plane X when those planes are parallel. |
| `Infinite=Yes/No` | Continue the linear radius change outside the axis endpoints, or use cubic smoothstep between the endpoints with constant scale outside. |
| `PreserveStructure=Yes/No` | Move existing NURBS controls, retaining degree, knots, weights and domains. Polysurfaces always use fitting and hide this option. |

Flags initially default to No. At either distance prompt, use `Name=Yes/No`, or
choose an option name and answer Yes or No. Complete scripts use `Name=value`.
Neither distance has a remembered numeric default. Equal start and end distances
retain the existing geometry and structure, without fitting.

## Preferences and history

All five flags save on successful outer completion. Escape keeps accepted
copies and the previous defaults, including edits after the last placement.
Enter saves the current choices. Undo, Redo and a new document keep preferences;
Copy also follows [RememberCopyOptions](remember-copy-options.md).

Names, colors and group memberships survive replacement and copying. Copies
are unselected and sources remain selected. The captured preselected workflows
retain selection through Undo/Redo. Command-first source picking uses the common
transient selection policy; its continuation has application tests.

Geometry is staged before any document changes. Invalid distances or axes,
collapsed mesh facets and exhausted fit budgets fail atomically. Signed numbers
can reverse the radial coordinates; Infinite can cross zero scale. Geometry
that collapses at that crossing may fail validation.

## Measured compatibility and limits

[Live cursor previews](../taper-preview.md) share prepared display cages with
Twist and Bend. Radius guides and pending geometry follow the same resolved
point used for a click. Thirty private native captures constrain curve and
surface cages, grouped rigid placements, display modes and radius mouse planes.

Private Xvfb captures from Rhino **8.32.26160.13001** retain 90 geometry recipes
and 33 preference/history steps. They cover points, lines, curves, a surface,
solid boxes and colored quad meshes, all flags, repeated Copy, signed distances,
point picks, reversed/spatial axes, three construction planes, fitting tolerances
and radius boundaries. The [provenance record](../taper-command-provenance.json)
also retains probe precision diagnostics.

Points and preserved controls use epsilon `1e-11`, rigid placement `1e-7`, and
fitted samples `2 * max(document absolute tolerance, 1e-5)`. Native fitting
stops refining below `1e-5` in the retained tolerance sweep. The command applies
this floor independently of the kernel's explicit fitting tolerance contract.

Validation uses sampled witnesses and finite fitting budgets. It is not a
continuous error certificate or exhaustive Rhino parity. Additional axis
construction options and complex trimmed shape compatibility
remain pending. Native source workflows use preselection; command-first input
is checked in application tests.

```sh
cargo test -p viboceros-command taper::tests
cargo test -p viboceros --bin viboceros app::tests::taper
python3 -m unittest tools.rhino_oracle.test_taper_command
```
