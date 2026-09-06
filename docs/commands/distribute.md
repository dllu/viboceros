# Distribute

[Transforms](transforms.md) · [Oracle](../oracle.md)

```text
Distribute XAxis Mode=Gap Spacing=Automatic
Distribute YAxis Mode=Center Spacing=3
Distribute ZAxis Spacing=-2
Distribute Direction 0,0,0 1,2,3 Mode=Center
```

With at least three selected objects/group units, distribute them along an
active CPlane axis or a two-point direction. The defaults are `Mode=Gap` and
`Spacing=Automatic`; settings are not sticky. Options are case-insensitive and
accept `Name=Value` or `Name Value`, including underscore-prefixed tokens.
These implement the [documented Distribute options](https://docs.mcneel.com/rhino/8/help/en-us/commands/align.htm).

In the UI, enter `Distribute` (optionally with settings) or
`Distribute Direction Mode=Center` to pick/type two direction points. Ordinary
viewport picks use the construction plane and drafting aids; explicit world
points may define a fully 3D direction, as observed in Rhino. Coincident points
keep the prompt open for correction; Escape cancels without editing geometry.
There is a direction guide, but no live layout preview or sticky option state.

## Layout and groups

Units are ordered by their tight bounding-box minimum along the distribution
direction, not by their center. Exact leading-coordinate ties compare the two
transverse minimum coordinates in a right-handed direction frame. X uses
CPlane `(X,Y,Z)`; Y uses `(Y,-X,Z)`; Z uses `(Z,X,Y)`. For a typed direction,
the transverse guide is `CPlane Z × direction`, falling back to CPlane Y when
parallel. Fully identical minima retain selection order; that final tie policy
is deterministic natively, but is not established as a Rhino compatibility rule.

- `Gap` equalizes signed edge-to-edge gaps. Negative gaps allow overlap.
- `Center` equalizes bounding-box-center spacing, not geometric centroids.
- Automatic spacing keeps the first and last units fixed.
- An explicit finite spacing, including zero or a negative value, keeps the
  first unit fixed and lays out every subsequent unit from it.

Each object's last ordered group membership determines its rigid unit.
Overlapping groups are not merged into connected components. Only selected
members participate; unselected members remain unchanged. This top-group rule
is checked against actual Rhino commands with nested, overlapping, duplicated,
and partially selected groups, including adding an object to an older group
after a newer one. The document stores each object's membership order;
3DM import/export and undo/redo preserve it. Adding an existing membership is a
no-op, not a reorder. See [groups](../groups.md) for the model, copied groups,
and the separate ordered-membership oracle fixture.

Objects move in place, retaining identity, attributes, layers, group membership,
and selection. Every bound, displacement, and replacement geometry is staged
before one atomic undo transaction. Too few units, invalid options, unresolved
rational poles, bounds-budget exhaustion, or an unrepresentable result fails
without partial geometry or an undo entry.

## Geometry and verification

The independent `viboceros-command::distribute` module shares the
geometry-local `object_bounds` query with [BoundingBox](bounding-box.md).
Translation precedes rotation, avoiding cancellation from a distant CPlane
origin. It uses tight curve, surface, and retained trimmed-face bounds, not
control nets or display meshes. Their existing [numerical limits](../trimmed-face-bounds.md)
still apply. Gap accumulation uses compensated summation, and unit sorting is
O(n log n). Document lookups and geometry queries have separate costs.
No performance parity with Rhino is claimed by these untimed probes.

`distribute.json` contains 188 actual command comparisons: all six world planes
and an oblique CPlane, signed/zero spacing, 3D direction points, seven curve
representations, surfaces, trimmed B-reps, mesh/point-cloud vertices, group
rules, and 57 additional tie-order cases. The comparison epsilon is absolute
`1e-8`, relative `1e-12`; maximum passing coordinate error is `3.83e-10`.
Records include every source's retained ID, selection,
current layer, domains, sampled points, and groups.

Six comparisons remain explicitly failing in `distribute_diagnostics.json`:
oblique quadratic-surface and disk-face placement, and a transformed disk's
world-Z placement, in both modes. Maximum placement differences are about
`0.000280`, `0.000179`, and `0.000782`, respectively. Independent closed-form
quadratic extrema, including interior stationary points and circular boundaries,
check the native translations; the epsilon is not widened to match Rhino's
less accurate bounds.

Native tests additionally cover exact spacing equations, transverse-coordinate
preservation, distant CPlane origins, near-normal directions, undo/redo, and
atomic failures. Python tests check macro whitelisting, fresh versus stale
failure reports, and cleanup after initialization, insertion, grouping,
execution, recording, and inspection failures. The batch oracle rejects fewer
than three preselected objects before touching Rhino, since that case opens an
interactive selection prompt; fewer than three logical group units with at
least three selected objects is tested against Rhino's completed rejection.

A private GUI smoke test used an oblique CPlane, four named point clouds, and
one group. Automatic center spacing, automatic gap spacing through the two-point
prompt, and explicit spacing all survived 3DM export with maximum coordinate
error below `5.2e-15`; undo/redo and retained names, layers, and groups were checked.
