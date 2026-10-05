//! Model-space certificates; rounded UV endpoints are bounded, never snapped.
use super::*;

pub(super) fn extract<'a>(
    brep: &'a Brep,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<Polygon<'a>>, GeometryError> {
    if brep.faces.len() > MAX_INPUT_FACES {
        return Err(GeometryError::BrepBooleanWorkLimit);
    }
    if !brep.is_solid() {
        return Err(unsupported("closed manifold shells are required"));
    }
    let mut output = Vec::new();
    for face in &brep.faces {
        let surface = &face.surface;
        let controls = surface.control_points();
        if surface.degree_u() != 1
            || surface.degree_v() != 1
            || controls.len() != 4
            || controls.iter().any(|c| c.weight() != controls[0].weight())
            || [surface.knots_u(), surface.knots_v()]
                .iter()
                .any(|k| k.len() != 4 || k[0] != k[1] || k[2] != k[3])
        {
            return Err(unsupported(
                "exact affine clamped bilinear surfaces are required",
            ));
        }
        let origin = point(controls[0].point());
        let u = sub(&point(controls[1].point()), &origin);
        let v = sub(&point(controls[2].point()), &origin);
        let normal = cross(&u, &v);
        check_point(&normal)?;
        if zero(&normal)
            || (0..3)
                .any(|i| rational(controls[3].point().to_array()[i]) != &origin[i] + &u[i] + &v[i])
        {
            return Err(unsupported("nonaffine or degenerate surface control net"));
        }
        let mut loops = Vec::new();
        for boundary in &face.loops {
            let mut ring = Vec::new();
            for trim in &boundary.trims {
                budget.spend(1)?;
                let p = point(brep.vertices[trim.vertices[0]].point);
                let q = point(brep.vertices[trim.vertices[1]].point);
                if !dot(&normal, &sub(&p, &origin)).is_zero()
                    || !dot(&normal, &sub(&q, &origin)).is_zero()
                {
                    return Err(unsupported("model vertices are not exactly coplanar"));
                }
                let edge = &brep.edges[trim.edge.ok_or_else(|| unsupported("singular trims"))?];
                let c = edge.curve.control_points();
                budget.spend(c.len())?;
                if !straight_edge(edge)
                    || c[0].point() != brep.vertices[edge.vertices[0]].point
                    || c.last().unwrap().point() != brep.vertices[edge.vertices[1]].point
                {
                    return Err(unsupported("certified clamped straight edges are required"));
                }
                let c = trim.curve.control_points();
                budget.spend(c.len())?;
                if !trim.curve.is_straight_segment() {
                    return Err(unsupported("certified clamped straight trims are required"));
                }
                for (c, expected) in [c.first().unwrap(), c.last().unwrap()]
                    .into_iter()
                    .zip([&p, &q])
                {
                    let parameter = c.point().to_array().map(rational);
                    let domains = [surface.domain_u(), surface.domain_v()];
                    if (0..2).any(|i| {
                        parameter[i] < rational(*domains[i].start())
                            || parameter[i] > rational(*domains[i].end())
                    }) {
                        return Err(unsupported(
                            "trim endpoint outside supporting surface domain",
                        ));
                    }
                    let fractions: [Rational; 2] = std::array::from_fn(|i| {
                        (&parameter[i] - rational(*domains[i].start()))
                            / (rational(*domains[i].end()) - rational(*domains[i].start()))
                    });
                    let lifted: ExactPoint = std::array::from_fn(|i| {
                        &origin[i] + &fractions[0] * &u[i] + &fractions[1] * &v[i]
                    });
                    check_point(&lifted)?;
                    let error = sub(&lifted, expected);
                    // Both images are straight segments. The norm of their
                    // affine correspondence is bounded by its endpoint norms;
                    // same-sign rational weights only change parameter speed.
                    if dot(&error, &error)
                        > rational(tolerance.absolute()) * rational(tolerance.absolute())
                    {
                        return Err(unsupported(
                            "straight trim/model correspondence exceeds tolerance",
                        ));
                    }
                }
                ring.push(p);
            }
            loops.push(ring);
        }
        certify_loops(&loops, &normal, budget)?;
        let rings = decompose(&loops, &normal, surface, budget)?;
        for ring in rings {
            let mut polygon = Polygon {
                ring,
                normal: normal.clone(),
                source: face,
                reversed: face.reversed,
            };
            if face.reversed {
                polygon.ring.reverse();
                polygon.normal = polygon.normal.map(|c| -c);
            }
            output.push(polygon);
            if output.len() > MAX_OUTPUT_FACES {
                return Err(GeometryError::BrepBooleanWorkLimit);
            }
        }
    }
    embedding::certify(brep, &output, budget)?;
    Ok(output)
}

