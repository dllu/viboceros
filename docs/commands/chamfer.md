# Chamfer

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/chamfer.htm)

Select two open curves whose chosen ends are straight, then run
`Chamfer 0.5 1`. The distances are measured from the intersection along the
first and second selected curves. The command trims or extends those ends,
adds a straight bevel, and joins the result into a selected polycurve. Undo
restores both sources.

`Pick1=x,y,z` and `Pick2=x,y,z` choose the participating ends; otherwise the
nearest pair of endpoints is used. `Join=No` creates two retained source
curves and a bevel. `Trim=No` retains the originals and adds only the bevel.
The bevel inherits the first source's attributes and groups. Separate
retained curves inherit their respective sources' attributes and groups.

The current geometry solver supports straight terminal segments, including
terminal segments of polycurves. It rejects chamfers that consume either
entire terminal segment or use curved terminal segments. Zero distances on both
sides produce a sharp join when trimming is enabled.

The current tests cover exact analytic line geometry and command Undo. A live
Rhino output comparison for this command is still pending.
