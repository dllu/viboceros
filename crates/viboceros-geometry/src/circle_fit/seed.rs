use super::*;

/// The compact Kasa solve is sufficient for ordinary data; thin data needs
/// Taubin's gradient-normalized algebraic fit to avoid Kasa's radius bias.
pub(super) fn algebraic(
    points: &[[Real; 3]],
    well_conditioned: bool,
) -> Result<Vector2<Real>, GeometryError> {
    if well_conditioned {
        let mut mean = [0.; 3];
        for p in points {
            mean[0] += p[0];
            mean[1] += p[1];
            mean[2] += p.iter().map(|v| v * v).sum::<Real>();
        }
        mean.iter_mut().for_each(|v| *v /= points.len() as Real);
        let mut matrix = Matrix2::zeros();
        let mut rhs = Vector2::zeros();
        for p in points {
            let d = Vector2::new(p[0] - mean[0], p[1] - mean[1]);
            matrix += d * d.transpose();
            rhs += d * ((p.iter().map(|v| v * v).sum::<Real>() - mean[2]) * 0.5);
        }
        let center = matrix
            .lu()
            .solve(&rhs)
            .filter(|v| v.iter().all(|x| x.is_finite()))
            .ok_or(GeometryError::CircleFitDidNotConverge)?;
        if center.norm() <= 16. {
            return Ok(center);
        }
    }
    let squares = points
        .iter()
        .map(|p| p.iter().map(|v| v * v).sum::<Real>())
        .collect::<Vec<_>>();
    let mut sum = FiniteSum::default();
    for &q in &squares {
        sum.add(q)?;
    }
    let mean = sum.mean()?;
    let normalization = 2. * mean.sqrt();
    // Minimize ||a(q-mean(q)) + b*x + c*y|| subject to
    // 4*mean(q)*a^2 + b^2 + c^2 = 1, using the smallest right singular vector.
    let design = Mat::from_fn(points.len(), 3, |r, c| {
        if c == 0 {
            (squares[r] - mean) / normalization
        } else {
            points[r][c - 1]
        }
    });
    let svd = design
        .thin_svd()
        .map_err(|_| GeometryError::CircleFitDidNotConverge)?;
    let mut vector: [Real; 3] = std::array::from_fn(|i| svd.V()[(i, 2)]);
    let fixed = (0..3)
        .max_by(|&a, &b| vector[a].abs().total_cmp(&vector[b].abs()))
        .unwrap();
    let free = (0..3).filter(|&i| i != fixed).collect::<Vec<_>>();
    let eigenvalue = svd.S().column_vector()[2].powi(2);
    let entry = |i: usize, j: usize| -> Result<Real, GeometryError> {
        let mut sum = FiniteSum::default();
        for r in 0..points.len() {
            sum.add(design[(r, i)] * design[(r, j)])?;
        }
        sum.total()
    };
    // Recover small eigenvector components from the well-conditioned block,
    // rather than dividing SVD's absolute vector error by a tiny curvature.
    let matrix = Matrix2::new(
        entry(free[0], free[0])? - eigenvalue,
        entry(free[0], free[1])?,
        entry(free[0], free[1])?,
        entry(free[1], free[1])? - eigenvalue,
    );
    if let Some(solution) = matrix
        .lu()
        .solve(&Vector2::new(
            -entry(free[0], fixed)?,
            -entry(free[1], fixed)?,
        ))
        .filter(|v| v.iter().all(|x| x.is_finite()))
    {
        vector[fixed] = 1.;
        vector[free[0]] = solution[0];
        vector[free[1]] = solution[1];
    }
    let a = vector[0] / normalization;
    let center = Vector2::new(-vector[1] / (2. * a), -vector[2] / (2. * a));
    if a == 0. || center.iter().any(|x| !x.is_finite()) {
        return Err(GeometryError::CircleFitDidNotConverge);
    }
    Ok(center)
}
