# SimplifyCrv

Select curves and run `SimplifyCrv`, or start the command, pick curves, and press
Enter. Other object types are ignored. Geometry is prepared before any source
is replaced, so failure leaves the document and redo history intact.

The command recognizes linear and circular NURBS forms, splits NURBS at fully
multiple interior knots, removes constant rational weights, combines collinear
lines and co-circular arcs, and assembles consecutive linear pieces as polylines.
It retains ordinary nonsimplifiable curves, including backtracking polylines.
Closed polylines retain their seam. Degree-one NURBS use their exact control polygon directly; polyline reductions
reuse one native predicate buffer. Parent domains and retained polyline vertex
parameters are preserved. Rebuilding an analytic form can change parameter
speed while preserving the geometric locus within the recognition tolerance.

Object identities, attributes, groups and attribute user text are retained.
Geometry-root user text is retained for no-ops and constant-weight normalization;
reconstructed representations get fresh geometry data, as in the native command.
Preselected objects stay selected. Changed command-first inputs finish unselected,
including after Undo/Redo; command-first no-ops retain their picks. Changes make
one undo step. No-ops preserve the redo branch.

Both builds use the public OpenNURBS `ON_Curve::IsLinear` and `IsArc` predicates
with the document's absolute tolerance. `native-smlib` uses usd-brep's existing
exact splitter at fully multiple knots. The portable build uses the existing
Rust splitter. Neither path fits a replacement spline. Simplification is bounded
at 65,536 leaf pieces and one million controls of preparation work.

The current workflow accepts whole curves. Temporary `SubCrv` inputs and general
G1 join tuning remain pending. Near-tangent line/arc no-ops are covered by the
reference captures; that does not establish compatibility for every tangent
cleanup configuration. The command does not certify global curve fairness or
continuous approximation error independently of the recognition predicates.

The retained 41 actual Rhino commands cover lines, rational arcs and circles,
ellipses, closed and backtracking polylines, fully and partially multiple knots,
constant weights, mixed representations, tolerance boundaries, custom domains,
preselection, command-first input, mixed curve/point selection, batches, metadata,
and history. Replay compares classes, full NURBS definitions, child/parent domains,
polyline vertex parameters and sampled geometry at `1e-8`, plus selection through
Undo/Redo. Shared native 3dm sources preserve the actual inputs.

```sh
cargo test -p viboceros-command --test simplify_curves
cargo test -p viboceros-command --features native-smlib --test simplify_curves
cargo test -p viboceros app::tests::simplify_curve
cargo test -p viboceros-geometry --lib curve_simplify
```

See [Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/simplifycrv.htm),
the [public simplification API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_Curve.htm),
and [validation record](../simplify-curve-validation.json).
