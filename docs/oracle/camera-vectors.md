# Stored camera vectors

[Camera qualification](cameras.md) · [Draft analysis](../commands/draft-angle-analysis.md)

The [35-case request](../../tools/rhino_oracle/fixtures/camera_vectors.json) and
[frozen Rhino 8 observation](../../tools/rhino_oracle/observations/camera_vectors.json)
were captured in an owned private Xvfb window. Producers are archived by SHA-256
alongside the observation.

Parallel, perspective and two-point cameras retain their raw direction and up
vectors, including length and skew, through native named-view 3DM serialization.
The supplied scales range from `1e-8` to `1e48`. Pan and zoom retain these values;
World Top and Plan replace them with the standard unit axes.

Four acknowledged right-button drags qualify camera axes at 0.3° per pixel.
Perspective orbits yaw about world Z, pitch about the rotated screen right, and
replace both vectors with the resulting unit axes. Two-point orbits yaw about the
initial screen up, normalize the direction, and retain the locked raw up vector.
Their recomputed orthonormal frame gives that up vector priority. The raw direction
can consequently differ from the optical frame's forward axis.

Viboceros keeps raw metadata independently of the rendering frame. Picking uses
the rendered frame. Named views, view Undo/Redo and app SaveAs/Open preserve the raw
values. Offscreen egui drags replay all four native orbit results across several
frames, evaluating each gesture from its initial camera.

Four native draft-panel View choices confirm that the pull vector is the negated
raw camera direction. Its length affects dot-product colors; it is not implicitly
normalized. App replays cover this control after named-view save/restore and verify
that active getters, model geometry and redo remain intact.

The [18 camera archives](../../tools/rhino_oracle/fixtures/camera_archive.json)
retain the actual native 3DM bytes and cached camera axes, including two-point
frames after orbit. [Two supplemental archives](../../tools/rhino_oracle/fixtures/camera_archive_projection.json)
provide samples in front of the skew locked cameras; one original camera puts all
fixed sample points behind it. Both observations include public native projected
coordinates, named-view restores and direct projection installation.

Rhino's named-view restore preserves the cached frame. Its public
`SetViewProjection` instead rebuilds the frame from the raw vectors. The 3DM bridge
retains the cached right/up/viewward axes separately; authored records may omit
these axes to derive a frame from their raw vectors. `ThreeDmNamedView::camera_frame`
provides the common optical frame used by import and command picking.

File checks cover repeated saves, unit conversion, and atomic rejection of invalid
frames. App replays cover native named-view import/restore, SaveAs/Open, frame axes
at `2e-12`, projected picking coordinates at `1e-8` pixels, and displayed coordinates
within f32 rounding. This qualifies the supplied vectors, frames and projections.
It does not establish native navigation speed, clipping updates, or every orbit's
camera position/frustum.

```sh
cargo test -p viboceros --bin viboceros camera_vectors
cargo test -p viboceros --bin viboceros camera_archive
cargo test -p viboceros-io --test camera_archive
python3 -m unittest tools.rhino_oracle.test_camera_vectors tools.rhino_oracle.test_camera_archive
```
