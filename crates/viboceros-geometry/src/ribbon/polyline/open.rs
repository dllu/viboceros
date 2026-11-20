//! Offset walks for two inward corners with crossing or coincident clips.
use super::*;
use crate::{CurveOffsetCornerStyle, curve_offset::OffsetPolylineLayout, parameter::map_parameter};

#[derive(Clone, Copy)]
struct Cut {
    fraction: Real,
    point: Point3,
}

struct Rail {
    source: LineSegment,
    offset: LineSegment,
}

impl Rail {
    fn at(&self, fraction: Real) -> Result<Cut, GeometryError> {
        Ok(Cut {
            fraction,
            point: self.offset.point_at(fraction)?,
        })
    }

    fn project(&self, point: Point3, tolerance: Tolerance) -> Result<Cut, GeometryError> {
        let delta = self.offset.start().vector_to(point)?;
        let direction = self.offset.direction(tolerance)?;
        let fraction = delta.dot(direction.as_vector())? / self.offset.length()?;
        Ok(Cut { fraction, point })
    }

    fn source_part(
        &self,
        [a, b]: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        let domain = self.source.domain();
        let start = map_parameter(a, 0. ..=1., domain.clone())?;
        let end = map_parameter(b, 0. ..=1., domain)?;
        LineSegment::try_new(
            self.source.point_at(a)?,
            self.source.point_at(b)?,
            tolerance,
        )?
        .try_reparameterized(start..=end)?
        .to_nurbs()
    }
}

struct Walk {
    patches: Vec<Patch>,
    width: Real,
    tolerance: Tolerance,
    sharp: bool,
}

impl Walk {
    fn push<E: From<GeometryError>>(
        &mut self,
        rail: &Rail,
        [a, b]: [Cut; 2],
        anchors: [bool; 2],
        ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
    ) -> Result<(), E> {
        let [lo, hi] = [a.fraction, b.fraction];
        if !self.sharp && anchors[0] {
            self.cap(rail, [0., lo], a.point, ruled)?;
        }
        let start = if self.sharp && anchors[0] { 0. } else { lo };
        let end = if self.sharp && anchors[1] { 1. } else { hi };
        let source = rail.source_part([start, end], self.tolerance)?;
        let offset = LineSegment::try_new(a.point, b.point, self.tolerance)?
            .try_reparameterized(source.domain())?
            .to_nurbs()?;
        self.patches.push(patch(
            &source,
            &offset,
            self.width,
            self.tolerance,
            [None; 4],
            ruled,
        )?);
        if !self.sharp && anchors[1] {
            self.cap(rail, [hi, 1.], b.point, ruled)?;
        }
        Ok(())
    }

    fn cap<E: From<GeometryError>>(
        &mut self,
        rail: &Rail,
        range: [Real; 2],
        point: Point3,
        ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
    ) -> Result<(), E> {
        if range[0] < range[1] {
            let source = rail.source_part(range, self.tolerance)?;
            self.patches.push(patch(
                &source,
                &constant_with_basis(point, &source)?,
                self.width,
                self.tolerance,
                [None; 4],
                ruled,
            )?);
        }
        Ok(())
    }
}

