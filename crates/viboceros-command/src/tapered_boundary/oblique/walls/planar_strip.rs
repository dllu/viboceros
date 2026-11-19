//! Cut regular ruled strips wherever the planar section spans faces.
use super::*;
use crate::curve_intersections::brep_events;

mod partition;

pub(super) fn retain(
    stock: &Brep,
    components: &[Vec<usize>],
    cutter: &Brep,
    plane: Plane,
    displacement: Vector3,
    closed: bool,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let alignment = displacement.dot(plane.normal().as_vector())?;
    let cut = PlaneCut {
        cutter,
        plane,
        displacement,
        alignment,
        tolerance,
    };
    let [first, rest @ ..] = components else {
        return Err(REJECTED);
    };
    let (leading, divided) = cut.component(stock.sub_brep(first, tolerance)?, !rest.is_empty())?;
    if closed
        && divided
        && (alignment > 0.) != cutter.faces()[0].is_reversed()
        && let Some(last) = rest
            .iter()
            .find(|component| component.last().copied() == Some(stock.faces().len() - 1))
    {
        let (closing, divided) = cut.component(stock.sub_brep(last, tolerance)?, true)?;
        if !divided {
            return Ok(closing);
        }
    }
    Ok(leading)
}

#[derive(Clone, Copy)]
struct Ruling {
    base: Point3,
    hit: Point3,
}

struct PlaneCut<'a> {
    cutter: &'a Brep,
    plane: Plane,
    displacement: Vector3,
    alignment: f64,
    tolerance: Tolerance,
}

#[derive(Clone, Copy, Default)]
struct Coverage {
    complete: bool,
    reached: [bool; 2],
}

impl PlaneCut<'_> {
    fn component(&self, stock: Brep, allow_partial: bool) -> Result<(Brep, bool), CommandError> {
        let rulings = stock
            .faces()
            .iter()
            .map(|face| self.rulings(face.surface()))
            .collect::<Result<Vec<_>, _>>()?;
        // Intersect the entire wall with the cutter's support plane. The wall
        // below the section can extend beyond the finite cutter's footprint.
        let support = covering_support(&stock, self.plane, self.displacement, self.tolerance)?;
        let tool = boundary_tool(&support, self.displacement.scaled(-2.)?, self.tolerance)?;
        let section = rulings
            .iter()
            .flatten()
            .flat_map(|ruling| [ruling.base, ruling.hit]);
        let cut = clip_inside(&stock, &tool, section, self.tolerance)?;
        let mut traces = vec![None; cut.edges().len()];
        let mut count = 0;
        let mut complete = true;
        for (index, edge) in cut.edges().iter().enumerate() {
            if !self.on_plane(edge.curve()) {
                continue;
            }
            let coverage = self.coverage(edge.curve())?;
            traces[index] = Some(coverage);
            count += 1;
            complete &= coverage.complete;
        }
        if count != stock.faces().len() {
            return Err(REJECTED);
        }
        if complete {
            return Ok((cut, true));
        }
        if !allow_partial {
            return Err(REJECTED);
        }
        Ok((
            partition::apply(self, stock, &cut, &tool, &rulings, &traces)?,
            false,
        ))
    }

    fn rulings(&self, wall: &NurbsSurface) -> Result<[Ruling; 2], CommandError> {
        validate_wall(
            wall,
            Some((self.plane, self.alignment.signum())),
            self.tolerance,
        )?;
        let u = wall.domain_u();
        let v = wall.domain_v();
        let ruling = |v| {
            let base = wall.evaluate(*u.start(), v)?;
            let end = wall.evaluate(*u.end(), v)?;
            let a = self.plane.signed_distance_to(base)?;
            let b = self.plane.signed_distance_to(end)?;
            let hit = wall.evaluate(wall.parameter_at_u(a / (a - b))?, v)?;
            Ok::<_, GeometryError>(Ruling { base, hit })
        };
        Ok([ruling(*v.start())?, ruling(*v.end())?])
    }

    fn on_plane(&self, curve: &NurbsCurve) -> bool {
        curve.control_points().iter().all(|control| {
            self.plane
                .signed_distance_to(control.point())
                .is_ok_and(|distance| distance.abs() <= self.tolerance.absolute())
        })
    }

    fn coverage(&self, curve: &NurbsCurve) -> Result<Coverage, CommandError> {
        let mut coverage = Coverage::default();
        let domain = curve.domain();
        for event in brep_events(curve, self.cutter, None, self.tolerance)? {
            if let CurveBrepIntersectionEvent::Overlap(overlap) = event {
                let interval = overlap.curve_interval();
                let start = interval.start() == domain.start();
                let end = interval.end() == domain.end();
                coverage.complete |= start && end;
                coverage.reached[0] |= start;
                coverage.reached[1] |= end;
            }
        }
        Ok(coverage)
    }
}

fn covering_support(
    stock: &Brep,
    plane: Plane,
    displacement: Vector3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let chart = Frame3::try_from_unit_normal(plane.origin(), plane.normal())?;
    let rise = displacement.dot(plane.normal().as_vector())?;
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    // Projection along the sweep is affine. The nonrational stock's control
    // hull encloses its projection, so the tool's sides miss the entire strip.
    for face in stock.faces() {
        for control in face.surface().control_points() {
            let point = control.point();
            let fraction = -plane.signed_distance_to(point)? / rise;
            let projected = point.translated(displacement.scaled(fraction)?)?;
            let xy = chart.projected_coordinates_of(projected)?;
            for axis in 0..2 {
                min[axis] = min[axis].min(xy[axis]);
                max[axis] = max[axis].max(xy[axis]);
            }
        }
    }
    let pad = displacement.length()?.max(tolerance.absolute() * 16.);
    let [left, bottom] = min.map(|v| v - pad);
    let [right, top] = max.map(|v| v + pad);
    let surface = NurbsSurface::try_bilinear([
        chart.point_at([left, bottom, 0.])?,
        chart.point_at([right, bottom, 0.])?,
        chart.point_at([right, top, 0.])?,
        chart.point_at([left, top, 0.])?,
    ])?;
    Ok(Brep::try_surface_face(surface, tolerance)?)
}
