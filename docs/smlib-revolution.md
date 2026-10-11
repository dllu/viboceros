# Native curve revolution

Linux builds with `--features native-smlib` now use usd-brep for Revolve's NURBS
surface construction. Parsing and document staging live in the separate
`revolve` command module. Native builds exclude the Rust rotational control-net
builder from this command; other builds retain it.

The bridge invokes `SmBSplineSurface::CreateSurfOfRevolution` on a retained
profile handle and exports through the shared owned surface snapshot. This
upstream constructor preserves the complete profile tensor, including kinked
and rational spans. The higher-level wrapper can return only the first surface
after splitting a profile. Native reparameterization sets circular U and profile
V; the constructor's UV flag is inverted relative to its pinned header text.

Profile preparation is shared with ExtrudeCrv: existing de Boor extraction
clamps active ends, and a uniformly negative homogeneous gauge is normalized.
Original objects and native input curves remain unchanged. Mixed-sign weights
and discontinuous interior knots remain adapter restrictions.

The command uses existing affine transforms for its start angle, axis-local
construction and world placement. Working at the axis origin avoids radial
degeneracy thresholds inflated by large translations. A negative sweep uses
increasing circular parameters starting at the geometric end angle, matching
the recorded Rhino command chart. The low-level native surface method also
supports axis reversal explicitly.

The circular parameter length retains the existing command's 65-station radius
estimate. That policy is now shared by both backends; it is not an exact
maximum-radius query or an error bound. No second revolution geometry algorithm
or radial optimizer was added. Further radius-domain qualification remains open.

All surfaces are staged before document changes. Current-layer outputs, source
deletion/retention, selection, groups and Undo/Redo retain the existing command
workflow. As before, Deformable=Yes is unsupported; the existing SplitAtTangents
option parsing does not yet provide a separate splitting workflow.

The [live Rhino fixture](../tools/rhino_oracle/fixtures/curve_native_revolution.json)
and [raw observations](../tools/rhino_oracle/observations/curve_native_revolution.json)
cover full, partial, negative sweeps with nonzero starts, and kinked profiles with
CreaseSplitting disabled. The probe explicitly sends the requested sweep amount
after the start angle. See the [comparison](smlib-revolution-comparison.json) for
complete raw NURBS field results at `1e-10` absolute / `1e-12` relative epsilon.
All four cases match every recorded field, with no native audit errors and no
domain, order or control-net normalization.
The workflow follows [McNeel's command options](https://docs.mcneel.com/rhino/8/help/en-us/commands/revolve.htm).

Independent regressions cover profile domains/weights, preserved native handles,
complete kinked profiles, signed and arbitrary-axis endpoints, invalid angles,
and translated small-radius profiles. These fixtures do not establish arbitrary
surface equivalence, axis-crossing validity, singular topology, scale/aspect-ratio
robustness or performance parity. See the [limitations ledger](usd-brep-limitations.md).

Release validation passed 1,307 native command tests and 49 native bridge tests
(three tests ignored across those suites), the default-backend Revolve
regression, and 1,095 Python oracle tests. Native clippy with warnings denied
and workspace formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test revolution
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib revolve
```
