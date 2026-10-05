# Commands on chained polyhedral solids

[Geometry construction](polyhedral-booleans.md) · [Union](commands/boolean-union.md) · [Intersection](commands/boolean-intersection.md) · [Difference](commands/boolean-difference.md)

The three Boolean commands use the convex kernel when its certificate succeeds,
then the general polyhedral kernel when the input is outside that certificate.
Arithmetic, work-limit, and output-topology failures propagate as errors. All
geometry and metadata are prepared before the document transaction changes objects.

Concave faces, face holes, straight spline edges from earlier results, separate
shells, cavities, and nested islands are accepted under the
[polyhedral input certificates](polyhedral-booleans.md#input-certificates).
Curved or open inputs, overlapping shells within one input, unsupported Cartesian
roundoff, and singular output topology remain outside this scope. General curved
Boolean parity and native performance are unproven.

## Geometry API and command policies

The geometry API defines material by odd/even shell containment and preserves
cavities in their enclosing material component. It now also exposes
`union_polyhedral_breps`, `intersect_polyhedral_breps`,
`intersect_polyhedral_brep_sets`, and `subtract_polyhedral_breps` for multiple
original operands. These expressions use one arrangement of original faces,
without rounded intermediate Booleans. Union reports material and boundary
contributors; set intersection reports contributing and maximal pair regions
within each output component.

Exact membership states on both sides of every arrangement boundary patch
certify these reports. Pair containment is checked within each output component;
no approximate centroid test decides whether one nonconvex pair region contains
another. Boundary and subtraction interaction queries are also available,
including positive-length contact for subtraction. Limits include 128 operands,
256 original faces per operand, 4,096 arrangement fragments, two million exact
work units, and 8,192 bits per checked rational.

Interactive commands additionally follow the measured native policies below.
These policies can produce a different region from the mathematical API.

| Command | Compound shell policy in the captured cases |
| --- | --- |
| Union | Split the constructed result into boundary shells, keep shells with an interacting original source face, and insert each as an outward object. Untouched shells within a consumed original can disappear. |
| Difference | Subtract interacting cutters per target, keep participating boundary shells, and insert each outward. A target with no interacting cutter is copied whole if another target succeeds. |
| Intersection, two objects | Intersect original shells separately. An inward shell within a compound input denotes its exterior; a single reversed input is normalized. Separate shell-pair results remain separate objects. |

Union clears geometry user text when a material result exports multiple shells.
Difference clears it when a target exports multiple pieces. Intersection clears
first-source geometry text when its compound pipeline produces multiple pieces;
an unchanged second-source result retains that source's metadata. Two-set
intersection on ordinary material inputs takes attributes from the first maximal
first-set contributor and geometry text from the last maximal first-set
contributor. Existing option memory, selection phases, deletion, and history
policies continue to apply.

Mixed inward compound shells with more than two accepted objects, or a pair of
compound inward shells, are not certified for native Intersection behavior and
return an error before editing the document. The mathematical multi-operand APIs
remain available for their independently defined material expressions.

## Retained native evidence

The closed probe uses public geometry setup and public interactive commands at
idle in an empty owned document. Live execution uses private Xvfb and a fresh
private settings scheme. Raw source records, macros, command histories,
EndCommand snapshots, post-script snapshots, metadata, and physical polygon
boundaries are retained in the
[65-recipe capture](../tools/rhino_oracle/observations/polyhedral_boolean_command.json).
The [fixture](../tools/rhino_oracle/fixtures/polyhedral_boolean_command.json)
includes concave and holed earlier results, cavities, nested islands, disconnected
shells, reversed inputs, multiple targets/cutters, multiple intersection sets,
selection modes, input retention, and Undo/Redo.

Three native successes remain outside the command certificate:

- `i_singular_two_holes`: Rhino returns a valid open B-rep; the kernel rejects
  the singular non-solid result.
- `i_cavity_first_multi`: Rhino returns two overlapping outward objects with
  volumes 2.25 and 3.75.
- `i_cavity_second_multi`: Rhino returns one solid of volume 5.0625. Its measured
  handling of the cavity differs from the preceding multi-object recipe.

Command tests replay the other 62 recipes with volume and centroid epsilon
`1e-10`, area epsilon `1e-9`, and finite bidirectional boundary witnesses at
`1e-7`. They check attributes, geometry user text, groups, retained source IDs,
selection, outward output orientation, and captured history twice. Source face
and edge counts agree. Seven output seam/count differences are asserted against
the [explicit partition record](polyhedral-command-partitions.json), while all
physical and metadata comparisons remain in force. The three uncertified
recipes check unchanged objects, selection, Undo, and a usable Redo entry.
Application tests cover actual picking and preselection of an earlier holed
result through all three commands, filtering, selection, and history.
See [capture provenance](polyhedral-command-provenance.json).

Compound pair construction additionally limits work to 128 shell pairs and
4,096 total output faces; each exact construction has the kernel work budget.

The latter two mixed-shell recipes are deliberately retained as unresolved
compatibility evidence. No coordinate-specific exception selects their results.
Continuous boundary error, arbitrary compound contributor configurations,
wireframe seam parity, and native component ordering remain unproven.

```sh
cargo test --release -p viboceros-command boolean_solids::tests -- --nocapture
python3 -m unittest tools.rhino_oracle.test_polyhedral_boolean_commands

# Choose a fresh private scheme for each live capture.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/polyhedral_boolean_command.json --scheme VibocerosOraclePolyhedralCommandsExample --timeout 900 --output /tmp/polyhedral-commands-rhino.json
```
