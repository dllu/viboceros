//! Simple-loop admission, pair intersections and exact-trim nesting queries.
use super::*;
use crate::{BoundingBox3, Circle3, Curve3, CurveSegment3, Point2, PolyCurve3};

struct Region {
    geometry: NurbsCurve,
    shape: Curve3,
    face: BrepFace,
    start: Point2,
    bounds: BoundingBox3,
    padding: Real,
}
impl Region {
    fn new(
        brep: &Brep,
        face: &BrepFace,
        trims: &[BrepTrim],
        outer: bool,
        normal: UnitVector3,
        tolerance: Tolerance,
        operations: Option<CurveOffsetOperations>,
    ) -> Result<Self, GeometryError> {
        let geometry = curves::joined(brep, trims)?;
        if !geometry.is_closed()? {
            return Err(invalid("inset requires closed boundary geometry"));
        }
        // Extract once for the cheap whole-control sector certificate. Exact
        // subdivision does not change the physical or parameter-space locus.
        let spans = geometry.try_bezier_spans()?;
        let split = PolyCurve3::try_new(
            spans
                .into_iter()
                .map(CurveSegment3::NurbsCurve)
                .collect::<Vec<_>>(),
        )?
        .to_nurbs()?;
        let uv = face.surface.try_pullback_affine_curve(&split, tolerance)?;
        let orientation = if let Some(ccw) = curved_boundaries::simple_loop_orientation(&uv) {
            ccw
        } else {
            let sign = Curve3::NurbsCurve(geometry.clone())
                .offset_region_inward_sign_with_operations(normal, tolerance, operations)?
                .ok_or_else(|| invalid("inset boundary has no simple closed region"))?;
            sign > 0.
        };
        if orientation != outer {
            return Err(invalid("inset boundary winding is inconsistent"));
        }
        let tight = Tolerance::try_new(
            tolerance.absolute() * 0.01,
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let shape = if let Some(center) = geometry.circular_center(tight)? {
            let radius = center.distance_to(geometry.evaluate(*geometry.domain().start())?)?;
            if (geometry.length(tight)? - std::f64::consts::TAU * radius).abs()
                > 16. * tolerance.absolute()
            {
                return Err(invalid("inset circular boundary winds more than once"));
            }
            Curve3::Circle(Circle3::try_new(center, radius, normal, tolerance)?)
        } else {
            Curve3::NurbsCurve(geometry.clone())
        };
        let start = uv.start_point()?;
        let uv = if outer { uv } else { uv.reversed()? };
        let trim = BrepTrim::try_new(
            [0, 0],
            Some(0),
            false,
            uv,
            BrepTrimType::Boundary,
            SurfaceIso::NotIso,
            [0.; 2],
        )?;
        let face = BrepFace::try_new(
            face.surface.clone(),
            false,
            vec![BrepLoop::try_new(BrepLoopType::Outer, vec![trim])?],
        )?;
        let bounds = shape.as_ref().tight_bounds(tolerance)?;
        let scale = bounds
            .min()
            .to_array()
            .into_iter()
            .chain(bounds.max().to_array())
            .map(Real::abs)
            .fold(1., Real::max);
        let padding = tolerance.absolute().max(4. * tolerance.relative() * scale);
        Ok(Self {
            geometry,
            shape,
            face,
            start,
            bounds,
            padding,
        })
    }
    fn contains(
        &self,
        other: &Self,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Result<bool, GeometryError> {
        if matches!(self.shape, Curve3::Circle(_)) {
            self.shape
                .offset_region_contains(
                    other.geometry.evaluate(*other.geometry.domain().start())?,
                    normal,
                    tolerance,
                )?
                .ok_or_else(|| invalid("inset region containment is unresolved"))
        } else {
            self.face
                .contains_parameters(other.start.x(), other.start.y(), tolerance)
        }
    }
    fn disjoint_boundary(
        &self,
        other: &Self,
        tolerance: Tolerance,
        operations: Option<CurveOffsetOperations>,
    ) -> Result<(), GeometryError> {
        if !self.bounds_overlap(other) {
            return Ok(());
        }
        let contact = match self
            .shape
            .offset_region_boundary_relation(&other.shape, tolerance)?
        {
            Some(contact) => contact,
            None => {
                let events = if let Some(ops) = operations {
                    (ops.intersect)(&self.geometry, &other.geometry, tolerance)?
                } else {
                    self.geometry
                        .intersection_events_with_curve(&other.geometry, tolerance)?
                };
                !events.is_empty()
            }
        };
        if contact {
            return Err(invalid("inset boundaries intersect or touch"));
        }
        Ok(())
    }
    fn bounds_overlap(&self, other: &Self) -> bool {
        let padding = self.padding.max(other.padding);
        (0..3).all(|k| {
            self.bounds.min().to_array()[k] <= other.bounds.max().to_array()[k] + padding
                && other.bounds.min().to_array()[k] <= self.bounds.max().to_array()[k] + padding
        })
    }
}

fn check_region(
    loops: &[Region],
    normal: UnitVector3,
    tolerance: Tolerance,
    operations: Option<CurveOffsetOperations>,
) -> Result<(), GeometryError> {
    for hole in &loops[1..] {
        loops[0].disjoint_boundary(hole, tolerance, operations)?;
        if !loops[0].contains(hole, normal, tolerance)? {
            return Err(invalid("inset hole lies outside its outer loop"));
        }
    }
    if loops.len() < 3 {
        return Ok(());
    }
    // Sweep along the widest world-space extent, also handling vertical and
    // tilted planes. Disjoint bounds exclude both crossings and containment.
    let extent = |axis| {
        let lo = loops[1..]
            .iter()
            .map(|r| r.bounds.min().to_array()[axis])
            .fold(Real::INFINITY, Real::min);
        let hi = loops[1..]
            .iter()
            .map(|r| r.bounds.max().to_array()[axis])
            .fold(Real::NEG_INFINITY, Real::max);
        hi - lo
    };
    let axis = (0..3)
        .max_by(|&a, &b| extent(a).total_cmp(&extent(b)))
        .unwrap();
    let padding = loops.iter().map(|r| r.padding).fold(0., Real::max);
    let mut order = (1..loops.len()).collect::<Vec<_>>();
    order.sort_unstable_by(|&a, &b| {
        loops[a].bounds.min().to_array()[axis].total_cmp(&loops[b].bounds.min().to_array()[axis])
    });
    for (position, &i) in order.iter().enumerate() {
        for &j in &order[position + 1..] {
            if loops[j].bounds.min().to_array()[axis]
                > loops[i].bounds.max().to_array()[axis] + padding
            {
                break;
            }
            if !loops[i].bounds_overlap(&loops[j]) {
                continue;
            }
            loops[i].disjoint_boundary(&loops[j], tolerance, operations)?;
            if loops[i].contains(&loops[j], normal, tolerance)?
                || loops[j].contains(&loops[i], normal, tolerance)?
            {
                return Err(invalid("inset holes overlap or nest"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate(
    brep: &Brep,
    face: &BrepFace,
    caps: &[Vec<BrepTrim>],
    normal: UnitVector3,
    tolerance: Tolerance,
    operations: Option<CurveOffsetOperations>,
) -> Result<(), GeometryError> {
    let old = face
        .loops
        .iter()
        .enumerate()
        .map(|(i, l)| Region::new(brep, face, &l.trims, i == 0, normal, tolerance, operations))
        .collect::<Result<Vec<_>, _>>()?;
    let new = caps
        .iter()
        .enumerate()
        .map(|(i, l)| Region::new(brep, face, l, i == 0, normal, tolerance, operations))
        .collect::<Result<Vec<_>, _>>()?;
    check_region(&old, normal, tolerance, operations)?;
    check_region(&new, normal, tolerance, operations)?;
    for (i, (a, b)) in old.iter().zip(&new).enumerate() {
        a.disjoint_boundary(b, tolerance, operations)?;
        let nested = if i == 0 {
            a.contains(b, normal, tolerance)?
        } else {
            b.contains(a, normal, tolerance)?
        };
        if !nested {
            return Err(invalid("inset offset changes the boundary topology"));
        }
    }
    Ok(())
}
