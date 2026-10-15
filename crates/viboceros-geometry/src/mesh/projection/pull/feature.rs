//! Affine closest maps restricted to convex triangle Voronoi regions.
use super::*;

pub(super) fn append(
    triangle: [Point3; 3],
    source: &E,
    direction: &E,
    output: &mut Vec<Feature>,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let triangle = triangle.map(exact);
    for i in 0..3 {
        let a = &triangle[i];
        let edges = [
            sub(&triangle[(i + 1) % 3], a),
            sub(&triangle[(i + 2) % 3], a),
        ];
        let mut range = [Rational::zero(), Rational::one()];
        let offset = sub(source, a);
        for edge in &edges {
            clip(&mut range, -dot(&offset, edge), -dot(direction, edge));
        }
        add(
            range,
            a.clone(),
            std::array::from_fn(|_| Rational::zero()),
            source,
            direction,
            output,
            budget,
        )?;

        let edge = &edges[0];
        let squared = dot(edge, edge);
        if squared.is_zero() {
            continue;
        }
        let start = dot(&offset, edge) / &squared;
        let speed = dot(direction, edge) / &squared;
        let fraction = dot(&edges[1], edge) / &squared;
        let inward: E = std::array::from_fn(|j| &edges[1][j] - &edge[j] * &fraction);
        let mut range = [Rational::zero(), Rational::one()];
        clip(&mut range, start.clone(), speed.clone());
        clip(&mut range, Rational::one() - &start, -&speed);
        clip(&mut range, -dot(&offset, &inward), -dot(direction, &inward));
        let origin = std::array::from_fn(|j| &a[j] + &edge[j] * &start);
        let delta = std::array::from_fn(|j| &edge[j] * &speed);
        add(range, origin, delta, source, direction, output, budget)?;
    }
    let a = &triangle[0];
    let ab = sub(&triangle[1], a);
    let ac = sub(&triangle[2], a);
    let aa = dot(&ab, &ab);
    let cc = dot(&ac, &ac);
    let cross = dot(&ab, &ac);
    let determinant = &aa * &cc - &cross * &cross;
    if determinant.is_zero() {
        return Ok(());
    }
    let barycentric = |p: &E| {
        let x = dot(p, &ab);
        let y = dot(p, &ac);
        [
            (&cc * &x - &cross * &y) / &determinant,
            (&aa * y - &cross * x) / &determinant,
        ]
    };
    let start = barycentric(&sub(source, a));
    let speed = barycentric(direction);
    let mut range = [Rational::zero(), Rational::one()];
    clip(&mut range, start[0].clone(), speed[0].clone());
    clip(&mut range, start[1].clone(), speed[1].clone());
    clip(
        &mut range,
        Rational::one() - &start[0] - &start[1],
        -&speed[0] - &speed[1],
    );
    let origin = std::array::from_fn(|j| &a[j] + &ab[j] * &start[0] + &ac[j] * &start[1]);
    let delta = std::array::from_fn(|j| &ab[j] * &speed[0] + &ac[j] * &speed[1]);
    add(range, origin, delta, source, direction, output, budget)
}

fn clip(range: &mut [Rational; 2], origin: Rational, delta: Rational) {
    if delta.is_zero() {
        if origin < Rational::zero() {
            range[1] = range[0].clone();
        }
    } else {
        let root = -origin / &delta;
        if delta > Rational::zero() {
            range[0] = range[0].clone().max(root);
        } else {
            range[1] = range[1].clone().min(root);
        }
    }
}

fn add(
    range: [Rational; 2],
    origin: E,
    delta: E,
    source: &E,
    direction: &E,
    output: &mut Vec<Feature>,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    budget.charge(1)?;
    if range[0] >= range[1] {
        return Ok(());
    }
    if output.len() >= MAX_FEATURES {
        return Err(GeometryError::MeshPullLimit);
    }
    let a = sub(source, &origin);
    let b = sub(direction, &delta);
    let distance = [dot(&a, &a), rational(2.) * dot(&a, &b), dot(&b, &b)];
    budget.check(range.iter().chain(&origin).chain(&delta).chain(&distance))?;
    output.push(Feature {
        range,
        origin,
        delta,
        distance,
    });
    Ok(())
}
