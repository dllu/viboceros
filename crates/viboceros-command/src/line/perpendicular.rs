//! Backward-compatible constructors for perpendicular curve-contact getters.
pub use super::curve_input::{Input, Stage};
pub fn requested(args: &[&str]) -> bool {
    super::curve_input::requested(args) == Some(super::curve_input::Kind::Perpendicular)
}
