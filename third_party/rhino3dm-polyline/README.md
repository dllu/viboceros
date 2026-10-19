# Rhino3dm polyline reference

Public MIT-licensed C# source from [McNeel/rhino3dm](https://github.com/mcneel/rhino3dm/tree/068baee9e853929139af486fc22a6da7c730fb2f).
Only `Polyline.ReduceSegments` is used as a reference for the Rust reduction in
`crates/viboceros-geometry/src/polyline/reduction.rs`. The Rust implementation
uses an explicit stack, existing robust segment queries, and rejects invalid
collapsed outputs. The reference is retained unmodified for provenance and is
not compiled or required at runtime. See `LICENSE` and `provenance.json`.
