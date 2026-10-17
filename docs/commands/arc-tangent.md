# Arc Tangent

Start `Arc Tangent` (`A Tangent`), or choose `Tangent` at Arc’s first prompt.
Pick locations on curves or surface edges. Choose `Point` before each free
point. Three contacts determine a supporting circle; Enter accepts two.
Then pick the suggested arc to create it. The final pick selects a direction
around the circle and is projected into its plane.

With two curves and Enter, the second picked tangent location stays fixed
while the first contact slides. `FromFirstPoint`, after the first curve,
holds its picked location instead. A positive radius after the first contact
fixes the size and advances to arc choice after the second contact. After
two curve contacts, `Radius` opens a numeric getter; Enter uses its saved value.

For three contacts with exactly two straight guides, those guides supply
ordered endpoints. The final pick chooses the short or long arc between
them; the remaining curve or point determines the supporting circle.
Otherwise, the pick chooses the interval between neighboring contacts.
Changing guide order can change start/end order and curve direction.
An unused contact can lie outside the chosen interval.

Two free points and Enter give a supporting diameter circle. With a free
point followed by a radius and another free point, the last point supplies
the center direction; arc choice then selects a semicircle.

```text
Arc Tangent Curve=<id> 1.6,0,0 Curve=<id> 0.6,1.3,0 Enter 0.5,0.3,0
Arc Tangent Curve=<id> 3,0,0 3 Curve=<id> 0,4,0 1,1,0
Arc Tangent Curve=<id> 3,0,0 FromFirstPoint Point 0,4,0 0,1,0
Arc Tangent Point 3,0,0 Point 0,4,0 Enter 0,0,0
Arc Tangent Edge=<id>,<index> 3,0,0 Curve=<id> 0,4,0 Enter 1,1,0
```

Circle and Arc share contact input, immutable source snapshots, viewport
location queries and the supporting-circle solver. Arc caches that circle
for its final preview. It creates one `Arc` history entry, preserves sources
and selection, and adds no temporary model objects. Cancellation at either
stage creates no geometry or history. Changed, unavailable or deleted sources
and changed document tolerance reset the contact picker.

The saved Tangent radius starts at 1 and survives model Undo/Redo. Ordinary
Arc construction leaves this value unchanged. Point input uses the common
world/CPlane/relative, snap, filter and constraint pipeline. The construction
retains its plane, and final arc choice uses the fitted circle’s plane.

[Validation](../arc-tangent-validation.json) compares 38 successful native
workflows and two cancellations through command and frontend paths on both
backends. Full NURBS controls, weights, knots, domains, lengths, source
immutability, selection and actual Undo/Redo use absolute `1e-6` comparisons.
Curved guides include circles, circular arcs and NURBS lines, with reordered
guides, spatial planes and saved radii. Surface-edge routing has local coverage.

Seven native endpoint-recovery cases exceed `1e-6` and remain explicit
compatibility exceptions. Supporting-circle agreement does not guarantee the
same recovered endpoints near a tangency; these captures differ by roughly
`1.76e-6` to `2.8e-6`, sometimes changing the native NURBS span count. Viboceros
retains the solved tangent contacts. Original observations and failing
comparison evidence are preserved; the threshold is unchanged.

Arc’s Tilted, Deformable and Extension options remain pending. Exhaustive
branch, snap, viewport-aperture and performance parity are unqualified.

Reference: McNeel’s public [Arc command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/arc.htm).
