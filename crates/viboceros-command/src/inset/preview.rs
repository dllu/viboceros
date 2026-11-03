//! Bounded wire previews for the staged geometry.
use super::*;
impl Plan {
    pub fn preview_edges(&self) -> Result<Vec<LineSegment>, CommandError> {
        let mut lines = vec![];
        for geometry in self.outputs.iter().flat_map(|o| &o.geometries) {
            let pairs = match geometry {
                Geometry::SubD(s) => {
                    if s.edges().len().saturating_add(lines.len()) > 400_000 {
                        return Err(too_many_span_outputs("Inset"));
                    }
                    s.edges()
                        .iter()
                        .map(|e| {
                            [
                                s.control_points()[e[0] as usize],
                                s.control_points()[e[1] as usize],
                            ]
                        })
                        .collect::<Vec<_>>()
                }
                Geometry::Mesh(m) => {
                    if m.face_count().saturating_mul(4).saturating_add(lines.len()) > 400_000 {
                        return Err(too_many_span_outputs("Inset"));
                    }
                    m.topology_edge_points()
                }
                Geometry::Brep(b) => {
                    let mut pairs = Vec::new();
                    for edge in b.edges() {
                        append_curve(edge.curve(), lines.len(), &mut pairs)?;
                    }
                    pairs
                }
                Geometry::NurbsCurve(c) => {
                    let mut pairs = Vec::new();
                    append_curve(c, lines.len(), &mut pairs)?;
                    pairs
                }
                _ => unreachable!(),
            };
            lines.extend(pairs.into_iter().filter_map(|[a, b]| {
                LineSegment::try_new(a, b, Tolerance::NUMERICAL_VALIDATION).ok()
            }));
        }
        Ok(lines)
    }
}

fn append_curve(
    curve: &NurbsCurve,
    existing: usize,
    pairs: &mut Vec<[Point3; 2]>,
) -> Result<(), CommandError> {
    let steps = if curve.degree() == 1 { 1 } else { 8 };
    let count = curve.spans().count().saturating_mul(steps);
    if existing.saturating_add(pairs.len()).saturating_add(count) > 400_000 {
        return Err(too_many_span_outputs("Inset"));
    }
    let sampler = curve.parameter_sampler()?;
    for span in sampler.spans() {
        let mut a = span.evaluate(0.)?;
        for i in 1..=steps {
            let b = span.evaluate(i as f64 / steps as f64)?;
            pairs.push([a, b]);
            a = b;
        }
    }
    Ok(())
}
