use super::*;

pub(super) fn lift(curve: &NurbsCurve2) -> Result<NurbsCurve, GeometryError> {
    NurbsCurve::try_new_rational(
        curve.degree(),
        curve
            .control_points()
            .iter()
            .map(|p| {
                WeightedPoint3::try_new(
                    Point3::try_new(p.point().x(), p.point().y(), 0.)?,
                    p.weight(),
                )
            })
            .collect::<Result<_, _>>()?,
        curve.knots().to_vec(),
    )
}

pub(super) fn crossings(
    source: &Brep,
    face: usize,
    axis: usize,
    cut: Real,
    tolerance: Tolerance,
    budget: &mut Budget,
) -> Result<Vec<(usize, Vec<Real>)>, GeometryError> {
    let face = &source.faces[face];
    let frame = face.local_parameter_frame()?;
    let local_cut = crate::parameter::exact_difference(cut, frame.origin[axis]).ok_or(
        GeometryError::InvalidBrepTopology {
            context: "face partition knot cannot be localized exactly",
        },
    )?;
    let mut splits = BTreeMap::<usize, Vec<Real>>::new();
    for (trim, local) in face
        .loops
        .iter()
        .flat_map(|l| &l.trims)
        .zip(frame.face.loops.iter().flat_map(|l| &l.trims))
    {
        budget.charge(trim.curve.control_points().len())?;
        if super::rings::side(&trim.curve, axis, cut)?.is_some() {
            continue;
        }
        let Some(edge_index) = trim.edge else {
            // Singular UV boundaries have no spatial edge to subdivide.
            // They are partitioned separately after shared-edge certification.
            continue;
        };
        let edge = &source.edges[edge_index];
        let domain = trim.curve.domain();
        for t in crossing_parameters(&local.curve, axis, local_cut, budget)? {
            let uv = local.curve.evaluate(t)?;
            let target = frame.face.surface.evaluate(uv.x(), uv.y())?;
            let fraction = (t - *domain.start()) / (*domain.end() - *domain.start());
            let fraction = if trim.reversed_3d {
                1. - fraction
            } else {
                fraction
            };
            let proposed = edge.curve.parameter_at(fraction)?;
            // The direct map is only a proposal; the subsequently subdivided
            // trim must still land exactly on the cut and pass its certificate.
            let parameter =
                if edge.curve.evaluate(proposed)?.distance_to(target)? <= tolerance.absolute() {
                    proposed
                } else {
                    budget.charge(edge.curve.control_points().len().saturating_mul(4096))?;
                    edge.curve.closest_parameter(target, tolerance)?
                };
            if parameter <= *edge.curve.domain().start()
                || parameter >= *edge.curve.domain().end()
                || edge.curve.evaluate(parameter)?.distance_to(target)?
                    > tolerance.absolute().max(edge.tolerance)
            {
                return invalid("face partition could not resolve an edge crossing");
            }
            splits.entry(edge_index).or_default().push(parameter);
        }
    }
    let mut count = 0;
    for parameters in splits.values_mut() {
        parameters.sort_by(Real::total_cmp);
        parameters.dedup();
        count += parameters.len();
        if count > MAX_PARTS {
            return invalid("too many face partition crossings");
        }
    }
    Ok(splits.into_iter().collect())
}

pub(super) fn crossing_parameters(
    curve: &NurbsCurve2,
    axis: usize,
    cut: Real,
    budget: &mut Budget,
) -> Result<Vec<Real>, GeometryError> {
    if curve.degree() > 16 {
        return invalid("face partition root proposal exceeds degree 16");
    }
    budget.charge(
        curve
            .control_points()
            .len()
            .saturating_mul((curve.degree() + 1).pow(3)),
    )?;
    let mut roots = Vec::new();
    for span in scalar_bezier_spans(curve, axis, cut)? {
        roots_in_span(&span.coefficients, span.parameter, 0, &mut roots, budget)?;
    }
    roots.sort_by(Real::total_cmp);
    roots.dedup();
    if roots.len() > MAX_PARTS {
        return invalid("too many face partition crossings");
    }
    let domain = curve.domain();
    roots.retain(|&t| t > *domain.start() && t < *domain.end());
    for &t in &roots {
        if parameter_coordinate(curve.evaluate(t)?, axis) != cut {
            return invalid("face partition UV crossing is not exactly representable");
        }
    }
    Ok(roots)
}

fn roots_in_span(
    coefficients: &[Real],
    parameter: [Real; 2],
    depth: usize,
    roots: &mut Vec<Real>,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    budget.charge(coefficients.len().saturating_mul(coefficients.len()))?;
    if coefficients[0] == 0. {
        roots.push(parameter[0]);
    }
    if coefficients.last() == Some(&0.) {
        roots.push(parameter[1]);
    }
    if bernstein_sign_changes(coefficients) == 0 {
        return Ok(());
    }
    let middle = parameter[0].midpoint(parameter[1]);
    if depth == 64 || middle <= parameter[0] || middle >= parameter[1] {
        roots.push(middle);
        return Ok(());
    }
    let (a, b) = subdivide_bernstein_half(coefficients);
    roots_in_span(&a, [parameter[0], middle], depth + 1, roots, budget)?;
    roots_in_span(&b, [middle, parameter[1]], depth + 1, roots, budget)
}

