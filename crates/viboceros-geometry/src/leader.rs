//! A planar text annotation with an arrow at the first path vertex.
use std::sync::Arc;

use viboceros_opennurbs::{LeaderAppearance, LeaderCurveType, LeaderTextAngle};

use crate::{
    AffineTransform3, AnnotationStyle, BoundingBox3, CurveRef, Frame3, GeometryError, NurbsCurve,
    Point3, Polyline3, Tolerance, UnitVector3,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Leader {
    plane: Frame3,
    path: Polyline3,
    spline: Option<Arc<NurbsCurve>>,
    arrow: Arc<[Point3]>,
    text: Arc<str>,
    rich_text: Arc<str>,
    style: AnnotationStyle,
}

impl Leader {
    pub fn try_new(
        plane: Frame3,
        points: Vec<Point3>,
        text: impl Into<Arc<str>>,
        style: AnnotationStyle,
    ) -> Result<Self, GeometryError> {
        let text = text.into();
        Self::try_styled(plane, points, Arc::clone(&text), text, style)
    }

    pub fn try_styled(
        plane: Frame3,
        points: Vec<Point3>,
        text: impl Into<Arc<str>>,
        rich_text: impl Into<Arc<str>>,
        style: AnnotationStyle,
    ) -> Result<Self, GeometryError> {
        let text = text.into();
        let rich_text = rich_text.into();
        if text.contains('\0') || rich_text.contains('\0') {
            return Err(GeometryError::InvalidLeader);
        }
        let vertices = points
            .into_iter()
            .map(|point| {
                let [x, y, _] = plane.coordinates_of(point)?;
                plane.point_at([x, y, 0.])
            })
            .collect::<Result<Vec<_>, _>>()?;
        let path = Polyline3::try_new(vertices, Tolerance::NUMERICAL_VALIDATION)?;
        let appearance = style.leader_appearance();
        // A path can be valid even if its styled display geometry overflows.
        let spline = if appearance.curve_type() == LeaderCurveType::Spline {
            leader_spline(&path).ok().flatten().map(Arc::new)
        } else {
            None
        };
        let arrow = arrow_outline(plane, &path, appearance).unwrap_or_default();
        Ok(Self {
            plane,
            path,
            spline,
            arrow,
            text,
            rich_text,
            style,
        })
    }

    pub fn plane(&self) -> Frame3 {
        self.plane
    }

    pub fn path(&self) -> &Polyline3 {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn rich_text(&self) -> &str {
        &self.rich_text
    }

    pub fn style(&self) -> &AnnotationStyle {
        &self.style
    }

    pub fn bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(
            self.path
                .vertices()
                .iter()
                .copied()
                .chain(self.arrow.iter().copied())
                .chain(self.landing().into_iter().flatten()),
        )
        .expect("validated leader has a nonempty path")
    }

    /// The visible curve is distinct from the editable control polygon.
    pub fn body(&self) -> Option<CurveRef<'_>> {
        match self.style.leader_appearance().curve_type() {
            LeaderCurveType::None => None,
            LeaderCurveType::Polyline => Some(CurveRef::Polyline(&self.path)),
            LeaderCurveType::Spline => self.spline.as_deref().map(CurveRef::NurbsCurve),
        }
    }

    pub fn arrow_triangles(&self) -> impl Iterator<Item = [Point3; 3]> + '_ {
        // All native built-in polygons, including Open Arrow, are
        // triangulable with a fan from their first vertex.
        self.arrow
            .windows(2)
            .skip(1)
            .map(|edge| [self.arrow[0], edge[0], edge[1]])
    }

    pub fn text_attachment(&self) -> Option<(Point3, UnitVector3)> {
        let (tail, direction) = self.tail()?;
        let appearance = self.style.leader_appearance();
        let distance = appearance.landing_length.max(0.0) + appearance.text_gap;
        let point = tail
            .translated(direction.as_vector().scaled(distance).ok()?)
            .ok()?;
        Some((point, direction))
    }

    pub fn landing(&self) -> Option<[Point3; 2]> {
        let length = self.style.leader_appearance().landing_length;
        if length <= 0.0 {
            return None;
        }
        let (tail, direction) = self.tail()?;
        let end = tail
            .translated(direction.as_vector().scaled(length).ok()?)
            .ok()?;
        Some([tail, end])
    }

    fn tail(&self) -> Option<(Point3, UnitVector3)> {
        let [.., previous, tail] = self.path.vertices() else {
            return None;
        };
        let segment = previous.direction_to(*tail).ok()?;
        let direction = match self.style.leader_appearance().text_angle() {
            LeaderTextAngle::Horizontal => {
                let x = self.plane.x_axis();
                if x.as_vector().dot(segment.as_vector()).ok()? < 0.0 {
                    x.as_vector().scaled(-1.0).ok()?.normalized_nonzero().ok()?
                } else {
                    x
                }
            }
            LeaderTextAngle::Aligned => segment,
            LeaderTextAngle::Rotated(angle) => self
                .plane
                .vector_at([angle.cos(), angle.sin(), 0.0])
                .ok()?
                .normalized_nonzero()
                .ok()?,
        };
        Some((*tail, direction))
    }

    pub fn transformed(&self, map: AffineTransform3) -> Result<Self, GeometryError> {
        let plane = Frame3::try_from_directions(
            map.transform_point(self.plane.origin())?,
            map.transform_vector(self.plane.x_axis().as_vector())?,
            map.transform_vector(self.plane.y_axis().as_vector())?,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let vertices = self
            .path
            .vertices()
            .iter()
            .map(|&point| map.transform_point(point))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_styled(
            plane,
            vertices,
            Arc::clone(&self.text),
            Arc::clone(&self.rich_text),
            self.style.clone(),
        )
    }
}

fn leader_spline(path: &Polyline3) -> Result<Option<NurbsCurve>, GeometryError> {
    // Match ON_Leader::Curve exactly. The leader vertices are the control
    // points and the first chord lengths determine the clamped knot intervals.
    let points = path.vertices();
    let degree = (points.len() - 1).min(3);
    let mut knots = vec![0.0; degree + 1];
    let mut length = 0.0;
    for chord in points.windows(2).take(points.len() - degree) {
        length += chord[0].distance_to(chord[1])?;
        knots.push(length);
    }
    if length <= f64::EPSILON.sqrt() {
        return Ok(None);
    }
    knots.resize(points.len() + degree + 1, length);
    NurbsCurve::try_new(degree, points.to_vec(), knots).map(Some)
}

fn arrow_outline(
    plane: Frame3,
    path: &Polyline3,
    appearance: LeaderAppearance,
) -> Result<Arc<[Point3]>, GeometryError> {
    let outline = appearance.arrow_outline();
    if appearance.arrow_size <= 0.0 || outline.is_empty() {
        return Ok(Arc::from([]));
    }
    let tip = path.vertices()[0];
    let outward = path.vertices()[1].direction_to(tip)?.as_vector();
    let across = plane.z_axis().as_vector().cross(outward)?;
    outline
        .iter()
        .map(|&[x, y]| {
            tip.translated(outward.scaled(x * appearance.arrow_size)?)?
                .translated(across.scaled(y * appearance.arrow_size)?)
        })
        .collect()
}
