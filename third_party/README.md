# Third-party code

- `opennurbs` is a shallow, pinned submodule of McNeel's official OpenNURBS
  toolkit. It provides genuine 3DM archive compatibility through Viboceros's
  narrow C ABI bridge. OpenNURBS and its bundled dependencies remain under the
  terms in [`opennurbs/LICENSE`](opennurbs/LICENSE), independently of
  Viboceros's MIT-licensed Rust code.
- `opennurbs_rust` contains attributed, modified Rust adaptations of public
  OpenNURBS routines; see its [README](opennurbs_rust/README.md) and
  [license](opennurbs_rust/LICENSE).
- `monstertruck-io` is a local patch of the Apache-2.0 licensed 0.4.0 crate
  by Yoshinori Tanimura and Moritz Moeller. Its STEP writer labels repeated
  same-face edge uses as `SEAM_CURVE`; its loader retains explicit p-curves on
  closed surface curves and uses them for exact face trims. See its
  [license](monstertruck-io/LICENSE).
