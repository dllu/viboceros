# Native kernels and numerical geometry

[Architecture overview](../architecture.md) · [Documentation](../README.md)

The geometry crate owns validated primitives, curve and surface representations,
B-rep topology, and geometric queries. Document policy, command input and UI state
remain outside the kernel.

## Native backends

`viboceros-opennurbs` owns the shared OpenNURBS build and C ABI. Geometry uses
its public curve, mesh and SubD operations; file interchange links the same
native archive. Geometry does not depend on the I/O crate.

`viboceros-smlib` adapts usd-brep's SMLib kernel for native B-rep and NURBS
modeling. Supported native command paths delegate construction, refinement,
intersections and solid operations to this adapter. Rust retains operand
selection, tolerance policy, document metadata and atomic admission. Native
calls are serialized and operate on copied operands with owned output geometry.

Portable and native capabilities differ. Their supported operations and remaining
limits belong in the [native modeling guide](../native-modeling.md) and command
pages, rather than a second feature list in this architecture overview.

## Geometry modules

NURBS construction, evaluation, editing, integration and queries have separate
modules under `nurbs/`. Conic constructors share the existing validated curve
representation; legacy regressions are grouped by operation under `nurbs/tests/`.
Prepared samplers and derived caches retain immutable geometry snapshots.

Shared algorithms supply curve fitting, surface pullback/pushup, trimming and
Boolean classification. Exact constructions and certified portable paths remain
available where the selected native backend does not supply the operation.
Commands adapt these results without duplicating geometry algorithms.

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
