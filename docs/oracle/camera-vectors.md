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

This qualifies the supplied vectors and camera axes. It does not establish native
navigation speed, clipping updates, every orbit's camera position/frustum, or cached
locked-frame persistence after serialization of an already-orbited skew camera.

```sh
cargo test -p viboceros --bin viboceros camera_vectors
python3 -m unittest tools.rhino_oracle.test_camera_vectors
```
