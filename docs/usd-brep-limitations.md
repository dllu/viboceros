# USD BRep limitations and bugs

This ledger records findings from integrating usd-brep. Viboceros currently pins
1.0.1. Some captures were made with 1.0.0 and retain their original revisions.
See the [integration guide](usd-brep.md) and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md).

Findings are grouped by subsystem. Each entry retains its source, reproduction
recipe, qualification limits and any generated repair.

## Native API behavior verified during integration

- [NURBS bases and evaluation](native-limitations/basis.md)
- [Splitting, extraction and knot removal](native-limitations/extraction.md)
- [Modeling, interpolation and fitting](native-limitations/modeling.md)
- [Topology, transfer and runtime](native-limitations/topology.md)

## Observed compatibility differences

[Rhino comparison results](native-limitations/compatibility.md) separate geometric
differences from parameterization, partitioning and representation differences.

## Adapter restrictions, not established upstream bugs

- [Contact, interpolation and fitting policies](native-limitations/adapter-modeling.md)
- [Charts, representation and resource limits](native-limitations/adapter-representation.md)

Keep new findings tied to a source, reproducible test or retained capture.
Do not label a Rhino partition/representation difference as an upstream geometry
bug without evidence that the mathematical output is invalid.
