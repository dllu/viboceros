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

`ExtendArcsBy=Arc|Line` chooses a circular continuation or a tangent line
for nonmeeting arcs. `ExtendOtherCurvesBy=Line|Smooth` chooses a tangent line
or a smooth continuation for nonmeeting NURBS. The defaults are `Arc` and
`Line`. The selected ends are connected first; each chamfer distance is then
measured along the connected curve from the meeting point. Native arc and
NURBS pieces are retained when the chosen continuation supports them.
Earlier polycurve leaves are preserved. Zero distances on both sides produce
a sharp join when trimming is enabled.

The solver rejects chamfers that consume an entire connected source curve.
A setback may cross a tangent extension and continue into the original arc or
NURBS. Smooth NURBS extension currently supports the curve pairs handled by
Connect; rational smooth extension can differ from Rhino.

The current tests cover analytic line and arc geometry, smooth NURBS length
setbacks, and command Undo. A live Rhino output comparison for this command
is still pending.
