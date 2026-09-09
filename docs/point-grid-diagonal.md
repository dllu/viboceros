# PointGrid Diagonal investigation

[PointGrid command](commands/point-grid.md) · [Oracle setup](oracle.md)

Typed native Diagonal construction now reproduces eight measured cases from
Rhino `8.32.26160.13001`. Interactive Diagonal prompting remains deferred.

## Observed command behavior

Changing `XCount` removed `Diagonal` from the current first-point prompt;
entering `_Diagonal` afterward produced `Unknown command`. The recorded
[command history](../tools/rhino_oracle/observations/point_matrix_diagonal_count_edit.txt)
shows the option list before and after that count edit. This is evidence for
that tested command sequence, not every Rhino version or selection path.

The diagnostic sets remembered counts by creating and deleting one owned
ordinary grid, then starts a fresh `PointGrid Diagonal` without editing counts.
Each grid operation restores the construction plane and selection and removes
only its own geometry, using the existing scoped command helper.

Four successful cases establish:

- Non-coplanar corners completed without an additional height input, including
  a second corner below the first.
- Coplanar corners accepted an explicit world-space height point, including a
  negative height and a base elevated above World XY.
- In these World XY cases, X varies fastest, followed by Y and Z, with every
  axis directed from the first input toward the opposite corner or height point.
  Reversed X corners therefore produce descending X points. The ordinary grid's
  normalized corner order must not be copied into Diagonal mode.

Numeric-only height inputs `-2` and `2` left the coplanar command at `Height:`
in the observed trials. That does not distinguish rejection from a distance
constraint awaiting a direction point. The diagnostic uses `height_point` and
rejects numeric `height` to avoid knowingly incomplete scripts.

## Reproduce and validate

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_diagonal.json --timeout 180
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/point_matrix_diagonal_planes.json --timeout 180
python3 -m unittest tools.rhino_oracle.test_point_grid_diagonal
```

The `point_grid_diagonal_prompt` operation is supported by both oracle engines;
it compares source order directly, without lattice sorting. The four-case live
comparison passed with zero coordinate difference.
The [raw response](../tools/rhino_oracle/observations/point_matrix_diagonal_prompt.json)
retains all 54 points in Rhino's source order. An independent Python test compares
them exactly with four analytic directed lattices. A native replay also compares
ordered numeric coordinates exactly. Worker tests check separate
count initialization and world-coordinate height serialization.

An additional [oriented-plane response](../tools/rhino_oracle/observations/point_matrix_diagonal_planes.json)
records 48 points across World XZ, World YZ, an oblique CPlane, and a coplanar XZ
base with a height point. Native source order matched in every case; maximum
coordinate difference was `4.5e-16` (zero for the axis-aligned cases). Independent
Python world-coordinate formulas and native ordered replays validate these
measurements at `1e-12`, without canonical sorting.

Still unverified: the near-coplanar auto-completion threshold, numeric height
and Enter semantics, exhaustive CPlane orientations, and all diagonal corner permutations.
These need evidence before native prompt behavior can be claimed compatible.
Native typed construction currently uses an exact computed zero normal component
to require a height point; it does not infer Rhino's prompt tolerance.