pub(super) fn build<E: From<GeometryError>>(
    source: &Polyline3,
    layout: &OffsetPolylineLayout,
    distance: Real,
    corner: RibbonCornerStyle,
    side_point: Option<Point3>,
    tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Option<Brep>, E> {
    if source.segment_count() != 3
        || source.is_closed()
        || !layout.inside_corners[1]
        || !layout.inside_corners[2]
    {
        return Ok(None);
    }
    let middle = source.segments().nth(1).unwrap();
    let first = layout.corners[1].0;
    let second = layout.corners[2].0;
    let coincident =
        crate::curve_points_coincident(first, second) && first.is_near(second, tolerance);
    if !coincident && layout.segment_advances(1, middle)? {
        return Ok(None);
    }
    let rails = raw_rails(source, layout.normal, distance, tolerance)?;
    let [a, b, c] = rails.as_slice() else {
        unreachable!()
    };
    let ac = a.project(first, tolerance)?;
    let bb = b.project(first, tolerance)?;
    let bc = b.project(second, tolerance)?;
    let cc = c.project(second, tolerance)?;
    let mut walk = Walk {
        patches: Vec::new(),
        width: distance.abs(),
        tolerance,
        sharp: corner == RibbonCornerStyle::Sharp,
    };
    let anchors = [false; 2];
    let into_corner = [false, true];
    let out_of_corner = [true, false];
    let interior = |cut: Cut| cut.fraction > 0. && cut.fraction < 1.;
    let mut coincident_offsets = None;
    let opposite_ends_touch = [
        (first, b.offset.end()),
        (second, b.offset.start()),
        (a.offset.end(), c.source.start()),
        (c.offset.start(), a.source.end()),
    ]
    .into_iter()
    .all(|(p, q)| crate::curve_points_coincident(p, q) && p.is_near(q, tolerance));
    let close = if walk.sharp && opposite_ends_touch && interior(ac) && interior(cc) {
        // At exactly the separation of the outer segments, their offsets
        // land on the opposite sources. The middle rail drops out.
        walk.push(a, [a.at(0.)?, a.at(1.)?], anchors, ruled)?;
        walk.push(c, [cc, c.at(1.)?], anchors, ruled)?;
        false
    } else if [ac, bb, bc, cc].into_iter().all(interior) {
        if coincident {
            let Some(pivot) = overlap_midpoint(a.offset, c.offset, tolerance)? else {
                return Ok(None);
            };
            if !walk.sharp {
                return Ok(None);
            }
            let pivot_a = a.project(pivot, tolerance)?;
            let pivot_c = c.project(pivot, tolerance)?;
            let pick = side_point.unwrap_or(pivot);
            if a.project(pick, tolerance)?.fraction < pivot_a.fraction {
                walk.push(a, [a.at(0.)?, pivot_a], anchors, ruled)?;
                walk.cap(a, [pivot_a.fraction, 1.], pivot, ruled)?;
                walk.cap(b, [0., 1.], pivot, ruled)?;
                walk.cap(c, [0., pivot_c.fraction], pivot, ruled)?;
                walk.push(c, [pivot_c, c.at(1.)?], anchors, ruled)?;
            } else {
                let first = walk.patches.len();
                walk.push(a, [ac, a.at(1.)?], anchors, ruled)?;
                walk.push(b, [b.at(0.)?, bc], into_corner, ruled)?;
                walk.push(c, [cc, pivot_c], out_of_corner, ruled)?;
                let second = walk.patches.len();
                walk.push(a, [pivot_a, ac], into_corner, ruled)?;
                // The two cuts of the first rail meet at one offset vertex
                // when the middle interval between the cuts vanishes.
                coincident_offsets = Some([(first, 0), (second, 1)]);
                walk.push(b, [bb, b.at(1.)?], out_of_corner, ruled)?;
                walk.push(c, [c.at(0.)?, cc], anchors, ruled)?;
            }
            true
        } else {
            // Each inside intersection exchanges the outgoing fragments.
            // When their order has reversed on the middle rail, the walk
            // visits both ends of it and then joins the last rail again.
            walk.push(a, [a.at(0.)?, ac], into_corner, ruled)?;
            walk.push(b, [bb, b.at(1.)?], out_of_corner, ruled)?;
            walk.push(c, [c.at(0.)?, cc], anchors, ruled)?;
            walk.push(b, [bc, bb], anchors, ruled)?;
            walk.push(a, [ac, a.at(1.)?], anchors, ruled)?;
            walk.push(b, [b.at(0.)?, bc], into_corner, ruled)?;
            walk.push(c, [cc, c.at(1.)?], out_of_corner, ruled)?;
            false
        }
    } else {
        // A crossing beyond the middle segment cannot clip that end.
        // A crossing beyond a short outer segment needs a different end fit.
        if bb.fraction == 1.
            || bc.fraction == 0.
            || (!interior(ac) && interior(bb))
            || (!interior(cc) && interior(bc))
        {
            return Ok(None);
        }
        let valid_start = interior(ac) && interior(bb);
        let valid_end = interior(bc) && interior(cc);
        walk.push(
            a,
            [a.at(0.)?, if valid_start { ac } else { a.at(1.)? }],
            [false, valid_start],
            ruled,
        )?;
        walk.push(
            b,
            [
                if valid_start { bb } else { b.at(0.)? },
                if valid_end { bc } else { b.at(1.)? },
            ],
            [valid_start, valid_end],
            ruled,
        )?;
        walk.push(
            c,
            [if valid_end { cc } else { c.at(0.)? }, c.at(1.)?],
            [valid_end, false],
            ruled,
        )?;
        false
    };
    Ok(Some(assembly::sew_walk(
        walk.patches,
        close,
        coincident_offsets,
        tolerance,
    )?))
}

fn raw_rails(
    source: &Polyline3,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Vec<Rail>, GeometryError> {
    let raw = polyline_offset_layout_with_trim(
        source,
        distance,
        normal,
        tolerance,
        CurveOffsetCornerStyle::Sharp,
        false,
    )?;
    source
        .segments()
        .enumerate()
        .map(|(i, source)| {
            let [a, b] = raw.segment_points(i);
            Ok(Rail {
                source,
                offset: LineSegment::try_new(a, b, tolerance)?,
            })
        })
        .collect()
}

pub(super) fn short_corner<E: From<GeometryError>>(
    source: &Polyline3,
    layout: &OffsetPolylineLayout,
    distance: Real,
    both_sides: bool,
    inside_first: bool,
    tolerance: Tolerance,
    ruled: &mut impl FnMut(&NurbsCurve, &NurbsCurve) -> Result<NurbsSurface, E>,
) -> Result<Option<Brep>, E> {
    let mut blocked = false;
    for (i, segment) in source.segments().enumerate() {
        blocked |= !layout.segment_advances(i, segment)?;
    }
    if !blocked || !layout.inside_corners[1] {
        return Ok(None);
    }
    let rails = raw_rails(source, layout.normal, distance, tolerance)?;
    let [a, b] = rails.as_slice() else {
        unreachable!()
    };
    if both_sides && inside_first {
        let cosine = a
            .source
            .direction(tolerance)?
            .as_vector()
            .dot(b.source.direction(tolerance)?.as_vector())?
            .abs();
        if cosine > 0. && cosine <= 64. * Real::EPSILON {
            // A rounding-sized tilt can make Rhino retain a different
            // component when the ruling almost coincides with the source.
            return Ok(None);
        }
    }
    let q = layout.corners[1].0;
    let (u, v) = (
        a.project(q, tolerance)?.fraction,
        b.project(q, tolerance)?.fraction,
    );
    let radius = distance.abs();
    let mut walk = Walk {
        patches: Vec::new(),
        width: radius,
        tolerance,
        sharp: true,
    };
    if u < 0. && v > 0. && v < 1. {
        let Some(cap) = endpoint_cut(b.offset, a.source.start(), radius, true, tolerance)? else {
            return Ok(None);
        };
        let cap = b.at(cap)?;
        if inside_first {
            walk.push(a, [a.at(0.)?, a.at(1.)?], [false; 2], ruled)?;
            walk.push(b, [b.at(0.)?, cap], [false; 2], ruled)?;
        } else {
            walk.push(b, [cap, b.at(1.)?], [false; 2], ruled)?;
        }
    } else if v > 1. && u > 0. && u < 1. {
        let Some(cap) = endpoint_cut(a.offset, b.source.end(), radius, false, tolerance)? else {
            return Ok(None);
        };
        let cap = a.at(cap)?;
        if inside_first {
            walk.push(a, [cap, a.at(1.)?], [false; 2], ruled)?;
            walk.push(b, [b.at(0.)?, b.at(1.)?], [false; 2], ruled)?;
        } else {
            walk.push(a, [a.at(0.)?, cap], [false; 2], ruled)?;
        }
    } else {
        return Ok(None);
    }
    Ok(Some(assembly::sew_walk(
        walk.patches,
        false,
        None,
        tolerance,
    )?))
}

// The short rail never reaches its miter. Its far source endpoint supplies
// the circle that limits the other translated rail.
fn endpoint_cut(
    rail: LineSegment,
    center: Point3,
    radius: Real,
    after_center: bool,
    tolerance: Tolerance,
) -> Result<Option<Real>, GeometryError> {
    let direction = rail.direction(tolerance)?.as_vector();
    let delta = rail.start().vector_to(center)?;
    let separation = delta.cross(direction)?.length()?;
    if separation >= radius {
        return Ok(None);
    }
    let height = (radius - separation).sqrt() * (radius + separation).sqrt();
    let signed = if after_center { height } else { -height };
    let fraction = (delta.dot(direction)? + signed) / rail.length()?;
    Ok((fraction > 0. && fraction < 1.).then_some(fraction))
}

fn overlap_midpoint(
    a: LineSegment,
    b: LineSegment,
    tolerance: Tolerance,
) -> Result<Option<Point3>, GeometryError> {
    let direction = a.direction(tolerance)?;
    let other = b.direction(tolerance)?;
    if direction.as_vector().dot(other.as_vector())? >= 0.
        || direction.as_vector().cross(other.as_vector())?.length()? > tolerance.angular()
    {
        return Ok(None);
    }
    let station = |p| -> Result<Real, GeometryError> {
        let delta = a.start().vector_to(p)?;
        if delta.cross(direction.as_vector())?.length()? > tolerance.absolute() {
            return Ok(Real::NAN);
        }
        Ok(delta.dot(direction.as_vector())?)
    };
    let [far, near] = [station(b.start())?, station(b.end())?];
    if !far.is_finite() || !near.is_finite() {
        return Ok(None);
    }
    let length = a.length()?;
    let (lo, hi) = (0.0_f64.max(near), length.min(far));
    if lo >= hi {
        return Ok(None);
    }
    Ok(Some(a.point_at((lo + hi) / (2. * length))?))
}
