# Point-ended Loft

[Loft usage and architecture](loft.md)

```text
Loft StartPoint=0,0,-1 EndPoint=0,0,3
```

`StartPoint=x,y,z` and `EndPoint=x,y,z` taper either end to a world-space point.
One curve and one point suffice; both point ends are optional. Inputs remain
selected and unchanged, and the output has fresh attributes on the current layer.
The operation is one undo/redo step. `Natural`, `SplitAtTangents`, and the global
crease preference retain their ordinary Loft meanings.

Point ends support Normal, Loose, Tight, and Straight. Uniform point-ended lofts
and combining point ends with `Closed=Yes` are rejected before document mutation.
A closed profile with two point ends produces a closed B-rep; a single point end
leaves the other rim open. Open profiles produce sheets with singular boundaries.
Poles share the grid's topology rather than introducing coincident mesh vertices.
The sampled closed-solid normalizer also covers solids closed by poles, including
reversed profile traversal. Collapsed boundaries reuse tessellation's sampled
normal limits on several meridians; all outside probes still have to agree.
Numeric options are available through the command line; interactive Point picking
is still incomplete.

`LoftEndPoints` and `try_loft_nurbs_curves_with_end_points` expose the ordered
geometry API. The usd-brep adapter delegates its basis edits and interpolation
through `loft_surface_with_end_points`. Real profiles first share their basis and
weight policy; point rows then inherit their adjacent prepared profile's weights.
This keeps collapsed curves out of region matching. Pole rows count toward the
256-section limit. Shared fitting, knot, and numerical limits still apply.

The [official Point workflow](https://docs.mcneel.com/rhino/8/help/en-us/commands/loft.htm)
and [ordered SDK endpoint parameters](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFromLoft.htm)
describe the native feature used as the reference.

## Validation and native differences

Two immutable public Rhino 8.32 API captures contain 150 cases with one through
four profiles and either or both point ends. All 96 returned surfaces that honor
the requested poles match both backends within `1e-9`, including all coefficients,
weights, full knot vectors, domains, and 289 surface points. Source definitions
remain unchanged. Tests also exercise large/small model units and common weight
gauges, early limits, malformed options, closed pole topology, meshing, command
entry, and undo/redo. [Source and capture bindings](loft-point-ends.json) retain
the exact scope; these samples do not certify continuous surface error.

Rhino returns no surface in 50 records, including all 30 Uniform cases. In four
linear-profile records it returns the ordinary loft and ignores the requested end
point. Those four strict numeric comparisons remain explicit ignored diagnostics;
the implementation honors the supplied point. Another 20 linear records return
no native surface, while the kernel constructs point-ended surfaces. These are
behavior differences, with no claim of complete native parity. The captures
exercise the ordered API; native interactive Point selection is not replayed.

```sh
cargo test -p viboceros-geometry --test loft_points_native
cargo test -p viboceros-smlib --features native --test loft_points_native
cargo test -p viboceros-geometry --test loft_points_native -- --ignored
```

The last command reproduces the four native endpoint-omission differences.
