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

Choose the relevant [validation guide](development/checks.md): native kernel,
Booleans and topology, curves and surfaces, or UI and measurement. Each guide
keeps its detailed evidence and limitations with the subsystem it exercises.

Command regressions are grouped by topic under
`crates/viboceros-command/src/tests/`, with shared geometry fixtures in its
`mod.rs`. Add tests to the relevant topic instead of the command dispatcher.

Application regressions live under `src/app/tests/`, with shared fixtures in its
`mod.rs`. Command state and dispatch live in focused application modules; see
the [application module map](architecture/application.md) before adding to the app root.

Live Rhino oracle testing must run in a separate Xvfb display so it cannot
interfere with the shared desktop. Use `tools/rhino_oracle/run_headless.sh` for
captures, including its `exec python3 ...` mode when calling the Python oracle
API directly. See the oracle guide for capture and replay commands. Replaying
saved observations does not launch Rhino.

## Documentation

Keep the root README limited to a project overview, basic setup, and documentation
links. Put command syntax, behavior, examples, and limitations in
[`docs/commands/`](commands/README.md), and implementation or testing details in
the relevant page under `docs/`. Use the [documentation directory](README.md) and
existing command topic indexes for navigation. Keep examples with their topic,
and split an index by subject when it becomes difficult to scan.
Link overview pages to subject indexes instead of appending individual
qualification reports as features land.
Periodically review the README's capability and build instructions. Extract
oversized command implementations and their tests into focused modules before
adding more responsibilities to the dispatchers.

Historical usd-brep integration sources are retained through immutable Git
references instead of copied `*-prior-source` trees. Provenance maps named
`git_sha256` or ending in `_git_sha256` use `commit:path` keys, with the full
commit ID and the SHA-256 of the original bytes. Existing local maps retain
captured artifacts and other file hashes. A full clone contains the required
history; shallow clones must fetch the referenced commits.

```sh
python3 tools/verify_provenance.py docs/smlib-bridge-provenance.json \
  docs/smlib-import-provenance.json docs/fillet-edge-provenance.json
git show COMMIT:PATH
```
