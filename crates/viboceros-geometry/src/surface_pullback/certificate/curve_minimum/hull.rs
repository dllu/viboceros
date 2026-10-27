//! Exact extracted span hierarchy and separating-axis lower bounds.
use super::*;
pub(super) struct Hull {
    pub(super) range: [Rational; 2],
    pub(super) bounds: [P; 2],
    pub(super) points: Rc<Vec<P>>,
    pub(super) indices: Range<usize>,
    pub(super) net: Option<Vec<H>>,
    pub(super) depth: usize,
    pub(super) children: OnceCell<[Rc<Hull>; 2]>,
}

impl Hull {
    fn bezier(
        net: Vec<H>,
        range: [Rational; 2],
        depth: usize,
        budget: &mut Budget,
    ) -> Result<Rc<Self>, GeometryError> {
        let mut points = Vec::with_capacity(net.len());
        for p in &net {
            if p[3] <= Rational::zero() {
                return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
            }
            let point: P = std::array::from_fn(|i| &p[i] / &p[3]);
            for x in &point {
                budget.check(x)?;
            }
            points.push(point);
        }
        let mut bounds = [points[0].clone(), points[0].clone()];
        for p in &points {
            for i in 0..3 {
                bounds[0][i] = bounds[0][i].clone().min(p[i].clone());
                bounds[1][i] = bounds[1][i].clone().max(p[i].clone());
            }
        }
        let count = points.len();
        Ok(Rc::new(Self {
            range,
            bounds,
            points: Rc::new(points),
            indices: 0..count,
            net: Some(net),
            depth,
            children: OnceCell::new(),
        }))
    }

    pub(super) fn children(&self, budget: &mut Budget) -> Result<[Rc<Self>; 2], GeometryError> {
        if let Some(children) = self.children.get() {
            return Ok(children.clone());
        }
        if self.depth >= MAX_DEPTH {
            return Err(GeometryError::SurfaceCurveCertificateWorkLimit);
        }
        let (a, b) = algebra::split(self.net.as_ref().unwrap(), budget)?;
        let middle = (&self.range[0] + &self.range[1]) / rational(2.);
        let children = [
            Self::bezier(
                a,
                [self.range[0].clone(), middle.clone()],
                self.depth + 1,
                budget,
            )?,
            Self::bezier(b, [middle, self.range[1].clone()], self.depth + 1, budget)?,
        ];
        let _ = self.children.set(children.clone());
        Ok(children)
    }

    pub(super) fn width(&self) -> Rational {
        (0..3)
            .map(|i| &self.bounds[1][i] - &self.bounds[0][i])
            .max()
            .unwrap()
    }
}

pub(super) fn tree(
    curve: &NurbsCurve,
    spline: &curve::Spline<4>,
    budget: &mut Budget,
) -> Result<Rc<Hull>, GeometryError> {
    let points = Rc::new(
        curve
            .control_points()
            .iter()
            .map(|p| p.point().to_array().map(rational))
            .collect::<Vec<_>>(),
    );
    let domain = curve.domain();
    let start = rational(*domain.start());
    let length = rational(*domain.end()) - &start;
    let mut leaves = Vec::new();
    for span in curve.degree()..curve.control_points().len() {
        let (a, b) = (curve.knots()[span], curve.knots()[span + 1]);
        if a == b {
            continue;
        }
        let range = [
            (rational(a) - &start) / &length,
            (rational(b) - &start) / &length,
        ];
        let net = spline.extract(&range[0], &range[1], budget)?;
        leaves.push((
            Hull::bezier(net, range, 0, budget)?,
            span - curve.degree()..span + 1,
        ));
    }
    fn join(
        leaves: &[(Rc<Hull>, Range<usize>)],
        points: &Rc<Vec<P>>,
        budget: &mut Budget,
    ) -> Result<Rc<Hull>, GeometryError> {
        budget.charge(1)?;
        if leaves.len() == 1 {
            return Ok(leaves[0].0.clone());
        }
        let middle = leaves.len() / 2;
        let a = join(&leaves[..middle], points, budget)?;
        let b = join(&leaves[middle..], points, budget)?;
        let bounds = std::array::from_fn(|end| {
            std::array::from_fn(|i| {
                if end == 0 {
                    a.bounds[end][i].clone().min(b.bounds[end][i].clone())
                } else {
                    a.bounds[end][i].clone().max(b.bounds[end][i].clone())
                }
            })
        });
        let children = OnceCell::new();
        let _ = children.set([a.clone(), b.clone()]);
        Ok(Rc::new(Hull {
            range: [a.range[0].clone(), b.range[1].clone()],
            bounds,
            points: points.clone(),
            indices: leaves[0].1.start..leaves.last().unwrap().1.end,
            net: None,
            depth: 0,
            children,
        }))
    }
    join(&leaves, &points, budget)
}

pub(super) fn box_lower(a: &Hull, b: &Hull) -> Rational {
    (0..3)
        .map(|i| {
            let gap = (&a.bounds[0][i] - &b.bounds[1][i])
                .max(&b.bounds[0][i] - &a.bounds[1][i])
                .max(Rational::zero());
            &gap * &gap
        })
        .sum()
}

pub(super) fn axis_lower(
    a: &Hull,
    b: &Hull,
    axis: &P,
    budget: &mut Budget,
) -> Result<Rational, GeometryError> {
    let square: Rational = axis.iter().map(|x| x * x).sum();
    if square.is_zero() {
        return Ok(Rational::zero());
    }
    budget.check(&square)?;
    let mut extent = |hull: &Hull| -> Result<[Rational; 2], GeometryError> {
        let mut values = hull.points[hull.indices.clone()]
            .iter()
            .map(|p| p.iter().zip(axis).map(|(x, a)| x * a).sum::<Rational>());
        let first = values.next().unwrap();
        budget.check(&first)?;
        let mut result = [first.clone(), first];
        for value in values {
            budget.check(&value)?;
            result[0] = result[0].clone().min(value.clone());
            result[1] = result[1].clone().max(value);
        }
        Ok(result)
    };
    let x = extent(a)?;
    let y = extent(b)?;
    let gap = (&x[0] - &y[1]).max(&y[0] - &x[1]).max(Rational::zero());
    let result = &gap * &gap / square;
    budget.check(&result)?;
    Ok(result)
}
