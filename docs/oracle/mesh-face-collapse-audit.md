# Mesh face collapse: independent storage audit

[Capture investigation](mesh-face-collapse.md) · [Measurement report](mesh-face-collapse-validation.json)

The Rust [audit example](../../crates/viboceros-geometry/examples/mesh_face_collapse_audit.rs)
reads the immutable native meshes and calls existing geometry/kernel APIs.
Its [complete output](mesh-face-collapse-kernel-audit.json.gz) includes every
source/result, strict facet validation and explicitly named public-operation
trials. It does not implement `CollapseMeshFace` or claim image compatibility.

| Check | Result |
| --- | --- |
| Input facets accepted by `Tolerance::MESH_VALIDATION` | 120/120 |
| Result facets accepted under the same policy | 107/120 |
| Results rejected because their fixed A–C quad split has a zero-area triangle | 13 |
| Edited native results exactly reproduced by dissolving the first raw corner | 0/82 |
| Edited native results exactly reproduced by dissolving all raw corners | 0/82 |

The strict Rust check validates both stored triangles of a quad. Public
OpenNURBS `ON_MeshFace::IsValid` checks indices and distinct vertex positions,
which is a different predicate. The audit does not equate the native stored
validity flag with nondegenerate facets, and does not weaken geometry validation
to accept the retained outputs. The public dissolution trials retain source
facets/n-gon semantics. Simple storage replacement remains unqualified; storage
disagreement alone does not establish a geometric-image gap.

The earlier float-precision observation also needed correction. McNeel's
[open-source wrapper](https://github.com/mcneel/rhino3dm/blob/main/src/dotnet/opennurbs/opennurbs_mesh.cs)
implements vertex enumeration and `ToFloatArray` through the double-aware vertex
accessor. The new `mesh_face_collapse_buffers` family instead disables double
storage on an owned duplicate, records its float positions and guards the
original geometry with CRC/precision checks. Precision is set **after** adding
vertices; setting it before insertion did not preserve float-only source data.
The old captures remain immutable and retain their original field names.

```sh
cargo run --frozen -p viboceros-geometry --example mesh_face_collapse_audit
python3 -m unittest tools.rhino_oracle.test_mesh_face_collapse
```

With no arguments the example audits every retained mesh-face-collapse family;
family names can select a smaller diagnostic run. The example reports storage
agreement separately from geometric validity. Both are distinct from the
required `1e-7` image comparison, which remains unqualified for this command.
