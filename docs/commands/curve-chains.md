# Connected curve and surface-edge selection

Enter `SelChain`, then pick a curve or a B-rep/NURBS surface edge. Unique
compatible continuations select immediately. At a fork, the proposed path is
highlighted: click another branch or press Enter to confirm it. A second Enter
finishes selection. Escape restores the original selection.

Whole curves selected before starting remain selected and are ignored as seeds.
Selection does not edit source geometry, create model history, or consume redo.
Changing source geometry, visibility or tolerance invalidates the getter.

| Option | Behavior |
| --- | --- |
| `ChainContinuity=Position` | Match endpoints within the gap tolerance |
| `ChainContinuity=Tangency` | Also match endpoint tangent directions; initial default |
| `ChainContinuity=Curvature` | Also match endpoint curvature vectors |
| `Direction=Forward\|Backward\|Both` | Follow the seed's parameter direction; initially Both |
| `GapTolerance=distance` | Endpoint distance in model units; initially document absolute tolerance |
| `AngleTolerance=degrees` | Tangent tolerance; initially document angular tolerance |
| `Undo` | Remove one chain segment, including an automatically added segment |
| `Next` / `All` | Extend one segment / continue through compatible segments |

`Undo`, `Next`, `All` and option changes apply at the connected-curve prompt.
The branch-choice prompt accepts a branch or Enter. Changing continuity does
not expand the selection until the next pick, `Next` or `All`.
At mixed forks the first seed's type is preferred; among eligible branches the
newest source supplies the proposal. Closed paths stop after one circuit.
Accepted option changes persist for the application session, including cancellation.
Zero or values at/below `2^-32` (in model units or radians) restore the
corresponding document tolerance. Other positive values may be smaller than
the document tolerance.

Headless scripts can use `SelChain Seed=uuid` or `SelChain Edge=uuid,index`,
followed by the same options. These Viboceros extensions explicitly choose a
seed, use zero-based edge indices, and accept the default branch proposal.
Explicit seeds can also identify a preselected curve.

The [native comparison corpus](../oracle/curve-chains.md) qualifies standalone
selection. Nested `SelChain` inside another command, mesh/SubD edge chains,
and arbitrary complex trimmed-surface branch behavior remain unqualified.
