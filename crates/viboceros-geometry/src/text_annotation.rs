//! Model-sized text retained as an annotation rather than converted to curves.
use crate::{
    AffineTransform3, AnnotationStyle, BoundingBox3, Frame3, GeometryError, Point3, Tolerance,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAnnotationLayout {
    pub rotation: f64,
    pub wrapping_width: Option<f64>,
    /// Ink bounds in document units before rotating or placing the text plane.
    pub ink_bounds: [[f64; 2]; 2],
    /// Extra height factor relative to the archived style after model transforms.
    pub height_factor: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextAnnotation {
    plane: Frame3,
    text: Arc<str>,
    rich_text: Arc<str>,
    style: AnnotationStyle,
    layout: TextAnnotationLayout,
    bounds: BoundingBox3,
}

impl TextAnnotation {
    pub fn try_new(
        plane: Frame3,
        text: impl Into<Arc<str>>,
        rich_text: impl Into<Arc<str>>,
        style: AnnotationStyle,
        layout: TextAnnotationLayout,
    ) -> Result<Self, GeometryError> {
        let text = text.into();
        let rich_text = rich_text.into();
        let [min, max] = layout.ink_bounds;
        if text.contains('\0')
            || rich_text.contains('\0')
            || !layout.rotation.is_finite()
            || !layout.height_factor.is_finite()
            || layout.height_factor <= 0.0
            || layout
                .wrapping_width
                .is_some_and(|width| !width.is_finite() || width <= 0.0)
            || !(0..2).all(|i| min[i].is_finite() && max[i].is_finite() && min[i] <= max[i])
        {
            return Err(GeometryError::InvalidTextAnnotation);
        }
        let corners = quad(plane, layout)?;
        Ok(Self {
            plane,
            text,
            rich_text,
            style,
            layout,
            bounds: BoundingBox3::from_points(corners)?,
        })
    }

    pub fn plane(&self) -> Frame3 {
        self.plane
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
    pub fn layout(&self) -> TextAnnotationLayout {
        self.layout
    }
    pub fn bounds(&self) -> BoundingBox3 {
        self.bounds
    }

    /// Ink corners in reading order, starting at the top left of the text.
    pub fn quad(&self) -> [Point3; 4] {
        quad(self.plane, self.layout).expect("validated annotation quad")
    }

    pub fn outline(&self) -> [Point3; 5] {
        let [a, b, c, d] = self.quad();
        [a, b, c, d, a]
    }

    pub fn transformed(&self, map: AffineTransform3) -> Result<Self, GeometryError> {
        let x = map.transform_vector(self.plane.x_axis().as_vector())?;
        let y = map.transform_vector(self.plane.y_axis().as_vector())?;
        let scale = y.length()?;
        let plane = Frame3::try_from_directions(
            map.transform_point(self.plane.origin())?,
            x,
            y,
            Tolerance::NUMERICAL_VALIDATION,
        )?;
        let (sin, cos) = self.layout.rotation.sin_cos();
        let width_scale = map
            .transform_vector(self.plane.vector_at([cos, sin, 0.0])?)?
            .length()?;
        Self::try_new(
            plane,
            Arc::clone(&self.text),
            Arc::clone(&self.rich_text),
            self.style.clone(),
            TextAnnotationLayout {
                ink_bounds: self
                    .layout
                    .ink_bounds
                    .map(|corner| corner.map(|p| p * scale)),
                height_factor: self.layout.height_factor * scale,
                wrapping_width: self.layout.wrapping_width.map(|width| width * width_scale),
                ..self.layout
            },
        )
    }
}

fn quad(plane: Frame3, layout: TextAnnotationLayout) -> Result<[Point3; 4], GeometryError> {
    let [[left, bottom], [right, top]] = layout.ink_bounds;
    let (sin, cos) = layout.rotation.sin_cos();
    let point = |x, y| plane.point_at([x * cos - y * sin, x * sin + y * cos, 0.0]);
    Ok([
        point(left, top)?,
        point(right, top)?,
        point(right, bottom)?,
        point(left, bottom)?,
    ])
}
