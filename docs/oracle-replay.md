# Diagnostic oracle replay

[Oracle overview](oracle.md) · [Corner-clustering evidence](join-corner-clustering.md)

Normal `compare` requires both engines to finish successfully. Diagnostic replay
instead preserves each native operation's success or classified execution error,
so one rejected B-rep does not discard the successful cases around it. It runs
the native batch once, without retrying sub-batches or starting Rhino.

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_gap_chains.json --observations tools/rhino_oracle/observations/join_gap_chains.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

This example reports two full matches and two native execution failures, exiting
with status 1. Use `audit` instead of `replay`, without `--observations`, to get
raw native outcomes without a reference comparison. The Python CLI exits 0 only
when every case succeeds/matches, 1 for recorded failures/differences, and 2 for
invalid requests, protocol errors, or process failures.

## Python API

```python
from tools.rhino_oracle import OracleClient, load_request

request = load_request("tools/rhino_oracle/fixtures/join_gap_chains.json")
reference = load_request("tools/rhino_oracle/observations/join_gap_chains.json")
report = OracleClient().replay(request, reference, 1e-10, 1e-12)
for row in report.operations:
    if row.native_error is not None:
        print(row.id, row.native_error.kind, row.native_error.message)
    else:
        print(row.id, row.comparison.passed, row.comparison.differences)
```

`run_viboceros_audit(request)` returns the raw native diagnostic response.
`compare_audit_response(native, reference, absolute_epsilon, relative_epsilon)`
compares already loaded responses without launching either engine. Reports
separately count `matched`, `mismatched`, and `native_failed`. An errored operation
has no comparison object, invented geometry, zero residual, or timing sample.
Successful records use the same complete recursive comparison as normal mode;
the reference's order need not match, but its IDs and iteration count must.
No geometry, table order, parameters, or reference records are normalized.

Replay preflight checks IDs, metadata, and epsilons before launching the native
process. It does not prove that an arbitrary reference file was produced from
the supplied inputs: use paired fixtures and the recorded shared-artifact checks
and hashes. Harness timings are not kernel-speed comparisons.

For `join_command`, the request's top-level tolerance constructs source geometry.
An operation's `absolute_tolerance` changes the document/Join policy only; it
must not rebuild or collapse source features. Shared 3DM sources are exported
with the same construction tolerance. The [short-overlap audit](join-short-overlaps.md)
records the native harness correction and preserves the distinction in its baseline.

## Native execution and failure boundaries

`viboceros-oracle --audit REQUEST.json RESPONSE.json` and
`viboceros_oracle::run_request_audit` dispatch each operation once, honoring the
requested iteration count. The diagnostic envelope contains `outcomes`, distinct
from the normal response's `results`. Each outcome is either a successful
`OperationResult` or an ID and error `{kind, message}`. Error categories identify
the outer Rust error type: geometry, command, document, interchange, I/O, JSON,
input, timing, or fixture invariant. They are execution errors, not inferred
geometry mismatches; for example, a command-wrapped geometry rejection has kind
`command`.

Global request validation, including duplicate IDs, occurs before any operation
or export. Malformed requests, native panics/crashes, launch failures, and invalid
response schemas remain fatal; no panic recovery or automatic retry occurs.
Recoverable per-operation errors retain their category and message and allow
later independent operations to run. High-level `replay` and `compare` redirect
their supported artifact exports into owned temporary directories and clean them
on success or failure; caller paths are not overwritten. Low-level native/audit
execution still honors explicitly requested export paths and is not a filesystem
sandbox. Caller request and observation objects are not modified.

The native executable's exit status 0 means a diagnostic response was written;
inspect its outcomes or use the Python CLI for the aggregate 0/1 result.
