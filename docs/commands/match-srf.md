# MatchSrf

[Surfaces](surfaces.md) · [Matching geometry](../smlib-surface-matching.md)

`MatchSrf` changes one complete boundary of an open, untrimmed NURBS surface to
match another single untrimmed surface. Both portable and `native-smlib` builds
support position, tangency and curvature matching.

Enter `MatchSrf`, pick the source edge, then pick the target edge near the
corresponding end. Picking opposite ends reverses correspondence. The preview
is disposable: Enter accepts one undoable edit; Esc cancels and retains
pending redo. Source name, layer, color, groups and UUID survive. The accepted
source moves to the end of document order; Undo restores its original position.
Whole-object preselection clears before picking.

Options accept `Name=Value` or a name followed by a menu choice. Choosing
`Curvature` or `_Curvature` in a menu keeps `MatchSrf` active. See the
[completed menu replay](../match-surface-menu-validation.json).

Available options:

```text
Mode=Position|Tangency|Curvature
MatchByClosestPoints=Yes|No
RefineMatch=Yes|No
AverageSurfaces=Yes|No
IsocurveDirection=Automatic|MatchTarget|Perpendicular|Keep
```

`AverageSurfaces=Yes` supports position, tangency and curvature with refinement off. It
previews and accepts both surfaces atomically; see [averaging](../surface-averaging.md).

Defaults are curvature, opposite-end position preservation, automatic isocurve
direction, closest points off, refinement off and averaging off. Valid option changes survive
cancellation within the command registry. Separate registries start fresh.

In the captured Rhino 8.32 command, `PreserveOtherEnd` accepts `Nothing`,
`Position`, `Tangency` and `Curvature` but retains `Position`, including on quintic
inputs. Viboceros follows those command observations. The separate geometry API
supports all opposite-end policies; see [its qualification limits](../smlib-surface-matching.md).

Scripts can provide two explicit object/edge/pick triples before the options:

```text
MatchSrf source-uuid 1 1,0.25,0.0975 target-uuid 3 1.12,0.25,0.2375 Mode=Tangency
```

The original pick locations determine correspondence and survive ambiguity
choices. Source, topology, attributes, units or tolerance changes invalidate a
pending preview. Averaged curvature/refinement, curve/trimmed targets, closed boundaries, `OnSurface`,
`MultipleMatches` and custom matching tolerances remain pending. Refinement
uses document tolerances and can return outside the requested continuity bounds.
See the [native tolerance investigation](../surface-match-tolerances.md).

[Finite command validation](../match-surface-command-validation.json) covers 21
native outcomes and 16 direct API edits per build, physical viewport picks,
metadata, identity, selection, history and 3DM round trips at `1e-9`. Four
additional diagnostic rows are excluded, including one menu trace that timed
out after an incorrect option value. Full Rhino and performance parity remain
unqualified.
