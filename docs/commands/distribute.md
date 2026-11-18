# Distribute

[Transforms](transforms.md) · [Oracle](../oracle.md)

```text
Distribute XAxis Mode=Gap Spacing=Automatic
Distribute YAxis Mode=Center Spacing=3
Distribute ZAxis Spacing=-2
Distribute Direction 0,0,0 1,2,3 Mode=Center
```

Start `Distribute` with three selected objects, groups, or grips, or
pick them when prompted. The options menu offers `XAxis`, `YAxis`, and
`ZAxis` in the active construction plane. Each axis applies the layout
immediately. `Direction` takes two viewport picks or typed points and
previews the layout while you place the second point. Typed world coordinates
can define a spatial direction.

Set `Mode` to `Gap` or `Center`. Open `Spacing` to enter a signed distance,
measure it with two points, or choose `Automatic`. Both direction prompts
allow changes to Mode and Spacing. Successful commands remember these
settings. Escape cancels the preview and discards pending option changes.
Complete positional commands still accept options as shown above.
[Rhino Distribute](https://docs.mcneel.com/rhino/8/help/en-us/commands/align.htm#Distribute)

Grips count as individual stations. Only selected grip positions move, and
coincident raw vertices belonging to a picked mesh topology vertex move
together. The preview uses the applicable grip basis and the final edit keeps
object IDs, layers, groups, and geometry metadata.

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

Objects move in place. Preselected inputs retain their selection. With
command-first picks, stationary objects stay selected after applying the layout.
Undo and Redo release the command-first picks. Every bound, displacement,
and replacement geometry is staged before one atomic edit. Too few units,
invalid options, unresolved rational poles, bounds-budget exhaustion, and
unrepresentable results leave geometry and Undo history intact.

## Geometry and verification

The independent `viboceros-command::distribute` module shares the
geometry-local `object_bounds` query with [BoundingBox](bounding-box.md).
Its private `planner` submodule owns interval spacing and displacement
calculation, with focused ordinary, overflow, and subnormal regression tests.
It receives finite intervals in leading-edge order after command validation
has established at least three units; it does not access the document.
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

Run this fixture with its explicit comparison limits (the runner defaults are
stricter and are not this fixture's acceptance limits):

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/distribute.json --absolute-epsilon 1e-8 --relative-epsilon 1e-12 --timeout 240
```

A fresh Rhino 8 run against native revision `fffcc02` reconfirmed all 188
comparisons after the spacing-overflow fixes, with maximum coordinate error
`3.829159211932165e-10`. At the runner's default limits, four curved-NURBS
placement cases fail; these established fixture limits were not changed.

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

Native spacing-planner tests also cover finite automatic spacing across
opposite-sign extreme endpoints and subnormal point intervals. Midpoints avoid
premature underflow; differences are scaled before subtraction when the raw
difference overflows but the averaged spacing is representable. Unrepresentable
final displacements remain errors. This verifies the interval planner, not
end-to-end Rhino parity or representability of every preceding bounds/frame
calculation at these extremes.
Gap planning advances from the translated far endpoint using a compensated
sum of endpoint, offset, and spacing; it does not require the interval width
itself to be representable. A four-interval regression covers a stationary
interval spanning opposite extreme coordinates, with automatic and explicit
zero gaps. No unused next position is computed beyond the final interval.

An independent integer-reference test checks 36,864 spacing plans for 3, 5,
and 9 units at scales `2^-500`, `1`, and `2^500`. It derives gap layouts from
total/prefix widths and center layouts from exact integer centers, rather than
repeating the planner's running-position algorithm. Cases include overlapping
and zero-width intervals, automatic spacing, and signed explicit spacing.
The chosen power-of-two denominators make all reference answers exactly
representable; these checks do not assert correct rounding for arbitrary inputs.
