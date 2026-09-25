# GCon

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/gcon.htm)

Select two open curves and enter `GCon`. The nearest pair of ends is checked by
default. `Pick1=x,y,z` and `Pick2=x,y,z` identify other ends. The command reports
endpoint gap, oriented tangent angle in degrees, and curvature-vector difference.
It does not change geometry or create an undo step.

The continuity result is `disconnected`, `G0` for position only, `G1` for
position and tangent, or `G2+` when curvature also matches. The plus sign means
the current kernel does not distinguish G3 and G4. Position and angular checks
use document tolerances. The curvature comparison uses a length-scaled absolute
tolerance and a relative tolerance. Five analytic oracle fixtures cover line
orientation, a kink, a gap, and opposite arc bends; live Rhino verification of
this command remains pending.

```text
GCon
GCon Pick1=0,0,0 Pick2=4,0,0
```
