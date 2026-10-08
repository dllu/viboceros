//! Exact orthogonal projection and finite-area sets on the first support plane.
use super::*;

/// Finite-area Boolean after orthogonally projecting affine planar sheets onto
/// the first input's support. Trim holes are physical coverage, not half-spaces.
/// Classification and projected vertices remain exact until final B-rep export.
/// Collapsed edge-on inputs contribute no area. Source inputs remain unchanged.
pub fn boolean_projected_planar_breps(
    breps: &[&Brep],
    operation: BrepBooleanOperation,
    tolerance: Tolerance,
) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
    if breps.is_empty() {
        return Err(unsupported("at least one planar operand required"));
    }
    if breps.len() > 128 {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    let mut budget = Budget(EXACT_WORK_LIMIT);
    let originals = breps
        .iter()
        .map(|b| input::extract_faces(b, tolerance, &mut budget))
        .collect::<Result<Vec<_>, _>>()?;
    let reference = originals[0]
        .first()
        .ok_or_else(|| unsupported("empty planar input"))?;
    let normal = &reference.normal;
    let length = dot(normal, normal);
    check_scalar(&length)?;
    let mut projected = Vec::new();
    for (owner, polygons) in originals.iter().enumerate() {
        let first = polygons
            .first()
            .ok_or_else(|| unsupported("empty planar input"))?;
        let mut owned = Vec::new();
        for polygon in polygons {
            budget.spend(polygon.ring.len() + 1)?;
            if !zero(&cross(&first.normal, &polygon.normal))
                || !first.plane_side(&polygon.ring[0]).is_zero()
            {
                return Err(unsupported("planar sheet input required"));
            }
            let ring = polygon
                .ring
                .iter()
                .map(|p| {
                    let distance = reference.plane_side(p) / &length;
                    check_scalar(&distance)?;
                    let q: ExactPoint = std::array::from_fn(|i| &p[i] - &distance * &normal[i]);
                    check_point(&q)?;
                    Ok(q)
                })
                .collect::<Result<Vec<_>, GeometryError>>()?;
            let Some(mut ring) = clean_ring(ring, &mut budget)? else {
                continue;
            };
            let signed = (1..ring.len() - 1)
                .map(|i| {
                    dot(
                        normal,
                        &cross(&sub(&ring[i], &ring[0]), &sub(&ring[i + 1], &ring[0])),
                    )
                })
                .sum::<Rational>();
            check_scalar(&signed)?;
            if signed.is_zero() {
                continue;
            }
            if signed.is_negative() {
                ring.reverse();
            }
            let source = breps[owner]
                .faces
                .iter()
                .position(|f| std::ptr::eq(f, polygon.source))
                .unwrap();
            owned.push((
                Polygon {
                    ring,
                    normal: normal.clone(),
                    source: reference.source,
                    reversed: reference.reversed,
                },
                [owner, source],
            ));
        }
        projected.push(owned);
    }
    let mut planes = Vec::new();
    for (p, _) in projected.iter().flatten() {
        for i in 0..p.ring.len() {
            add_plane(
                &mut planes,
                Plane {
                    anchor: p.ring[i].clone(),
                    normal: cross(normal, &sub(&p.ring[(i + 1) % p.ring.len()], &p.ring[i])),
                },
                &mut budget,
            )?;
        }
    }
    let mut unique = BTreeSet::new();
    let mut selected = Vec::new();
    for (p, source) in projected.iter().flatten() {
        for ring in arrange(p.ring.clone(), &planes, &mut budget)? {
            if selected.len() >= MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
            let center = mean(&ring, &mut budget)?;
            let mut contains = Vec::new();
            for polygons in &projected {
                let mut inside = false;
                for (candidate, _) in polygons {
                    budget.spend(1)?;
                    inside |= input::inside_ring(
                        &center,
                        &candidate.ring,
                        &candidate.normal,
                        &mut budget,
                    )? == Some(true);
                }
                contains.push(inside);
            }
            let include = match operation {
                BrepBooleanOperation::Union => contains.iter().any(|v| *v),
                BrepBooleanOperation::Intersection => contains.iter().all(|v| *v),
                BrepBooleanOperation::Difference => {
                    contains[0] && !contains.iter().skip(1).any(|v| *v)
                }
            };
            if include && unique.insert(canonical_ring(&ring)) {
                selected.push((p.with_ring(ring), *source));
            }
        }
    }
    if selected.is_empty() {
        return Ok(vec![]);
    }
    let mut support = reference.source.clone();
    let mut bounds = [
        [
            *support.surface.domain_u().start(),
            *support.surface.domain_u().end(),
        ],
        [
            *support.surface.domain_v().start(),
            *support.surface.domain_v().end(),
        ],
    ];
    for (p, _) in &selected {
        for point in &p.ring {
            budget.spend(1)?;
            let uv = uv_exact(&support.surface, point)?;
            for i in 0..2 {
                let value = scalar(&uv[i])?;
                bounds[i][0] = bounds[i][0].min(value.floor());
                bounds[i][1] = bounds[i][1].max(value.ceil());
            }
        }
    }
    support.surface = merge::extended_surface(&support.surface, bounds)?;
    let selected = selected
        .into_iter()
        .map(|(p, source)| {
            (
                Polygon {
                    ring: p.ring,
                    normal: p.normal,
                    source: &support,
                    reversed: p.reversed,
                },
                source,
            )
        })
        .collect();
    let built = arrangement::Arrangement {
        operands: vec![],
        cells: vec![],
        samples: vec![],
    };
    let mut plan = BrepPolyhedralBooleanPlan::from_arrangement(built, tolerance, budget);
    plan.export_open_patches(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sheet(points: [[f64; 3]; 4]) -> Brep {
        Brep::try_surface_face(
            NurbsSurface::try_bilinear(points.map(|p| Point3::try_from(p).unwrap())).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn projection_uses_the_first_support_and_discards_collapsed_area_without_editing_inputs() {
        let a = sheet([[1., -1., -1.], [1., 3., -1.], [1., 3., 3.], [1., -1., 3.]]);
        let b = sheet([[0., 1., 0.], [1.5, 4., 0.], [1.5, 4., 2.], [0., 1., 2.]]);
        let edge_on = sheet([[-1., 1., 0.], [3., 1., 0.], [3., 1., 2.], [-1., 1., 2.]]);
        let before = (a.clone(), b.clone(), edge_on.clone());
        for (refs, areas) in [
            (vec![&a, &b], [18., 12., 4.]),
            (
                vec![&b, &a],
                [7.4 * 5f64.sqrt(), 5f64.sqrt(), 2. * 5f64.sqrt()],
            ),
            (vec![&a, &edge_on], [16., 16., 0.]),
        ] {
            for (operation, expected) in [
                BrepBooleanOperation::Union,
                BrepBooleanOperation::Difference,
                BrepBooleanOperation::Intersection,
            ]
            .into_iter()
            .zip(areas)
            {
                let bodies =
                    boolean_projected_planar_breps(&refs, operation, Tolerance::DEFAULT).unwrap();
                let area = bodies
                    .iter()
                    .map(|p| p.brep.area(Tolerance::DEFAULT).unwrap())
                    .sum::<f64>();
                assert!((area - expected).abs() < 1e-9);
                let plane = refs[0].faces()[0]
                    .surface()
                    .plane(Tolerance::DEFAULT)
                    .unwrap()
                    .unwrap();
                for body in &bodies {
                    assert!(!body.brep.is_solid());
                    assert!(
                        body.brep.vertices().iter().all(|v| plane
                            .signed_distance_to(v.point())
                            .unwrap()
                            .abs()
                            < 1e-12)
                    );
                }
            }
        }
        assert_eq!((a, b, edge_on), before);
        assert!(
            boolean_projected_planar_breps(&[], BrepBooleanOperation::Union, Tolerance::DEFAULT)
                .is_err()
        );
    }
    #[test]
    fn projection_rejects_warped_and_excessive_inputs_before_export() {
        let warped = sheet([[0., 0., 0.], [1., 0., 0.], [1., 1., 1.], [0., 1., 0.]]);
        let plane = sheet([[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]);
        assert!(
            boolean_projected_planar_breps(
                &[&plane, &warped],
                BrepBooleanOperation::Union,
                Tolerance::DEFAULT
            )
            .is_err()
        );
        assert!(matches!(
            boolean_projected_planar_breps(
                &vec![&plane; 129],
                BrepBooleanOperation::Union,
                Tolerance::DEFAULT
            ),
            Err(GeometryError::BrepBooleanWorkLimit)
        ));
    }
}