pub(super) fn certify_loops(
    loops: &[Vec<ExactPoint>],
    normal: &ExactPoint,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    for (index, ring) in loops.iter().enumerate() {
        if ring.len() < 3 || ring.iter().collect::<BTreeSet<_>>().len() != ring.len() {
            return Err(unsupported("degenerate or repeated loop vertices"));
        }
        let mut area = Rational::zero();
        for i in 0..ring.len() {
            let (a, b, c) = (
                &ring[i],
                &ring[(i + 1) % ring.len()],
                &ring[(i + 2) % ring.len()],
            );
            budget.spend(1)?;
            if zero(&cross(&sub(b, a), &sub(c, b))) && !dot(&sub(b, a), &sub(c, b)).is_positive() {
                return Err(unsupported("backtracking polygon boundary"));
            }
            area += dot(normal, &cross(&sub(a, &ring[0]), &sub(b, &ring[0])));
            check_scalar(&area)?;
            for j in i + 1..ring.len() {
                if j == i + 1 || (i == 0 && j == ring.len() - 1) {
                    continue;
                }
                if !segment_contacts(a, b, &ring[j], &ring[(j + 1) % ring.len()], normal, budget)?
                    .is_empty()
                {
                    return Err(unsupported(
                        "self-intersecting or touching polygon boundary",
                    ));
                }
            }
        }
        if if index == 0 {
            !area.is_positive()
        } else {
            !area.is_negative()
        } {
            return Err(unsupported(
                "outer and hole loops must have natural surface winding",
            ));
        }
        for previous in &loops[..index] {
            for i in 0..ring.len() {
                for j in 0..previous.len() {
                    if !segment_contacts(
                        &ring[i],
                        &ring[(i + 1) % ring.len()],
                        &previous[j],
                        &previous[(j + 1) % previous.len()],
                        normal,
                        budget,
                    )?
                    .is_empty()
                    {
                        return Err(unsupported("face boundaries intersect or touch"));
                    }
                }
            }
        }
        if index > 0 {
            if inside_ring(&ring[0], &loops[0], normal, budget)? != Some(true) {
                return Err(unsupported("hole is not strictly inside outer loop"));
            }
            for other in &loops[1..index] {
                if inside_ring(&ring[0], other, normal, budget)? != Some(false)
                    || inside_ring(&other[0], ring, normal, budget)? != Some(false)
                {
                    return Err(unsupported("overlapping or nested face holes"));
                }
            }
        }
    }
    Ok(())
}

fn decompose(
    loops: &[Vec<ExactPoint>],
    normal: &ExactPoint,
    surface: &NurbsSurface,
    budget: &mut Budget,
) -> Result<Vec<Vec<ExactPoint>>, GeometryError> {
    let ring = &loops[0];
    let convex = loops.len() == 1
        && (0..ring.len()).all(|i| {
            !dot(
                normal,
                &cross(
                    &sub(&ring[(i + 1) % ring.len()], &ring[i]),
                    &sub(&ring[(i + 2) % ring.len()], &ring[(i + 1) % ring.len()]),
                ),
            )
            .is_negative()
        });
    if convex {
        return Ok(clean_ring(ring.clone(), budget)?.into_iter().collect());
    }
    let parameters = loops
        .iter()
        .flatten()
        .map(|p| uv_exact(surface, p))
        .collect::<Result<Vec<_>, _>>()?;
    let bounds: [[Rational; 2]; 2] = std::array::from_fn(|i| {
        [
            parameters.iter().map(|p| p[i].clone()).min().unwrap(),
            parameters.iter().map(|p| p[i].clone()).max().unwrap(),
        ]
    });
    let controls = surface.control_points();
    let origin = point(controls[0].point());
    let u = sub(&point(controls[1].point()), &origin);
    let v = sub(&point(controls[2].point()), &origin);
    let domains = [surface.domain_u(), surface.domain_v()];
    let rectangle = [[0, 0], [1, 0], [1, 1], [0, 1]]
        .map(|corners| {
            let t: [Rational; 2] = std::array::from_fn(|i| {
                (&bounds[i][corners[i]] - rational(*domains[i].start()))
                    / (rational(*domains[i].end()) - rational(*domains[i].start()))
            });
            std::array::from_fn(|i| &origin[i] + &t[0] * &u[i] + &t[1] * &v[i])
        })
        .to_vec();
    let mut planes = Vec::new();
    for ring in loops {
        for i in 0..ring.len() {
            add_plane(
                &mut planes,
                Plane {
                    anchor: ring[i].clone(),
                    normal: cross(normal, &sub(&ring[(i + 1) % ring.len()], &ring[i])),
                },
                budget,
            )?;
        }
    }
    let mut output = Vec::new();
    for cell in arrange(rectangle, &planes, budget)? {
        let center = mean(&cell, budget)?;
        let mut inside = true;
        for (i, ring) in loops.iter().enumerate() {
            let value = inside_ring(&center, ring, normal, budget)?
                .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            inside &= if i == 0 { value } else { !value };
        }
        if inside {
            output.push(cell);
        }
    }
    Ok(output)
}

