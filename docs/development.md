# Development and testing

[Project overview and setup](../README.md) · [Architecture](architecture.md)

Run these checks from the repository root. Release-mode tests keep the exhaustive
exact-arithmetic checks practical.

```sh
cargo test --workspace --release
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python3 -m unittest discover -s tools/rhino_oracle -t .
```

## Focused checks

- [Convex polyhedral Boolean kernel and native evidence](convex-booleans.md)
- [Chained polyhedral Booleans with concave faces, holes and multiple shells](polyhedral-booleans.md)
- [Command adapters and compound shell evidence](polyhedral-boolean-commands.md)
- [BooleanUnion command workflows and native evidence](commands/boolean-union.md)
- [BooleanIntersection common-set and two-set workflows](commands/boolean-intersection.md)
- [BooleanDifference targets, cutters, and native policies](commands/boolean-difference.md)
- [Viewport caching and performance](viewport-caching.md)
- [Opt-in offscreen GPU tests](gpu-tests.md)
- [Imported surface shading and mesh checks](imported-shading.md)
- [Rhino oracle setup, Python API, and comparisons](oracle.md)

Live Rhino oracle testing must run in a separate Xvfb display so it cannot
interfere with the shared desktop. Use `tools/rhino_oracle/run_headless.sh` for
captures, including its `exec python3 ...` mode when calling the Python oracle
API directly. See the oracle guide for capture and replay commands. Replaying
saved observations does not launch Rhino.

## Documentation

Keep the root README limited to a project overview, basic setup, and documentation
links. Put command syntax, behavior, examples, and limitations in
[`docs/commands/`](commands/README.md), and implementation or testing details in
the relevant page under `docs/`.
