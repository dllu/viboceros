//! A planar text annotation with an arrow at the first path vertex.
use std::sync::Arc;

use crate::{
    AffineTransform3, AnnotationStyle, Frame3, GeometryError, Point3, Polyline3, Tolerance,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Leader {
    plane: Frame3,
    path: Polyline3,
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
        Ok(Self {
            plane,
            path,
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
