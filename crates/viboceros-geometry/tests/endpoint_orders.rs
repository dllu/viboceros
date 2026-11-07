#[path = "support/endpoint_orders.rs"]
mod orders;
#[test]
fn all_mixed_endpoint_orders_reconstruct_known_polynomials() {
    orders::check(viboceros_geometry::try_interpolate_endpoints);
}
