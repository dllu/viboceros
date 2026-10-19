# MarkFoci

Select curves and run `MarkFoci`, or start the command, select curves, and press
Enter. `DrawAxis=Yes` adds a line between ellipse or hyperbola foci, or from a
parabola's vertex to its focus. Circles and circular arcs produce one center
point. The axis option starts at Yes and persists in the command registry,
including after cancellation and Undo/Redo.

```text
MarkFoci
MarkFoci DrawAxis=Yes
```

Sources retain their identity, representation, attributes, groups and geometry
user text. Added points and lines have fresh attributes on the current layer.
Sources remain selected; new annotations are unselected. Undo/Redo releases
command-first picks and retains preselection. One invocation makes one undo
step; inputs that produce no annotations preserve redo history.

The kernel reuses the ellipse recognizer's conditioned homogeneous coefficient
system. It recognizes whole conic loci across NURBS spans, including partial
arcs, degree elevation, knot refinement and composite representations. Each
span must pass planarity and conic residual bounds. It does not identify a
conic from a few sampled points. Hyperbola and parabola residual bounds are
converted to horizontal or vertical model-space distances. Numerically
ambiguous, degenerate, mixed-sign-weight or unresolved conics are skipped;
a tight model tolerance can reject an otherwise accurate representation.

The retained 58 actual Rhino commands cover circles, ellipses, hyperbolas,
parabolas, their arcs and composite forms, transformed/elevated/refined curves,
selection, metadata, option persistence and Undo/Redo. Fifty-seven cases match
at `1e-8` absolute epsilon. One perturbed multi-span ellipse is deliberately
skipped: Rhino marks an approximate fit, while Viboceros requires the whole
curve to pass its conic checks. Axis orientation and the order of the two foci
are compared without ordering; their direction or insertion order can differ
from Rhino. The recognition bounds and sensitivity guards use floating-point
arithmetic and do not certify an exact conic predicate or focus-error interval.

The [validation record](../mark-foci-validation.json) retains source hashes,
test results and compatibility limits. Additional kernel tests vary degree,
weights, domains, reversal, refinement and spatial placement. These bounded
comparisons do not establish general Rhino equivalence or performance.

```sh
cargo test -p viboceros-geometry --test conic_foci
cargo test -p viboceros-command --test mark_foci
cargo test -p viboceros app::tests::mark_foci
python3 -m unittest tools.rhino_oracle.test_mark_foci_probe
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_mark_foci
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/markfoci.htm)
