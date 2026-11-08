# Zero-radius Fillet beside closed sources

[Command reference](commands/fillet.md) · [Validation record](fillet-zero-validation.json)

For distinct curves containing a closed parent, radius zero intersects the curves
or their continuations and retains the picked portion of the open partner.
It inserts no arc. Closed parents remain complete. Join combines the retained
open portion using the existing source identity and attribute rules. Without
Join, the open source keeps its ID, attributes, layer and groups.

`Trim=No` is an immediate successful no-op, including for disjoint sources.
With Trim, contacts rank by summed unwrapped arc lengths from the two picks.
Line supports preserve their native charts. Circular, endpoint-tangent and smooth
NURBS continuation share geometry code with Chamfer; its existing mixed-style
guard remains in the Chamfer adapter. Endpoint contacts normalize within model
tolerance. A noncoplanar open line can trim to a genuine spatial intersection.

Trimming an already meeting open piece is an explicit replacement: it creates
an Undo entry and clears pending Redo even when the geometry is identical.
Prepared plans carry that replacement policy, so cached preview and acceptance
use the same candidate. True no-ops and cancellation preserve pending Redo.

Five immutable licensed-Rhino captures contain 83 observations: 40 discovery
workflows, three history controls, and a complete 40-workflow repeat. The controls
show that Undo/Redo commands can report Success with nothing to undo or redo.
The repeat therefore records public
[UndoRedo events](https://developer.rhino3d.com/api/rhinocommon/rhino.commands.command/undoredo)
and begin/end snapshots. It distinguishes 30 Undo records from 29 scene changes,
including one equal-geometry replacement. Each phase has its own matched serial;
Undo and Redo identify the same command but use different inverse records.

The final set includes 33 valid successes, four failures, one cancellation and
two successful commands that create invalid empty PolyCurves when joining only
closed parents. Those invalid outputs remain archived and audited. Viboceros
rejects those joins without altering the model or losing Redo; they have no
geometry-parity claim. One failed direct `RhinoDoc.Undo/Redo` observer attempt
published no observations. Its request, log and frozen source closure are retained.

The final capture contains 80 acknowledged physical source picks and 5,280 public
point/tangent stations. Both builds replay 33 explicit commands and 38 valid UI
outcomes; six API and two UI guards verify rejection purity and Redo preservation.
There are 74 3DM round trips per build. A windowless mouse test checks source
picking, exact preview/commit geometry, an untouched closed parent and one Undo.

```sh
python3 -m tools.rhino_oracle.check_fillet_zero
```

Captures use private Xvfb displays; egui/application checks open no windows.
Authored definitions and pre-command sight points supply inputs. Native outputs
serve only as expectations, with a 1e-6 model gate. This is finite coverage of
distinct pairs containing a closed source. [Coverage for two open curves](fillet-open-zero.md)
and same-parent behavior have separate fixtures. Arbitrary overlaps, tied contacts, folded continuations, rational
poles, nonplanar closed curves, tiny positive radii and corner contacts remain
unqualified. There is no continuous-distance, pixel-equality or comparative
performance claim.
