# Compound BooleanIntersection

[Polyhedral commands](polyhedral-boolean-commands.md) · [Exact regions](polyhedral-booleans.md#exact-intermediate-regions)

The measured two-set command first combines each populated set, then processes
its oriented closed shells separately. This can produce overlapping output
objects. Results of different shell pairs are retained independently.
Common intersection constructs odd/even material, then exports participating
connected boundaries. See [oriented pairs and common intersection](compound-pairs.md)
for the follow-up two-input and multiple-input evidence.

## Combining each set

Inputs whose entire boundary lies inside another input's material are pruned
when they have no proper boundary interaction. Equality retains the earlier
input. Boundary enclosure differs from complete volume containment: a box can
surround another object's cavity while all its own boundary lies in material.
The native set union discards that box rather than filling the cavity.

The remaining inputs are combined mathematically. Interaction participation is
resolved per original shell. A connected group of interacting original objects
exports only result shells containing an interacting original source face;
untouched shells of a consumed object can disappear. Independent objects retain
all their shells. Inward orientation survives this intermediate stage.

Attributes come from the first boundary contributor in the original-object
union group. Geometry user text comes from the last contributor if that group
exports one shell; it is cleared when the group exports multiple shells.
Disconnected parts of the same original object share this metadata decision.
A separately supplied island inside a cavity retains its own metadata.

One `BrepPolyhedralBooleanPlan` holds exact membership throughout pruning,
union, shell splitting and pair operations. Only final exports round Cartesian
and UV coordinates. Original supporting surfaces and face ownership survive.
Equivalent coplanar original-face aliases remain available for participation
checks even when exported faces have another canonical owner. Export ownership
is restricted to the original faces participating in the current shell pair;
coverage is certified for every retained patch.

## Oriented pairs

| Shell orientations | Measured policy |
| --- | --- |
| Outward / outward | Intersect when boundaries interact. For strict nesting, copy the enclosed shell with its intermediate metadata. Disjoint regions produce nothing. |
| Inward / outward, or outward / inward | For interaction, subtract the inward shell's finite enclosure from the outward region, using first-set metadata. Disjoint shells copy the outward region with its own metadata. Strict nesting produces nothing. Coincident nested boundaries are rejected. |
| Inward / inward | Crossing and face/edge contact union the finite enclosures; nesting copies the enclosing shell. Equality fails, while disjoint and point-only contact produce nothing. See the [follow-up capture](compound-pairs.md). |

A reversed single shell is normalized. Compound orientations inconsistent with
material nesting remain uncertified. Common intersection has a separate
participation and metadata policy, documented with its follow-up evidence.

Metadata is also cleared when one pair operation produces multiple components.
Original object IDs, layers, colors, attribute user text, geometry user text,
groups, selection, DeleteInput, Undo and Redo are checked by command replay.

## Captures and limitations

The [41-recipe fixture](../tools/rhino_oracle/fixtures/compound_intersection.json)
and [raw observations](../tools/rhino_oracle/observations/compound_intersection.json)
contain nine public SDK signed-pair calls and 32 interactive command cases.
All live execution used an empty owned document, private Xvfb, and a fresh
private Rhino settings scheme. SDK unions of each input set are retained as
diagnostics and never become command inputs. Source records before and after
SDK calls guard against source mutation.

The command cases include both mixed-set directions, input order, nested
islands, disconnected members, simultaneous shell contacts, preselection,
retained inputs, common intersection, no-result outcomes and failures. Four
cases record Undo/Redo. EndCommand object records, post-script records, command
history, null versus empty SDK responses and native failure markers are kept.

One edge-contact union produces a native open B-rep diagnostic and an interactive
failure that adds two `!` text dots. Viboceros rejects the singular intermediate
union and preserves the original objects and history, but does not add those
markers. The raw marker records remain in the capture and are asserted by replay.
Native Failure versus Nothing event codes are retained; local command errors do
not reproduce every native failure classification.

The original 65-case polyhedral command capture now has 65 physical replays,
including the non-solid two-hole intersection through the boundary exporter.
The historical compound replay
compares 31 complete physical/metadata command records and separately asserts
source preservation and the two-marker discrepancy for the remaining failure.
One new seam/count difference is asserted: `first_extra_cross` has 12 faces and
30 edges locally, versus native 10 faces and 24 edges. Physical geometry and
metadata still match. This difference is recorded in
[compound partitions](compound-intersection-partitions.json), separately from
physical comparisons. See [provenance](compound-intersection-provenance.json).

These finite boundary witnesses do not prove continuous error bounds, arbitrary
curved Boolean parity, general compound contributor behavior, native component
ordering or native performance. Limits remain 128 original inputs, 128 shell
pairs, 4,096 cumulative exported patches and one cumulative exact work budget.

```sh
cargo test --release -p viboceros-command boolean_solids::tests -- --nocapture
python3 -m unittest tools.rhino_oracle.test_compound_intersection

# Always choose a fresh private scheme for live work.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/compound_intersection.json --scheme VibocerosOracleCompoundExample --timeout 900 --output /tmp/compound-native.json
```
