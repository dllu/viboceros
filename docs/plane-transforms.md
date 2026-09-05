# Construction-plane transforms

[Transform commands](commands/transforms.md) · [Construction-plane primitives](construction-planes.md)

`Rotate`, `Mirror`, `Scale2D`, `Shear`, and `ProjectToCPlane` use the active
construction plane through the explicit `CommandContext`. One-line point
arguments remain world coordinates; interactive typed points resolve in the
viewport receiving them. The default Rust command context is World XY.

For example, in Front view select an object, enter `Rotate`, then `0`, `1,0`,
and `0,1`. Rotation takes place in XZ about the -Y axis. `Mirror` accepts an
axis start/end, while `Scale2D` and `Shear` accept center/reference/target picks.
The numeric angle or scale-factor forms remain available as one-line commands.

## Reference rules

| Command | Construction and reference policy |
| --- | --- |
| Rotate | Axis through the supplied center, along the plane normal. Picked angles use projected reference directions. |
| Mirror | Mirror plane through the first axis point, perpendicular to the construction plane. The second point's normal displacement is ignored. |
| Scale2D | Scale along plane X/Y about the supplied center; retain normal displacement. Picked factors use the ratio of full **3D** distances, including normal-only references. |
| Shear | Shear tangentially to the plane; retain normal displacement. The picked angle is spatial, signed by the plane normal. A tilted reference contributes an additional obliquity factor. |
| ProjectToCPlane | Project onto the actual plane, including its translated origin. |

For Shear, let `u` be the unit spatial reference vector and `h` the length of
its projection onto the construction plane. The effective shear coefficient
is `tan(angle)/h`, acting along the perpendicular in-plane direction in
proportion to displacement along the projected reference. A reference with
zero projected length is rejected. The angle uses `atan2` of the cross-product
length and dot product, avoiding loss of accuracy near parallel references.

Interactive Rotate/Mirror/Shear retain the plane captured at the first accepted
pick. Scale2D uses the viewport where the factor is supplied, as specified in
[Rhino's Scale2D workflow](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/scale.htm).
Projection uses the active viewport at execution, matching
[ProjectToCPlane](https://docs.mcneel.com/rhino/8/help/en-us/commands/projecttocplane.htm).

Transforms retain object identity in-place; `Copy=Yes` retains the original
selection and creates unselected copies with attributes and copied group
topology. ProjectToCPlane instead retains input by default and accepts
`DeleteInput=Yes`. Each completed command is one undo step. Invalid arguments
and unsupported degenerate geometry are rejected atomically.

## Verification and limits

`plane_transforms.json` compares 140 actual Rhino command executions on World,
Front, Right, translated, and oblique planes. Four affinely independent point
witnesses determine each 3D affine map; copy counts, identities and selection
are also checked. The comparison uses absolute/relative `1e-9`/`1e-12`.
The maximum measured coordinate difference is `7.11e-15`.
These are transform-map comparisons, not exhaustive representation comparisons
for every curve, surface, mesh, or collapsed B-rep. Native tests separately
cover command transactions, groups, attributes, rejected picks, and view changes.
The oracle restores its plane/selection and deletes only owned objects on
success, insertion failure, command failure, and recording failure.

`plane_transform_diagnostics.json` retains four parallel-reference cases.
Rhino introduces a roughly `3.7e-8` displacement for one exactly parallel 3D
reference pair. With parallel projected directions but differing heights,
its shear sign can change under a rigid rotation of the construction plane,
producing a much larger discrepancy. Native Shear preserves parallel identity
and uses the positive branch when the signed cross product of unit projected
directions is within eight machine epsilons of zero. Projection cancellation
is checked per dot product, so heavy tilt does not erase an otherwise resolved
turn direction. These diagnostics are not claimed to
agree at the ordinary comparison epsilon.

Custom viewport planes, Mirror's axis/three-point options, repeated interactive
copies, rigid Shear, scalar constraints during point prompts, and singular
projections that collapse validated geometry remain incomplete. Other transforms
and arrays still have their individually documented World-coordinate policies.
