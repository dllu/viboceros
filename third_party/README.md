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
  Void-shell export uses the required false orientation and reverses the
  underlying face sense to preserve the caller's material boundary orientation.
- `usd-brep` pins NVIDIA's Apache-2.0 licensed release 1.0.0
  (`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It contains the SMLib
  solid-modeling kernel and native/Python/USD APIs. The optional Rust bridge
  builds the kernel directly through Cargo; see [the trial](../docs/usd-brep.md), its
  [license](usd-brep/LICENSE), and
  [third-party notices](usd-brep/THIRD_PARTY_NOTICES.md).
