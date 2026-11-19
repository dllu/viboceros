#pragma once

// Scoped to the curve sweep that supplies its own profile segments. Other
// callers of the native primitive keep its original subdivision policy.
inline thread_local bool vb_taper_use_supplied_partitions = false;
