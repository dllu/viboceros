# Closed-source Fillet

[Command reference](commands/fillet.md) · [Validation record](fillet-closed-validation.json)

`Fillet` now supports distinct planar closed curves using picked tangent contacts.
Closed parents stay complete. Trim retains the picked side of an open partner;
Join combines that retained piece with the fillet arc. An open first source keeps
its ID, attributes and groups. A closed first source stays in place and gives its
attributes to a new, ungrouped polycurve. Without Join, new arcs use the current
layer with fresh attributes. Retained lines preserve their parameter charts.

The existing tangent-circle solver finds contacts from source parameters and picks.
Open NURBS trimming uses the selected portable/usd-brep backend. A prepared edit
plan supplies both dynamic preview and atomic admission, including source edits,
new objects and deletions. Cancellation preserves geometry and pending Redo.

Two immutable licensed-Rhino captures contain 16 complete workflows: 15 successful
edits and one dynamic cancellation, with 32 acknowledged physical source picks and
2,442 public point/tangent stations. Cases cover circle/line and circle/circle
combinations, internal/external and upper/lower contacts, reversed source order,
a rational NURBS circle, an ellipse, a tilted spatial plane and dynamic radius edits.
Replays use pre-command curve/sight-line witnesses; construction-plane projection
can pick the wrong branch on a tilted source. Native outputs are expectations only.

Both builds check all 15 explicit commands and 16 UI workflows against Native
coefficients, domains, samples, metadata, identity, selection, Undo/Redo and 3DM
archives. A separate windowless mouse test checks circle/line selection, an
unmodified parent and exact preview/commit geometry. Existing open-curve fillet
tests, tangent-solver tests and shared SubD checks run alongside the new cases.

Two failed attempts published no observations. Embedded Python rejected the first
pilot's relative import. A later perspective holdout projected two picks only four
pixels apart and opened a Selection Menu. Its owned Rhino PID and settings scheme
were verified before termination. The complete holdout set was repeated, using
Top view for the spatial case. Frozen producers, requests, logs and the private
display inspection are retained separately from successful captures.

```sh
python3 -m tools.rhino_oracle.check_fillet_closed
```

All captures use private Xvfb displays; application tests create no windows.
The model gate is 1e-6. These are sampled finite cases, without a continuous distance,
pixel-equality or comparative performance claim. Self-source fillets, zero radius
with closed sources, nonplanar closed curves, corner contacts and extension of
open curved partners remain unqualified. The current closed-source solver rejects
radii at or below the document's absolute tolerance.
