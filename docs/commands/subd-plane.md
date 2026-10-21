# SubDPlane

`SubDPlane` creates an open rectangular subdivision grid on the active
construction plane. Its boundary vertices have crease tags, so the limit
surface rounds the four control-cage corners inward.

```text
SubDPlane 0,0,0 4,3,0
SubDPlane XCount=2 YCount=3 Center 1,2,3 3,5,3
SubDPlane 3Point 1,2,3 4,6,3 0,5,5
SubDPlane Vertical 1,2,3 4,6,3 0,5,8
```

The default is two faces in each direction. Set `XCount` and `YCount` at the
first prompt, or enter a positive integer to set both. A leading `0` picks the
current CPlane origin. Counts are remembered independently of SubDBox, including
on cancellation. The command completes when the rectangle is finished.

`Center`, `3Point`, `Vertical`, numeric dimensions, and `AroundCurve` share the
Rectangle getter. `3Point` can also be chosen after the first corner. AroundCurve
selects a curve or surface edge and a center on it; the rectangle is perpendicular
to the guide. Scripts use `AroundCurve Curve=<id> center corner`, or
`Edge=<id>:<index>` for an edge. Script coordinate triples are world coordinates;
interactive input also supports the shared CPlane, world, relative, unit,
constraint and snap forms.

The rectangle preview stays outside the document. Escape cancels while keeping
Redo. Changed history or model tolerance invalidates a pending draft. Completion
adds one unselected SubD on the current layer with one Undo record. Existing
SubD display, control-point edits, transforms and 3DM interchange apply.
An oversized grid ends the draft without changing the model or consuming Redo.

The installed Rhino `8.32.26160.13001` rejects `EdgeMidpoint` for this command,
although the [help page](https://docs.mcneel.com/rhino/8/help/en-us/commands/subdbox.htm)
lists it. Its command finishes immediately after rectangle admission; counts
sent afterward are separate unknown commands. Three diagnostic captures retain
those raw outcomes, including fallback geometry.

The [qualification report](../subd-plane-qualification.json) retains 19 native
captures: 15 successful constructions, cancellation, and three diagnostics.
Command and interactive getter comparisons check topology, winding, tags,
vertex limit stations, sampled density-two surfaces, source preservation,
attributes, selection and grouped history at `1e-6`. AroundCurve coverage uses
one straight guide. General curved guides, surface-edge frame parity, native
face packing, texture/material channels and comparative speed need further
qualification. Sampled facets carry no continuous Hausdorff certificate.
Construction is limited to 100,000 control vertices.
