# Bend

[Command reference](README.md) · [Geometry and oracle](../bend-geometry.md)

Select objects, run `Bend`, pick the spine start and end, then pick a through
point off the spine. Starting without a selection prompts for objects first;
Enter accepts the selection. Typed coordinates, snaps and point filters use
the common drafting input path. Complete scripted invocations use:

```text
Point 2,1,5
SelAll
Bend 0,0,0 0,0,10 10,0,10 Angle=0 LimitToSpine=Yes
Undo
Bend 0,0,0 0,0,10 10,0,10 Angle=45 Copy=Yes
```

| Option | Behavior |
| --- | --- |
| `Copy=Yes/No` | Keep the sources and create copies, or replace their geometry. Interactive copies always use the original sources; Enter or Escape keeps accepted copies and finishes. One Undo reverses the batch. |
| `Rigid=Yes/No` | Place objects without deforming them, using their bounds centers. Selected peers in the same top group share one placement about their combined bounds. |
| `LimitToSpine=Yes/No` | Fix the circular region's arc length to the spine length, or construct the circle from the through point. Set this before choosing a positive Angle. |
| `Symmetric=Yes/No` | Also bend geometry behind the spine start, reflecting the axial deformation. |
| `PreserveStructure=Yes/No` | Move existing Euclidean NURBS controls while retaining degree, knots, weights and domains. Polysurfaces always use fitting and hide this option. |
| `NonAttenuated=Yes/No` | Use uniform circular deformation, or the measured angular attenuation inside the center of curvature. |
| `Angle=0…360` | Choose a fixed angle in degrees. The point supplies its plane; with LimitToSpine enabled, radius is spine length divided by angle. Zero restores through-point construction. |

Flags initially default to No. During interactive input, use `Name=Yes/No` or
choose an option name and answer its prompt. `Angle` opens a numeric prompt;
Enter recalls a saved number when available. Complete scripts use `Name=value`.

## Remembered choices and history

LimitToSpine, Symmetric, NonAttenuated and numeric Angle edits are remembered
immediately, including when the command is canceled. A saved positive angle
also affects the next Bend's geometry before its Angle option is chosen,
although Rhino displays the initial option as the word `Angle`. Use `Angle=0`
to explicitly restore through-point construction and clear that numeric default.
Undo, Redo and a new document retain these preferences.

Copy, Rigid and PreserveStructure are saved when the whole command succeeds.
For repeated Copy, Enter saves the current choices, including edits after the
last placement. Escape retains accepted geometry and the previous defaults for
these three flags. Copy also follows [RememberCopyOptions](remember-copy-options.md).

Names, colors and group memberships survive replacements and copies. Copies
remain unselected, with the sources selected. Successful Copy batches release
source selection during Undo/Redo. A canceled batch from preselected objects
retains the current selection during replay. With preselected in-place Bend,
Undo restores source selection; Redo releases it unless explicitly reselected.
Command-first picking uses the common source-selection cleanup policy.

Geometry is staged before any source changes. Invalid spines, on-spine through
points, collapsed mesh facets and exhausted fit budgets fail atomically.
`Angle=0` still bends geometry; it does not mean an identity operation.

## Measured compatibility and limits

Private Xvfb captures from Rhino **8.32.26160.13001** cover 76 geometry commands:
points, lines, cubic curves, a surface, solid boxes and colored quad meshes;
all flags, repeated Copy, reversed/spatial spines, fitting tolerances and rigid
frame boundaries. Rust replay checks geometry, names, colors, groups, selection,
Undo and Redo. Two native workflows cover 41 preference/history steps, replayed
through the application prompts with terminal geometry and selection snapshots.

Points and preserved controls use epsilon `1e-11`, rigid placements `1e-7`, and
native float mesh positions `1e-6`. Fitted samples use
`2 * max(document absolute tolerance, 1e-5)`, allowing each independent fit its
tolerance. Native definitions stop refining below `1e-5`; the command applies
that floor while the geometry kernel retains its explicit tolerance contract.
See the [provenance record](../bend-command-provenance.json).

## Live preview

After accepting the spine, mouse movement previews object-colored wires beside
the selected sources. Wireframe includes surface isocurves; Shaded and Ghosted
show pending boundaries and keep the source faces. An on-spine cursor removes
the pending bend. The free mouse plane passes through the spine start. Preview
and completion use the same effective numeric angle. See the
[preview evidence and cache design](../bend-preview.md).

Validation is sampled, with finite fitting budgets; it is not a continuous
error certificate or exhaustive Rhino parity. The command-first path has Rust
UI tests, while the native workflows use preselected objects. Arbitrary
construction planes, complex trimmed shapes and angles beyond one turn
remain unverified or unimplemented.

```sh
cargo test -p viboceros-command bend::tests
cargo test -p viboceros --bin viboceros app::tests::bend
python3 -m unittest tools.rhino_oracle.test_bend_command
```
