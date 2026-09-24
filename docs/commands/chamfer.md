# Chamfer

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/chamfer.htm)

Select two open curves, then run
`Chamfer 0.5 1` or `Chamfer Distances=0.5,1`. The distances are measured from the intersection along the
first and second selected curves. The command trims or extends those ends,
adds a straight bevel, and joins the result into a selected polycurve. Undo
restores both sources.

`Pick1=x,y,z` and `Pick2=x,y,z` choose the participating ends; otherwise the
nearest pair of endpoints is used. `Join=No` creates two retained source
curves and a bevel. `Trim=No` retains the originals and adds only the bevel.
The bevel inherits the first source's attributes and groups. Separate
retained curves inherit their respective sources' attributes and groups.

Straight terminal segments may extend to their supporting-line intersection.
Already meeting arcs and NURBS terminal segments are trimmed by arc length
and retain their native geometry. Earlier polycurve leaves are preserved.
The solver rejects chamfers that consume an entire terminal segment and
nonmeeting curved terminal segments. Zero distances on both sides produce a
sharp join when trimming is enabled.

The current tests cover analytic line and arc geometry, NURBS preservation,
and command Undo. A live Rhino output comparison for this command is still
pending.
