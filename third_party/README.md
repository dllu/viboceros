# Third-party code

- `opennurbs` is a shallow, pinned submodule of McNeel's official OpenNURBS
  toolkit. It provides genuine 3DM archive compatibility through Viboceros's
  narrow C ABI bridge. OpenNURBS and its bundled dependencies remain under the
  terms in [`opennurbs/LICENSE`](opennurbs/LICENSE), independently of
  Viboceros's MIT-licensed Rust code.
- `opennurbs_rust` contains attributed, modified Rust adaptations of public
  OpenNURBS routines; see its [README](opennurbs_rust/README.md) and
  [license](opennurbs_rust/LICENSE).
