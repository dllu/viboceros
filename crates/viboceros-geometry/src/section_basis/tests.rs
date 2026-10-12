use super::*;
use crate::Point3;

fn profiles() -> Vec<NurbsCurve> {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    vec![
        NurbsCurve::try_new(1, vec![p(0., 0., 0.), p(2., 0., 0.)], vec![0., 0., 1., 1.]).unwrap(),
        NurbsCurve::try_new(
            2,
            vec![p(0., 0., 2.), p(1., 1., 2.), p(2., 0., 2.), p(3., 1., 2.)],
            vec![0., 0., 0., 0.5, 1., 1., 1.],
        )
        .unwrap(),
    ]
}
fn limit() -> GeometryError {
    GeometryError::LoftResourceLimit {
        context: "test controls",
        maximum: 16,
    }
}

#[test]
fn external_section_operations_preserve_shared_layout_and_sources() {
    let source = profiles();
    let before = source.clone();
    let calls = std::cell::RefCell::new(Vec::new());
    let actual = prepare_with_operations(
        &source,
        16,
        limit(),
        WeightScale::PerSection,
        &mut |c| {
            calls.borrow_mut().push("clamp");
            c.clamped_to_active_domain()
        },
        &mut |c, d| {
            calls.borrow_mut().push("elevate");
            c.try_change_degree(d, false)
        },
        &mut |c, p, m| {
            calls.borrow_mut().push("refine");
            c.try_insert_knot(p, m)
        },
    )
    .unwrap();
    assert_eq!(
        *calls.borrow(),
        vec!["clamp", "elevate", "clamp", "elevate", "refine", "refine"]
    );
    assert_eq!(
        actual,
        prepare(&source, 16, limit(), WeightScale::PerSection).unwrap()
    );
    assert_eq!(source, before);
}

#[test]
fn section_budget_and_backend_layout_errors_preserve_typed_failure() {
    let source = profiles();
    let result = prepare_with_operations(
        &source,
        2,
        limit(),
        WeightScale::Common,
        &mut |_| -> Result<_, GeometryError> { panic!("over-budget input entered native clamp") },
        &mut |_, _| -> Result<_, GeometryError> { panic!("over-budget input entered elevation") },
        &mut |_, _, _| -> Result<_, GeometryError> {
            panic!("over-budget input entered refinement")
        },
    );
    assert!(matches!(
        result,
        Err(GeometryError::LoftResourceLimit { .. })
    ));
    let invalid = prepare_with_operations(
        &source,
        16,
        limit(),
        WeightScale::Common,
        &mut |c| c.try_reparameterized(10.0..=20.0),
        &mut |_, _| -> Result<_, GeometryError> { panic!("invalid clamp entered elevation") },
        &mut |_, _, _| -> Result<_, GeometryError> { panic!("invalid clamp entered refinement") },
    );
    assert!(matches!(
        invalid,
        Err(GeometryError::InvalidControlNet { .. })
    ));
}