fn axes(normal: &ExactPoint) -> (usize, usize) {
    let omitted = normal.iter().position(|c| !c.is_zero()).unwrap();
    ((omitted + 1) % 3, (omitted + 2) % 3)
}

pub(super) fn inside_ring(
    p: &ExactPoint,
    ring: &[ExactPoint],
    normal: &ExactPoint,
    budget: &mut Budget,
) -> Result<Option<bool>, GeometryError> {
    let (x, y) = axes(normal);
    let mut winding = 0;
    for i in 0..ring.len() {
        budget.spend(1)?;
        let (a, b) = (&ring[i], &ring[(i + 1) % ring.len()]);
        let determinant = (&b[x] - &a[x]) * (&p[y] - &a[y]) - (&b[y] - &a[y]) * (&p[x] - &a[x]);
        check_scalar(&determinant)?;
        if determinant.is_zero() && on_segment(p, a, b) {
            return Ok(None);
        }
        if a[y] <= p[y] && b[y] > p[y] && determinant.is_positive() {
            winding += 1;
        }
        if a[y] > p[y] && b[y] <= p[y] && determinant.is_negative() {
            winding -= 1;
        }
    }
    Ok(Some(winding != 0))
}

pub(super) fn on_segment(p: &ExactPoint, a: &ExactPoint, b: &ExactPoint) -> bool {
    zero(&cross(&sub(p, a), &sub(b, a)))
        && (0..3).all(|i| {
            p[i] >= a[i].clone().min(b[i].clone()) && p[i] <= a[i].clone().max(b[i].clone())
        })
}

pub(super) fn segment_contacts(
    a: &ExactPoint,
    b: &ExactPoint,
    c: &ExactPoint,
    d: &ExactPoint,
    normal: &ExactPoint,
    budget: &mut Budget,
) -> Result<Vec<ExactPoint>, GeometryError> {
    budget.spend(1)?;
    let (x, y) = axes(normal);
    let (u, v, w) = (sub(b, a), sub(d, c), sub(c, a));
    let determinant = &u[x] * &v[y] - &u[y] * &v[x];
    check_scalar(&determinant)?;
    if determinant.is_zero() {
        return Ok([a, b, c, d]
            .into_iter()
            .filter(|p| on_segment(p, a, b) && on_segment(p, c, d))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect());
    }
    let s = (&w[x] * &v[y] - &w[y] * &v[x]) / &determinant;
    let t = (&w[x] * &u[y] - &w[y] * &u[x]) / &determinant;
    check_scalar(&s)?;
    check_scalar(&t)?;
    if s < Rational::zero() || s > rational(1.) || t < Rational::zero() || t > rational(1.) {
        return Ok(Vec::new());
    }
    let p = std::array::from_fn(|i| &a[i] + &s * &u[i]);
    check_point(&p)?;
    Ok(vec![p])
}

/// C0 continuity and a same-sign rational convex hull certify the whole image,
/// including the collinear splines produced by MergeAllEdges.
fn straight_edge(edge: &BrepEdge) -> bool {
    let curve = &edge.curve;
    let controls = curve.control_points();
    let knots = curve.knots();
    let degree = curve.degree();
    if knots[0] != knots[degree] || knots[controls.len()] != *knots.last().unwrap() {
        return false;
    }
    let mut multiplicity = 1;
    for pair in knots[degree + 1..controls.len()].windows(2) {
        multiplicity = if pair[0] == pair[1] {
            multiplicity + 1
        } else {
            1
        };
        if multiplicity > degree {
            return false;
        }
    }
    let a = point(controls[0].point());
    let b = point(controls.last().unwrap().point());
    a != b
        && controls.iter().all(|c| {
            c.weight().is_sign_positive() == controls[0].weight().is_sign_positive()
                && on_segment(&point(c.point()), &a, &b)
        })
}
