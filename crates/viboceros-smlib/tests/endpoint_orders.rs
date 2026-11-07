#![cfg(feature = "native")]
#[path = "../../viboceros-geometry/tests/support/endpoint_orders.rs"]
mod orders;
#[test]
fn native_mixed_endpoint_orders_reconstruct_known_polynomials() {
    orders::check(viboceros_smlib::interpolate_endpoints);
}
