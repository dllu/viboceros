//! Regressions for complete coverage and topology comparison.
use super::*;
#[path = "../support/ribbon.rs"]
mod shared;
pub use shared::{coverage, incidence};
use shared::{covers, native_rectangular};

#[test]
fn incomplete_parameter_coverage_is_rejected() {
    let parent = [[0., 1.], [0., 1.]];
    assert!(covers(
        parent,
        &[[[0., 0.5], [0., 1.]], [[0.5, 1.], [0., 1.]]]
    ));
    assert!(!covers(
        parent,
        &[[[0., 0.5], [0., 1.]], [[0.5, 1.], [0., 0.75]]]
    ));
}

#[test]
fn a_missing_native_trim_or_wrong_edge_incidence_cannot_be_qualified() {
    let n = curves::read("ribbon_ruled");
    let value = &n["results"][0]["value"];
    let output = value["created"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["source"] == false)
        .unwrap();
    let original = &output["brep"]["geometry"];
    let surface = captured::surface(&output["surfaces"][0]);
    assert!(native_rectangular(original, 0, &surface));
    let mut changed = original.clone();
    changed["faces"][0]["loops"][0]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(!native_rectangular(&changed, 0, &surface));
    let source = curves::curve(&value["before"][0]["curve"]);
    let normal = Vector3::try_new(0., 0., 1.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let (actual, _) = build(&source, 0.5, false, normal, captured::model_tolerance()).unwrap();
    assert!(incidence(&actual, original, &[0]).is_some());
    let mut changed = original.clone();
    changed["topology"]["edges"][0] = json!([0, 0]);
    assert!(incidence(&actual, &changed, &[0]).is_none());
}
