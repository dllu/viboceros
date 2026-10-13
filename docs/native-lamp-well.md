# Native lamp-well conversion regression

`crates/viboceros-smlib/tests/lamp_well.rs` retains the degree-5 × degree-3
front-fender master and 7 mm control-net thickness from the supplied
`LMP2-Evo-V3-Source-and-Validation/lmp2_v3.rs`. Its only second object is an
extruded rational ellipse in the tangent frame at UV `(0.13, 0.30)`.
The original `body-boolean-ellipse.rs` experiment is absent from that archive;
this reconstruction does not assert that its cutter matches the lost case.

The ordinary `(0.16, 0.085)` semi-axis cutters, with rotations 0 and 0.5 radians,
pass Boolean difference → portable B-rep → 3DM write/readback, with Boolean
postprocessing enabled and disabled. Readback checks closure, topology counts,
and spatial-edge samples; native volume decreases, and the original operand's
native properties remain unchanged.

The larger `(0.30, 0.16)` cutter rotated 0.3 radians crosses the panel boundary.
On the retained usd-brep `c9c5979` kernel it returns manifold topology but invalid
geometry. Direct **native** endpoint evaluation before copying or extraction
measured edge 16's start-to-vertex gap as `6.430754482986e-4` and edge 19's
end gap as `4.708436575046e-3`, against stored vertex and edge radii near `1e-9`.
These are model-space distances; UV loop gaps are a separate parameter-space
quantity. Increasing extraction tolerance cannot correct this native result.

The bridge now translates legal additive native vertex/edge zone radii into
local portable vertex bounds. It certifies each additional bound against the
original native radii, without moving geometry or relaxing B-rep validation.
Native endpoint gaps beyond those radii report their edge, endpoint, vertex,
distance, and tolerances. This fixes the tolerance contract; the larger-cut
native construction defect remains under investigation.

```sh
cargo test -p viboceros-smlib --features native --test lamp_well
cargo test -p viboceros-smlib --features native --lib plan_tests
```
