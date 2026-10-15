# Shell

`Shell` hollows a closed manifold polysurface inward and opens the selected
faces. It requires the Linux `native-smlib` build.

Start `Shell`, pick face components, and press Enter. Face-component preselection
also works. Use `Thickness=1` or enter `Thickness` followed by a distance; the
interactive getter accepts unit suffixes such as `2mm`. `DeleteInput=No` keeps
the original. At least one face per source must remain. Esc cancels. A rejected
operation keeps the face picks so thickness can be corrected and retried.

For scripts, select source objects and specify `Face=0` or `Faces=0,2`.
`FaceRefs=id:index,...` addresses original faces across multiple sources:

```text
Shell Thickness=1 DeleteInput=No FaceRefs=1:0,1:2
```

Thickness must be finite and positive; the current default is one model unit.
DeleteInput defaults to Yes. Duplicate references are removed. Results inherit
source attributes and group membership. The entire batch, including source
removals, is one Undo/Redo step; failures preserve the document and history.
Disconnected material regions become separate objects, while enclosed cavity
boundaries stay with their surrounding material.

The native operation copies the imported B-rep, offsets retained faces, intersects
extended surfaces and constructs the wall volume by Boolean difference. The
adapter keeps an original-index face table; native traversal order is not used
as a substitute for input identity. Inputs are localized before shelling to
reduce coordinate cancellation.

Qualification covers rectangular boxes, multiple openings, original face order,
curved walls, source preservation and failure recovery as recorded in
[the qualification record](../shell-qualification.json). Public Python SDK
recipes and independent volume fixtures are in
[tools/rhino_oracle/shell_probe.py](../../tools/rhino_oracle/shell_probe.py).
Licensed Rhino captures remain pending because of the documented
[Wine startup blocker](../oracle-sspi.md); no full Rhino parity or performance
claim is made. The adapter rejects results outside the source bounds or without a measurable
cavity. Cavities smaller than 1e-7 of source volume decline conservatively.
Complex offsets, collapsed walls, singular surfaces and general
self-intersections need broader qualification. Live previews, History editing
and native saved-default behavior remain future work.

Reference behavior: [Rhino 8 Shell](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetsrf.htm#Shell)
and [Brep.CreateShell](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateShell.htm).

```sh
cargo test -p viboceros-smlib --features native --test shell
cargo test -p viboceros-command -p viboceros --features native-smlib shell::
python3 -m unittest tools.rhino_oracle.test_shell
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/shell.json --native-smlib
```

Native memory checks can instrument both complete kernel libraries and the C ABI
with `VIBOCEROS_SMLIB_ASAN=1`. On Linux aarch64, run target binaries through a
Cargo runner so the sanitizer is preloaded for tests:

```sh
VIBOCEROS_SMLIB_ASAN=1 \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER='env LD_PRELOAD=/usr/lib/aarch64-linux-gnu/libasan.so.8 ASAN_OPTIONS=detect_leaks=0' \
cargo test -p viboceros-smlib --features native --test shell
```

Unset the instrumentation variable for normal builds. The library path and
Cargo target runner key depend on the host platform. Recorded checks disabled
leak detection and do not certify leak freedom.
