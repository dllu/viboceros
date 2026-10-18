# FilletSrf

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/filletsrf.htm)

With `native-smlib`, run `FilletSrf` and pick two single-face surfaces on the
sides to keep. Click inside each face, or type `object-id u,v` using the
surface's original parameter coordinates. The second pick must belong to a
different object. The viewport shows a temporary result after both picks.
Change options, press Enter to accept, or Escape to cancel. Acceptance is one
undo step; picking and previews leave the document and its history untouched.

Options can be entered before either pick or while previewing:

| Option | Default | Behavior |
| --- | --- | --- |
| `Radius=positive` | `1` | Constant circular rolling-ball radius in model units |
| `Extend=Yes\|No` | `Yes` | Extend along the longer planar parent, or use the common extent |
| `Trim=Yes\|No\|Split` | `Yes` | Retain picked sides, preserve both parents, or keep all split pieces |
| `Join=Yes\|No` | `No` | Stitch the fillet and changed parent pieces; requires `Trim=Yes` |
| `BlendType=Arc` | `Arc` | Circular fillet; other blend types are pending |

`Radius 2`, `Trim Split`, and a bare positive radius also work in the interactive
prompt. A construction failure clears the preview; change options to recover,
or cancel. Changing a source or relevant document settings invalidates a cached
preview before acceptance.

For scripts, supply both pick coordinates and `Sources=id,id`, or preselect
exactly two sources. Explicit sources/picks bypass the interactive prompt.
`Preview=Yes` runs construction without changing the document.

```text
SrfPt 0,0,0 10,0,0 10,10,0 0,10,0
SrfPt 5,0,-5 5,0,5 5,10,5 5,10,-5
FilletSrf Sources=1,2 Pick1=0.2,0.5 Pick2=0.2,0.5 Radius=1 Trim=Split
Undo
FilletSrf Sources=1,2 Pick1=0.2,0.5 Pick2=0.2,0.5 Radius=1 Trim=Yes Join=Yes
```

Without joining, changed parents retain their IDs, attributes, groups and
geometry user text. Extra split pieces inherit that metadata. The new fillet
uses the current layer and is selected. A joined component inherits the lowest
ordered contributing parent's ID and metadata; parents absorbed by that
component are removed. Joining may leave separate components when their edges
do not meet. Parents the native operation leaves unchanged are preserved.
Changed parents keep their [original UV ranges](../fillet-parent-charts.md).
Existing rectangular faces also retain their full underlying source support.

Supported inputs are NURBS surfaces and single-face B-reps with rectangular
trims. Polysurface faces must first be detached with `ExtractSrf`. General curved
trim loops, selection among disconnected fillets, deformable/G2 blends, and
history-linked regeneration are pending. Curved `Extend=Yes` is declined because
its extents still differ from Rhino; use `Extend=No`. Closed-cylinder transfer
can also hit the existing bounded certification limit.

Construction and parent trimming delegate to usd-brep. See the
[native producer](../native-surface-fillets.md),
[pick-side qualification](../native-surface-fillet-picks.md), and
[parent-region qualification](../native-surface-fillet-parents.md) for the
measured scope of Rhino parity. These checks do not establish general FilletSrf
compatibility.

[Command and application validation](../fillet-srf-validation.json) records the
regression runs and the source versions used.
