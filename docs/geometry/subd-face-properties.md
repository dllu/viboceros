# SubD face properties

`SubD::face_properties()` exposes one `SubDFaceProperties` record per control
face, in the same native-ID order as `faces()` and `face_ids()`.
`try_with_face_properties` edits complete records on a private archive copy:

```rust
let edited = subd.try_with_face_properties(&[(0, SubDFaceProperties {
    color: Some([200, 40, 70, 0]),
    material_channel: 17,
})])?;
```

Colors use RGBA with OpenNURBS transparency: alpha 0 is opaque and 255 is
transparent. `None` inherits the object color; `[255; 4]` is reserved for unset
and must be supplied as `None`. Material channels range from 0 to 65535;
zero uses the object's material. Duplicate or invalid targets reject the whole
edit. Archive readback and geometry transforms preserve these properties.

Shaded and ghosted viewports display per-face colors on smooth SubDs and flat
control nets. Display triangles use their source control-face ordinal, so colors
do not blend across shared face boundaries. Selected wires retain face colors;
locked objects use the existing locked display policy. Ghosted faces use the
same opacity policy as colored mesh faces. Material channel indices are stored
but this change does not add material rendering.

The [validation receipt](subd-face-properties-validation.json) binds twenty
actual Rhino color workflows, including 21 `MergeFaces` invocations and 41 source
and result 3DM archives. Rhino clears colors on merged faces, even when inputs
share a color, and preserves untouched faces. No-op regions retain their colors.
Sequential edits and application Undo/Redo match these records. The native
[PerFaceColor API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.subdface/perfacecolor?version=8.x)
provides the independent color observations.

The [custom face-data qualification](subd-face-custom.md) extends native
comparison to material channels, custom UVWs, spatial transforms and sequential
history. Other allocating operations still need separate comparison.
