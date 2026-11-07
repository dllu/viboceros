# ArcBlend curved contacts and numeric input

[ArcBlend](arc-blend.md) · [Handles](arc-blend-handles.md) · [Radius targets](arc-blend-radius.md)

Open arcs, cubic and rational NURBS, polycurves and polylines can supply blend
contacts. Endpoint handles slide along the original bounded guide. Typed
points project onto that guide; clicks and held drags use its viewport
constraint. Moving a contact keeps the originally selected end orientation.
Trim retains the corresponding source portion, and Join combines the source
portions with the blend in the same undo step.

Arc parameters differ from the parameters of their rational display curves.
The editor converts viewport constraint parameters back to the original guide
before evaluating its point and tangent. `Parameter1` and `Parameter2` refer
to the effective guide domain, including arcs inside polycurves and
[temporary portions](arc-blend-subcurves.md).

Radius and ratio getters, and their scripted options, accept fractions,
functions, arithmetic and length suffixes using current model units. Examples
include `RadiusDifference=0.5cm`, `RadiusDifference=(1+1)/400m` and `Ratio=1/4`.
A final length suffix qualifies the complete expression. As in Rhino, ratio
entry also performs length-unit conversion. Invalid or nonfinite local input
keeps the accepted preview and remembered values.

[Validation](../arc-blend-sources-validation.json) retains 44 native
observations: 35 matched ArcBlend workflows, two Line point-input workflows,
and seven input-driver/rejection diagnostics. Both builds check coefficients,
geometry samples, requested targets, warnings, attributes, selection,
undo/redo and 74 combined 3DM round trips. Circular outputs also check 3,498
independent native point/tangent stations. Run:

```sh
python3 -m tools.rhino_oracle.check_arc_blend_sources
```

Rhino captures run in private Xvfb; application and egui tests are windowless.
Free mouse contact comparisons cover arcs and arc polycurves in Top view.
Temporary portions have their own qualification.
Other pointer trajectories, B-rep edge contacts, general continuous
distance, same-circle internal split parity and comparative speed remain
unqualified. Earlier qualification records retain their original scopes.
