#[path = "support/loft_rebuild.rs"]
mod comparison;

#[test]
fn rebuilt_lofts_match_native_surface_definitions() {
    comparison::compare(1e-6, viboceros_geometry::try_loft_nurbs_curves_rebuilt);
}

#[test]
#[ignore = "Rhino's arc-length rebuild stations differ above 1e-9; retained strict diagnostic"]
fn strict_native_rebuild_station_comparison() {
    comparison::compare(1e-9, viboceros_geometry::try_loft_nurbs_curves_rebuilt);
}
