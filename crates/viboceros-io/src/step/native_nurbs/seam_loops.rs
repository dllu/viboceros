//! Choose seam p-curves by cyclic UV continuity, independently of SET ordering.
use super::*;

pub(super) fn alternatives(
    curve: &Curve3D,
    surface: &Surface,
    reversed: bool,
    id: u64,
) -> Result<Vec<NurbsCurve2>, StepError> {
    let Curve3D::SurfaceCurve(curve) = curve else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for geometry in curve.associated_geometry() {
        let SurfaceCurveAssociatedGeometry::ParameterCurve(curve) = geometry else {
            continue;
        };
        if curve.surface().as_ref() != surface {
            continue;
        }
        let curve = trim_curve(curve.curve().as_ref(), id)?;
        let curve = if reversed { curve.reversed()? } else { curve };
        if !result.contains(&curve) {
            result.push(curve);
        }
    }
    Ok(result)
}

pub(super) fn select(
    choices: &[Vec<NurbsCurve2>],
    periodic: [bool; 2],
    tolerance: Tolerance,
    id: u64,
) -> Result<Vec<NurbsCurve2>, StepError> {
    struct Node {
        curve: NurbsCurve2,
        previous: Option<usize>,
    }
    let near = |a: Point2, b: Point2| {
        (a.x() - b.x()).abs()
            <= if periodic[0] {
                tolerance.angular()
            } else {
                tolerance.absolute()
            }
            && (a.y() - b.y()).abs()
                <= if periodic[1] {
                    tolerance.angular()
                } else {
                    tolerance.absolute()
                }
    };
    let unsupported = |reason| StepError::UnsupportedNativeShell { shell: id, reason };
    let first = choices
        .first()
        .ok_or_else(|| unsupported("empty seam loop"))?;
    for anchor in first {
        let start = anchor.start_point()?;
        let mut nodes = vec![Node {
            curve: anchor.clone(),
            previous: None,
        }];
        let mut states = vec![0];
        for candidates in &choices[1..] {
            let mut next = Vec::new();
            for &previous in &states {
                let end = nodes[previous].curve.end_point()?;
                for candidate in candidates {
                    let candidate = if periodic != [false; 2] {
                        align_periodic_trim(candidate.clone(), end, periodic, tolerance)?
                    } else {
                        candidate.clone()
                    };
                    if !near(end, candidate.start_point()?) {
                        continue;
                    }
                    let candidate_end = candidate.end_point()?;
                    if next.iter().any(|&index: &usize| {
                        nodes[index]
                            .curve
                            .end_point()
                            .is_ok_and(|p| p == candidate_end)
                    }) {
                        continue;
                    }
                    nodes.push(Node {
                        curve: candidate,
                        previous: Some(previous),
                    });
                    next.push(nodes.len() - 1);
                    if next.len() > 16 {
                        return Err(unsupported("seam loop branch limit"));
                    }
                }
            }
            states = next;
            if states.is_empty() {
                break;
            }
        }
        if let Some(&last) = states.iter().find(|&&index| {
            nodes[index]
                .curve
                .end_point()
                .is_ok_and(|end| near(end, start))
        }) {
            let mut result = Vec::with_capacity(choices.len());
            let mut index = Some(last);
            while let Some(current) = index {
                result.push(nodes[current].curve.clone());
                index = nodes[current].previous;
            }
            result.reverse();
            if result.len() == choices.len() {
                return Ok(result);
            }
        }
    }
    Err(unsupported("seam p-curves cannot close the face UV loop"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(a: [f64; 2], b: [f64; 2]) -> NurbsCurve2 {
        NurbsCurve2::try_new_rational(
            1,
            vec![
                WeightedPoint2::try_new(Point2::try_new(a[0], a[1]).unwrap(), 1.).unwrap(),
                WeightedPoint2::try_new(Point2::try_new(b[0], b[1]).unwrap(), 1.).unwrap(),
            ],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    }
    #[test]
    fn cyclic_continuity_selects_seams_independently_of_array_and_loop_start_order() {
        let bottom = line([0., 0.], [360., 0.]);
        let top = line([360., 1.], [0., 1.]);
        let right = line([360., 0.], [360., 1.]);
        let wrong_right = line([0., 0.], [0., 1.]);
        let left = line([0., 1.], [0., 0.]);
        let wrong_left = line([360., 1.], [360., 0.]);
        let expected = [bottom.clone(), right.clone(), top.clone(), left.clone()];
        for reverse in [false, true] {
            for rotation in 0..4 {
                let mut choices = vec![
                    vec![bottom.clone()],
                    vec![wrong_right.clone(), right.clone()],
                    vec![top.clone()],
                    vec![wrong_left.clone(), left.clone()],
                ];
                if reverse {
                    for c in &mut choices {
                        c.reverse();
                    }
                }
                choices.rotate_left(rotation);
                let mut expected = expected.to_vec();
                expected.rotate_left(rotation);
                assert_eq!(
                    select(&choices, [false; 2], Tolerance::DEFAULT, 1).unwrap(),
                    expected
                );
            }
        }
    }
    #[test]
    fn disconnected_seam_choices_are_rejected() {
        let choices = vec![
            vec![line([0., 0.], [1., 0.])],
            vec![line([2., 0.], [0., 0.])],
        ];
        assert!(select(&choices, [false; 2], Tolerance::DEFAULT, 1).is_err());
    }
}
