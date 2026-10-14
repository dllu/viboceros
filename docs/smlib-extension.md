# Native curve and surface extension

Linux `native-smlib` builds use usd-brep's `N_CrvExtendByParamDist` with
`NL_CMAX` for exact end-span extrapolation in `Extend` and `ExtendSrf`.
The adapter imports the complete source definition, clamps only an endpoint
being extended when needed, and exports the original degree and parameter
chart. Binary parameter conditioning, a local coordinate origin and a common
weight gauge protect the native calculation. Uniformly negative weights are
temporarily reversed in homogeneous coordinates and restored on export.

The geometry core supplies the existing curve-length solve, natural line/arc
classification, joined/separate representation and surface RMS-speed distance
convention. Native curve extension and checked segment extraction are callbacks
inside that policy. `Join=Merge`, `Join=Yes` and `Join=No` retain source identity,
attributes, selection, grouping and undo behavior. Native geometry errors keep
their existing command error type; results are prepared before document edits.
Command execution and oracle timings choose the same extension backend.

Surface extension applies the producer to each complete homogeneous control
curve in the selected direction. Whole-axis closure is checked once; collapsed
pole rows and individually closed rows remain legal in an open surface axis.
The untouched direction retains its exact topology and knots. Its degree may
exceed the native limit because it never enters a native surface constructor.
Requested-axis degrees above 17 retain the portable extension fallback.

The pinned producer underallocates output arrays when an end span borders a
full-order interior break. Both extension branches calculate
`m + 2*p - mult`, although no copies are inserted when `mult >= p`.
The [bounded repair](../third_party/patches/usd-brep-extension-break-allocation.patch)
uses `m + p + max(p-mult,0)`. Native CMake applies it to a generated source copy,
stops if patching fails, and leaves the pinned submodule unchanged.

The [standalone probe](../tools/usd_brep/extension_probe.cpp) independently
checks quadratic linear pieces, ordinary/full-order interior knots, both ends
and both parameter/distance producers. Before repair, both full-break parameter
cases fail and both full-break distance cases overflow their allocations.
The [retained comparison](smlib-extension-producer-comparison.json) includes
all isolated process results, source hashes and AddressSanitizer diagnostics.
All eight cases pass afterward, with maximum point error `9.93e-16` and exact
requested domains. Leak accounting alone is disabled for the process-wide
native context; invalid memory accesses remain checked.

Rust regressions cover 24 curve/surface fixture cases, rational and unclamped
ends, repeated knots, full-order breaks, common signed weight gauges `1e-280`
and `-1e280`, independent polynomial precision through degree 17, parameter
charts near `2^-800` and `2^810`, collapsed poles and degree-18 fallback/untouched
surface directions. Source objects remain unchanged. Direct controls/knots and
position grids are compared with the portable producer; the polynomial and
collapsed-pole laws provide independent geometric expectations.

The [Python comparison](../tools/numerics/compare_native_extensions.py) combines
six existing SDK and scripted-command fixtures with stable prefixed identifiers.
It can capture fresh Rhino observations in a private Xvfb session and replay
them through `OracleClient(native_smlib=True)`. Complete output definitions and
command metadata are checked; existing documented command epsilon ceilings
are retained. Its timings are harness diagnostics, not a kernel-speed claim.

The fresh 60-case Rhino capture on October 14 did not reach worker execution:
Wine's selected `sspicli.dll` lacked `AcquireCredentialsHandleW` during license
startup. A per-process built-in security override also left startup blocked.
Both owned test processes were closed; no reference observations were invented.
The [capture record](smlib-extension-rhino-capture.json) retains that limitation.
The [60-case comparison](smlib-extension-comparison.json) therefore records
native and portable Viboceros responses with their actual engine identities.
It checks complete output definitions and command metadata at `1e-10` absolute
and `1e-12` relative tolerance; this is backend equivalence, not new Rhino parity.
All 60 cases pass, with maximum numeric difference `2.84e-14`. The native suite,
19 command regressions, 39 geometry policy tests, nine oracle tests, 1,100 Python
tests and native/portable Clippy checks also pass.

```sh
cargo test --release -p viboceros-smlib --features native --test curve_extension --test surface_extension
cargo test --release -p viboceros-command -p viboceros-oracle \
  --features viboceros-command/native-smlib,viboceros-oracle/native-smlib extend
tools/rhino_oracle/run_headless.sh exec python3 -m tools.numerics.compare_native_extensions \
  /tmp/extensions.json --capture-rhino --timeout 900
python3 -m tools.numerics.compare_native_extensions /tmp/extensions-replay.json
python3 -m tools.numerics.compare_native_extensions /tmp/extensions-portable.json --portable-baseline

cmake -S tools/usd_brep -B /tmp/extension-before -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_EXTENSION_BREAK_PATCH=OFF
cmake --build /tmp/extension-before --target viboceros_usd_brep_extension_probe -j 10
cmake -S tools/usd_brep -B /tmp/extension-after -DCMAKE_BUILD_TYPE=Release \
  -DVIBOCEROS_EXTENSION_BREAK_PATCH=ON
cmake --build /tmp/extension-after --target viboceros_usd_brep_extension_probe -j 10
python3 tools/usd_brep/compare_curve_extension_producer.py \
  /tmp/extension-before/viboceros_usd_brep_extension_probe \
  /tmp/extension-after/viboceros_usd_brep_extension_probe /tmp/extension-producer.json
```

Build sanitizer variants with
`-DCMAKE_CXX_FLAGS="-fsanitize=address -fno-omit-frame-pointer"` and pass their
executables as `--asan-before` and `--asan-after` to the producer comparison.
Boundary-target `Extend`, Connect continuation, circular/linear style geometry
and negative `ExtendSrf` shrinking retain their current shared producers.
Arbitrary conditioning, rational poles and performance against native Rhino
remain further qualification work.