pub(super) fn certify(
    source: &Brep,
    output: &mut Brep,
    splits: &[(usize, Vec<Real>)],
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    // Edge subdivision retains each first slot and appends the remaining
    // pieces, highest interval first, in source-edge order.
    let mut ancestry = (0..source.edges.len()).collect::<Vec<_>>();
    let counts = splits
        .iter()
        .map(|(e, p)| (*e, p.len() + 1))
        .collect::<BTreeMap<_, _>>();
    for (edge, parameters) in splits {
        ancestry.extend(std::iter::repeat_n(*edge, parameters.len()));
    }
    if ancestry.len() != output.edges.len() {
        return invalid("unexpected face partition edge table");
    }
    for (edge, &old) in output.edges.iter_mut().zip(&ancestry) {
        if counts.contains_key(&old)
            && certificate::linear_endpoints(&source.edges[old].curve).is_some()
        {
            let mut controls = edge.curve.control_points().to_vec();
            let last = controls.len() - 1;
            for (slot, vertex) in [(0, edge.vertices[0]), (last, edge.vertices[1])] {
                if vertex >= source.vertices.len() {
                    controls[slot] = WeightedPoint3::try_new(
                        output.vertices[vertex].point,
                        controls[slot].weight(),
                    )?;
                }
            }
            edge.curve = NurbsCurve::try_new_rational(
                edge.curve.degree(),
                controls,
                edge.curve.knots().to_vec(),
            )?;
        }
    }
    let mut partitions = counts
        .keys()
        .map(|&e| (e, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (edge, &old) in output.edges.iter().zip(&ancestry) {
        if let Some(pieces) = partitions.get_mut(&old) {
            pieces.push(&edge.curve);
        }
    }
    for (old, mut pieces) in partitions {
        pieces.sort_by(|a, b| a.domain().start().total_cmp(b.domain().start()));
        exact_partition(&pieces, &source.edges[old].curve, budget)?;
    }
    for (old_face, new_face) in source.faces.iter().zip(&mut output.faces) {
        for (old_ring, new_ring) in old_face.loops.iter().zip(&mut new_face.loops) {
            let mut next = 0;
            for old in &old_ring.trims {
                let count = old.edge.and_then(|e| counts.get(&e)).copied().unwrap_or(1);
                let pieces = new_ring.trims.get_mut(next..next + count).ok_or(
                    GeometryError::InvalidBrepTopology {
                        context: "face partition lost a trim",
                    },
                )?;
                if count == 1 {
                    if pieces[0] != *old {
                        return invalid("face partition changed an untouched trim");
                    }
                } else {
                    let original = lift(&old.curve)?;
                    if certificate::linear_endpoints(&original).is_some() {
                        for piece in pieces.iter_mut() {
                            // Floating de Casteljau subdivision and direct
                            // evaluation can round a line endpoint differently.
                            // Propose the original evaluated point, then prove
                            // the entire ordered partition has the same locus.
                            let domain = piece.curve.domain();
                            let mut controls = piece.curve.control_points().to_vec();
                            let last = controls.len() - 1;
                            for (slot, t) in [(0, *domain.start()), (last, *domain.end())] {
                                controls[slot] = WeightedPoint2::try_new(
                                    old.curve.evaluate(t)?,
                                    controls[slot].weight(),
                                )?;
                            }
                            piece.curve = NurbsCurve2::try_new_rational(
                                piece.curve.degree(),
                                controls,
                                piece.curve.knots().to_vec(),
                            )?;
                        }
                    }
                    let pieces = pieces
                        .iter()
                        .map(|p| lift(&p.curve))
                        .collect::<Result<Vec<_>, _>>()?;
                    exact_partition(&pieces.iter().collect::<Vec<_>>(), &original, budget)?;
                }
                next += count;
            }
            if next != new_ring.trims.len() {
                return invalid("face partition gained an unexpected trim");
            }
        }
    }
    Ok(())
}

pub(super) fn exact_partition(
    pieces: &[&NurbsCurve],
    original: &NurbsCurve,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    let mut end = *original.domain().start();
    for piece in pieces {
        if *piece.domain().start() != end {
            return invalid("face partition left a parameter interval gap");
        }
        end = *piece.domain().end();
    }
    if end != *original.domain().end() {
        return invalid("face partition lost a parameter interval");
    }
    // A line cut at e.g. one third has no exact binary64 source parameter.
    // Certify the ordered *loci* instead: all pieces must be exact segments,
    // share exact endpoints, and cover the original segment monotonically.
    // Merely observing collinear endpoints or accepting a small UV error would
    // be insufficient for rational or reversing curves.
    if let Some(endpoints) = certificate::linear_endpoints(original) {
        budget.charge(original.control_points().len())?;
        let mut points = vec![endpoints[0]];
        for piece in pieces {
            budget.charge(piece.control_points().len())?;
            let Some([a, b]) = certificate::linear_endpoints(piece) else {
                return invalid("face partition changed a straight curve's locus");
            };
            if points.last() != Some(&a) {
                return invalid("face partition line pieces have a gap");
            }
            points.push(b);
        }
        if points.last() != Some(&endpoints[1]) {
            return invalid("face partition line pieces lost an endpoint");
        }
        let n = points.len();
        let mut knots = vec![0., 0.];
        knots.extend((1..n).map(|i| i as Real));
        knots.push((n - 1) as Real);
        let chain = NurbsCurve::try_new(1, points, knots)?;
        if certificate::linear_endpoints(&chain) != Some(endpoints) {
            return invalid("face partition line pieces reverse or leave their source");
        }
        return Ok(());
    }
    for piece in pieces {
        let domain = piece.domain();
        if certificate::restricted_curve_bound(
            piece,
            original,
            [*domain.start(), *domain.end()],
            0.,
            false,
            |n| budget.charge(n),
        )? != Some(0.)
        {
            return invalid("face partition cannot certify an exact curve restriction");
        }
    }
    Ok(())
}
