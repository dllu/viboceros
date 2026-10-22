# SubD control chains and sharpness

[SubD geometry](../subd.md) · [Soft weights](subd-soft-crease.md)

`SubD::try_edge_chains` partitions a unique selection of control edges using
the public OpenNURBS chain sorter. Each result has directed edge ordinals and
control-vertex ordinals. Closed chains repeat their first vertex. Branches split
chains; disconnected selections produce separate chains. Sorting ignores the
input selection order. Native component marks stay on a private decoded copy.

The query accepts boundary edges as well as interior edges. A `SubDCrease`
caller must restrict its selection to edges with two incident faces. Duplicate
or out-of-range ordinals reject without changing the source.

The sharpness helpers retain the native conversion and interpolation rules:

```rust
let chains = subd.try_edge_chains(&selected_edges)?;
let chain = &chains[0];
let start = SubD::soft_crease_sharpness(80.0)?;
let weights = SubD::interpolate_edge_sharpness([start, 0.0], chain.edges.len())?;
let edits = chain.edges.iter().zip(weights).map(|(edge, w)| {
    let w = if edge.reversed { [w[1], w[0]] } else { w };
    (edge.index, w)
}).collect::<Vec<_>>();
let (edited, changed) = subd.try_with_stored_edge_sharpness(&edits)?;
```

Percent conversion returns an unrounded double in native `0..=4` units.
Interpolation uses control-edge count, then applies the public native factory
to each interval. Its output consists of accepted float values promoted to
doubles. Applying those stored values preserves them exactly, including values
near an integer that would otherwise snap for a second time. Arbitrary doubles
that are not native float representations reject in the stored-value setter.

Explicit interpolation can itself sanitize an existing near-integer endpoint.
Its interior arithmetic still uses the original endpoint value. This differs
from restoring an internal snapshot, which preserves the accepted shape.

The [qualification](../subd-chains-qualification.json) checks topology, chain
counts and immutable sources against 48 native workflows with 50 source objects.
Unit regressions cover paths, loops, branches, disconnected selections,
orientation, invalid input, quantization and stored-value restoration.

The captures also preserve behavior needed for the command editor: per-chain
options, partial hard/weighted sources, repeated edits, odd loops, cancellation
and numeric retries. Moving labels are picked at positions measured from prior
native outputs. General profile construction, document admission and the
interactive `SubDCrease` command remain subsequent work.
The [validation record](../subd-chains-validation.json) lists the completed
kernel, replay, Python and lint checks.
