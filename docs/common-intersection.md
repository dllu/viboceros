# Ordered common intersection

[Command](commands/boolean-intersection.md) · [Convex kernel](convex-booleans.md) · [Compound inputs](compound-pairs.md)

An empty second selection set makes `BooleanIntersection` process the first set
in order. Preselection uses document table order; command-first picking and
explicit `FirstSet=` arguments preserve their supplied order. The mathematical
geometry API continues to compute the common region independently of these
command policies.

## Measured reduction

For ordinary inputs with one closed shell, the command keeps an exact current
region and processes each subsequent original input:

- A strictly enclosing input without boundary interaction leaves the region and
  metadata contributors unchanged.
- A strictly smaller input without boundary interaction replaces the region and
  takes its own metadata. The command succeeds only if an earlier or later stage
  has a proper boundary interaction.
- Equal intermediate and next regions fail. A final empty intersection produces
  no replacement. Both paths leave geometry and history unchanged.
- Proper crossings, partial coplanar overlap, and opposing area contact compute
  the exact intersection. Point and edge contacts alone do not establish success.

Metadata contributors also follow the reduction. When a complete next input is
retained, earlier contributors survive only when their original boundaries
interact with it. When the next input is partly cut, earlier contributors survive
only when they do not contain its whole finite region. The next input is then
appended. The first surviving contributor supplies attributes, layer, color,
groups, name, and object user text. A single output takes geometry user text from
the last surviving contributor; multiple outputs clear that text.

For example, two crossing boxes followed by a strictly smaller box inside their
intersection produce a copy of the smaller box. Selecting the smaller box before
those two enclosing boxes produces no replacement. An enclosing input can occur
anywhere without supplying output metadata. Boundary contact can preserve some
earlier contributors while removing others. These are observations of public
commands; the reducer contains no recipe-name branches.

Compound inputs retain their separately measured
[boundary participation policy](compound-pairs.md). The ordinary reducer does
not replace that policy. Closed inward results are normalized outward;
non-solid compound boundaries retain their measured winding.

## Exact construction

`BrepConvexBooleanPlan` certifies original convex inputs once. Its opaque regions
retain exact rational polygons and borrow original supporting faces. Containment,
boundary interaction, and repeated intersection share one exact work budget.
Intermediate regions are never exported and recertified as rounded operands.
Nonconvex single-shell inputs use the existing exact polyhedral arrangement with
the same command reducer. Arithmetic, resource, and export failures propagate;
only an unsupported convex input certificate selects the polyhedral path.

Final export rounds model and UV coordinates once and validates topology. Region
handles from different plans are rejected. Limits include 128 inputs, two million
exact work units, 8,192 bits per checked rational, and 4,096 cumulative exported
patches. Geometry, metadata, and contributor decisions are staged before document
edits. Existing input retention, selection cleanup, option memory, and one-step
Undo/Redo apply.

## Retained evidence and limits

The [80 closed recipes](../tools/rhino_oracle/fixtures/common_participation.json)
and [raw observations](../tools/rhino_oracle/observations/common_participation.json)
were captured with Rhino 8.32.26160.13001 in an empty owned document at idle, on
private Xvfb with a fresh private settings scheme. Sources are outward boxes,
constructed through the public SDK. The reusable runner records original inputs,
command macros, histories, EndCommand snapshots, post-script snapshots, metadata,
face boundaries, and four Undo/Redo recipes. SDK unions are diagnostics and never
supplied as command inputs. See [provenance](common-intersection-provenance.json).

Rust command tests reconstruct every source independently and compare all 80
outcomes, including failed-operation atomicity, retained IDs, selection, complete
metadata, mass properties, and finite bidirectional boundary witnesses. These
records have no output face/edge count differences. History recipes replay twice.
Application tests exercise actual ordered picking and document-order preselection.
Kernel tests additionally verify staged nonrepresentable rational cuts, original
surfaces, independent containment expectations, empty regions, foreign-plan
rejection, and resource limits.

Volume and centroid epsilon is `1e-10`, area epsilon is `1e-9`, and boundary
witness epsilon is `1e-7`. Finite witnesses do not certify a continuous error
bound. Native Failure/Nothing classifications are retained but local error
classes are not identical. General nonconvex command policy, curved inputs,
arbitrary contributor configurations, component ordering, and native performance
remain unproven. Full Rhino parity is not claimed.

```sh
cargo test --release -p viboceros-command boolean_solids::tests::common
cargo test --release -p viboceros-geometry convex_plan
cargo test --release -p viboceros boolean_intersection
python3 -m unittest tools.rhino_oracle.test_common_participation

# Use a new private scheme for each live run.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/common_participation.json --scheme VibocerosOracleCommonExample --timeout 900 --output /tmp/common-rhino.json
```
