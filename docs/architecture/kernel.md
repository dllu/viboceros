# Native kernels and numerical geometry

[Architecture overview](../architecture.md) · [Documentation](../README.md)

The geometry crate owns validated primitives, curve and surface representations,
B-rep topology, and geometric queries. Document policy, command input and UI state
remain outside the kernel.

## Modeling and interchange

`viboceros-opennurbs` owns the shared OpenNURBS build and C ABI. Geometry uses
its public curve, mesh and SubD operations; file interchange links the same
native archive. Geometry does not depend on the I/O crate.

`viboceros-smlib` adapts usd-brep's SMLib kernel for B-rep and NURBS modeling
and is required by the command, I/O and oracle crates. Construction, refinement,
intersection and solid commands use this adapter wherever it can represent the
operation and its inputs. Native calls are serialized and return owned geometry.
Rust handles operand selection, tolerances, document metadata and validation
before admission. See the [modeling guide](../native-modeling.md) for coverage
and known limitations.

## Geometry modules

NURBS construction, evaluation, editing, integration and queries have separate
modules under `nurbs/`. Conic constructors share the existing validated curve
representation; legacy regressions are grouped by operation under `nurbs/tests/`.
Prepared samplers and derived caches retain immutable geometry snapshots.

Shared Rust algorithms supply curve and surface evaluation, analytic
constructions, trim checks, and Boolean classification. Some commands use
specialized Rust producers where SMLib cannot provide the required geometry or
topology, including certain concave planar faces and compound polyhedral
Booleans.

The Rust B-rep is the validated representation shared by the document,
file adapters, and native modeling bridge. Its mesh conversion owns the
construction of polygon and planar n-gon faces. Face copies and trim removal
share topology compaction, which keeps retained vertices and edges in source
order. Validation and compaction classify a used edge from its total trim
count and the count in the current loop. Disjoint combination preserves shell
orientation and identities even when their vertices coincide.

## Numerical contracts

Finite-value validation, scaled frames, compensated arithmetic and exact
fallbacks protect difficult binary64 inputs. [Numerical robustness](../numerical-robustness.md)
records their scope and resource limits. Normalized integration charts map
returned parameters back to the source domain; numerical length estimates and
precise distance inverses retain separate contracts.

Operations requiring continuous source or trim agreement use conservative
[geometry certificates](../surface-curve-certificates.md). Sampled residuals can
propose a fit but do not prove a bound between stations. Unresolved proofs and
resource exhaustion return failure without admitting partial geometry.

The [Rhino oracle](../oracle.md) records public SDK and command outputs separately
from implementation. Qualification identifies the tested inputs, comparison
epsilon, source bindings and remaining discrepancies. A passing subset does not
establish complete Rhino parity or performance parity.
