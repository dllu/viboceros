# USD BRep limitations and bugs

This ledger tracks findings relevant to Viboceros's pinned usd-brep 1.0.0
(`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It distinguishes upstream
behavior from restrictions in our adapter. See [integration](usd-brep.md) for
setup and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md) for
issues reported by NVIDIA that have not all been reproduced here.

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
