# Contour and Box follow-up checkpoints

[Historical checkpoint](../../validation-checkpoint.md)

## Contour checkpoint

[Contour domain provenance](../../contour-domains-provenance.json) binds 52 captures
(40 distinct recipes and 12 repeat observations). All fields match at 1e-9
absolute + 1e-12 relative epsilon, including actual domains. The original five
surface-domain diagnostics are resolved; the original 21-case report also
passes. Historical failing reports and source bytes remain archived.

The parameter policy preserves natural edge and exact bilinear isocurve domains,
uses signed cutting-plane distances and ranks competing isocurves by the
magnitude of their fixed parameters. Eight independent UV-range experiments
and three repeats of four off-axis recipes support this inference. Source UV
shifts and rational weights are included. A positive-weight control-hull check
uses exact dyadic squared-distance bounds to reject overshoots and hidden lobes
before affine line simplification. Source geometry and transaction behavior
remain unchanged.

Native probes used private Xvfb, private settings schemes and per-process
native-first Wine security DLLs. Public SDK type/control/knot observations are
retained with exact helper bytes. No domain exclusions or geometric
normalization are used in the strict replay. The Python implementation is
unchanged since the preceding 1,070-test run; its four Contour preflight tests
and all 52 recipes were revalidated at this checkpoint.

Broader curved/trimmed B-rep families, extreme and near-tie UV values, closed
seams, hatches and performance parity remain open. README stays concise;
details are in [Contour](../../commands/contour.md) and
[parameterization](../../contour-domain-parameterization.md).

## B-rep Contour checkpoint

[B-rep Contour provenance](../../contour-brep-provenance.json) records a complete
5,598-test Rust run (38 ignored), 1,070 passing Python tests and clean Clippy,
formatting and diff checks. All 24 new B-rep/native grouping recipes and the
existing 52 surface-domain captures pass strict replay at 1e-9 absolute plus
1e-12 relative epsilon. No domain, ordering or seam exclusions are used.

The application circular-hole Boolean regression passed after limiting the
cyclic seam change to exactly coincident scan-line spans. Two direct kernel
regressions cover outer-boundary overextension and hole-side truncation.
Validation used an isolated target after the environment profile changed and
an old artifact lock was unavailable to the new tool session.

During this capture, the managed profile protected `.git` as read-only and an
owned write probe returned EROFS. A later profile restored filesystem access;
an October 9 create/write/delete probe now succeeds. The historical capture
record retains its original restriction. Broader geometry/command coverage and
performance remain open.

## Box topology checkpoint

[Native Box topology](../../box-native-topology.md) introduces a command-compatible
constructor using attributed OpenNURBS connectivity and parameter extent tables.
The Box command and SDK-box oracle share it; the generic normalized constructor
remains available. Four new regressions cover topology/UV/trim intervals, rotated
frames, tiny-box fallback and invalid intervals, and Box-to-Contour/Undo.
The full release run passes 5,602 Rust tests with 38 ignored; Clippy and format
checks are clean. The existing 24 B-rep and 52 surface-domain native captures
continue to pass. Python code is unchanged from the preceding 1,070-test run.

The initial tiny-box test used near-zero numerical validation tolerance and
failed ordinary rounded trim/edge correspondence. Its modelling tolerance was
set to 1e-24 for 1e-12 geometry; the constructor continues to use the caller's
tolerance without relaxation. Source attribution and hashes are in
[provenance](../../box-native-provenance.json).
