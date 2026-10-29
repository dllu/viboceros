# SubD custom face data

SubDs expose per-face material channels, colors and custom three-dimensional UVW
coordinates through the geometry API. These values remain attached to native
face IDs across archive readback, transforms and document history.

`face_texture_points()` follows the same ID-sorted face and oriented corner order
as `faces()`. An absent face array means no custom points; an absent corner means
an unset or nonfinite native value. The original archive retains incomplete
native metadata. `texture_coordinate_type()` reports the public OpenNURBS texture
source, including packed, unpacked and face-point modes.

`try_with_face_texture_points()` accepts unique original face ordinals and either
a complete finite UVW array or `None` to clear a face. Array lengths must equal
the corresponding corner counts. Invalid indices, duplicate targets, malformed
arrays and nonfinite values reject the complete edit. Empty edits return the
original value. Input is bounded to 100,000 face edits and 400,000 corners.

Setting any array activates `FromFaceTexturePoints`; clearing arrays leaves the
texture source unchanged. Spatial transforms leave UVWs in texture space. Face
color and material setters operate independently of these points.

## Merge behavior

Actual Rhino MergeFaces outputs establish the following behavior:

- Rewritten merged faces clear material channels, colors and custom texture
  points, including equal materials and fully coherent or constant UVWs.
- Unchanged regions keep their data. Surviving neighbors retain their stored
  corner array; when cleanup reduces their edge count, only the remaining prefix
  is active. This follows native storage behavior rather than remapping values
  to vertices by position.
- The texture source stays unchanged by the merge. Sequential commands and both
  Undo/Redo steps restore the captured properties and geometry.

Rhino's preparation/evaluation of an untextured document seed activates Packed
mapping when its authored source was Unset. The audit permits that specific
normalization only while importing such seeds. Operation results and custom UVW
texture sources are compared exactly.

## Validation

The [validation receipt](subd-face-custom-validation.json) binds current code,
authored input archives and two immutable Rhino capture families: 52 workflows,
54 merge invocations and 214 native 3DM models. Inputs cover nonzero and maximum
material channels, explicit face/edge/vertex picks, missing texture arrays,
neighbors, disconnected regions, holes, no-ops, coherent UVWs, winding, a pentagon
beside a quad and public translation, reflection, scale and shear transforms.

The IO audit reads Rhino's saved outputs with public OpenNURBS, independently
replays the edits and compares IDs, tags, loops, control/limit points and sampled
oriented surface facets. Geometry uses `1e-7`; stored UVWs, material channels,
colors and texture sources are exact. Application tests replay all workflows and
compare geometry/property states after commands, Undo and Redo. The corpus also
runs after explicit SMLib initialization.

```sh
cargo test -p viboceros-geometry --lib subd::texture_points
cargo test -p viboceros-io --test subd_face_merge
cargo test -p viboceros-io --test subd_face_merge --features native-smlib
cargo test -p viboceros --bin viboceros app::merge_faces::
python3 -m unittest tools.rhino_oracle.test_subd_face_custom
```

[InsertPoint qualification](subd-insert.md) covers allocating edge splits, incomplete
UVW corners and disconnected-pick rejection.

Material-table references, texture image rendering, arbitrary nonmanifold regions,
other editing commands and comparative performance need further work. The new API
handles stored corner coordinates; it does not certify those other behaviors.
