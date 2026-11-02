//! 3DM camera metadata, cached frames, validation and native bridge conversion.
use super::*;

pub(super) fn encode_view(view: &ThreeDmNamedView, name: *const c_char) -> ffi::ViboNamedView {
    ffi::ViboNamedView {
        name,
        projection: view.projection as u8,
        has_camera_axes: u8::from(view.camera_axes.is_some()),
        camera_axes: view
            .camera_axes
            .map_or([[0.; 3]; 3], |axes| axes.map(|v| v.as_vector().to_array())),
        has_target: u8::from(view.target.is_some()),
        camera_location: view.camera_location.to_array(),
        camera_direction: view.camera_direction.to_array(),
        camera_up: view.camera_up.to_array(),
        target: view.target.map_or([0.0; 3], Point3::to_array),
        cplane_origin: view.construction_plane.origin().to_array(),
        cplane_x: view.construction_plane.x_axis().as_vector().to_array(),
        cplane_y: view.construction_plane.y_axis().as_vector().to_array(),
        frustum: view.frustum,
        screen_port: view.screen_port,
    }
}
pub(super) fn scale_view(view: &mut ThreeDmNamedView, scale: f64) -> Result<(), ThreeDmError> {
    if scale == 1.0 {
        return Ok(());
    }
    let scaled_point = |point: Point3| -> Result<Point3, GeometryError> {
        Point3::try_from(point.to_array().map(|coordinate| coordinate * scale))
    };
    view.camera_location = scaled_point(view.camera_location)?;
    view.target = view.target.map(scaled_point).transpose()?;
    view.construction_plane = Frame3::try_from_directions(
        scaled_point(view.construction_plane.origin())?,
        view.construction_plane.x_axis().as_vector(),
        view.construction_plane.y_axis().as_vector(),
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    view.frustum = view.frustum.map(|coordinate| coordinate * scale);
    Ok(())
}

pub(super) fn decode_view(raw: &ffi::ViboNamedView) -> Result<ThreeDmNamedView, ThreeDmError> {
    let projection = match raw.projection {
        1 => ThreeDmProjection::Parallel,
        2 => ThreeDmProjection::Perspective,
        3 => ThreeDmProjection::TwoPointPerspective,
        _ => return Err(ThreeDmError::MalformedBridge("invalid view projection")),
    };
    let view = ThreeDmNamedView {
        camera_axes: if raw.has_camera_axes != 0 {
            let unit = |v| Vector3::try_from(v)?.normalized_nonzero();
            Some([
                unit(raw.camera_axes[0])?,
                unit(raw.camera_axes[1])?,
                unit(raw.camera_axes[2])?,
            ])
        } else {
            None
        },
        name: c_text(raw.name)?,
        projection,
        camera_location: Point3::try_from(raw.camera_location)?,
        camera_direction: Vector3::try_from(raw.camera_direction)?,
        camera_up: Vector3::try_from(raw.camera_up)?,
        target: (raw.has_target != 0)
            .then(|| Point3::try_from(raw.target))
            .transpose()?,
        construction_plane: Frame3::try_from_directions(
            Point3::try_from(raw.cplane_origin)?,
            Vector3::try_from(raw.cplane_x)?,
            Vector3::try_from(raw.cplane_y)?,
            Tolerance::NUMERICAL_VALIDATION,
        )?,
        frustum: raw.frustum,
        screen_port: raw.screen_port,
    };
    view.camera_frame()?;
    Ok(view)
}

pub(super) fn validate_view_camera(
    view: &ThreeDmNamedView,
    description: &str,
) -> Result<(), ThreeDmError> {
    view.camera_frame().map_err(|error| {
        ThreeDmError::InvalidModel(format!("{description} has an invalid camera: {error}"))
    })?;
    let [left, right, bottom, top, near, far] = view.frustum;
    if !view.frustum.iter().all(|value| value.is_finite())
        || left >= right
        || bottom >= top
        || near <= 0.0
        || near >= far
        || view.screen_port[0] == view.screen_port[1]
        || view.screen_port[2] == view.screen_port[3]
    {
        return Err(ThreeDmError::InvalidModel(format!(
            "{description} has an invalid frustum"
        )));
    }
    Ok(())
}

impl ThreeDmNamedView {
    /// Optical camera frame. OpenNURBS stores this separately from the raw
    /// direction/up vectors, which may be skew or constrained by camera locks.
    pub fn camera_frame(&self) -> Result<Frame3, GeometryError> {
        let direction = self.camera_direction.normalized_nonzero()?.as_vector();
        let up = self.camera_up.normalized_nonzero()?.as_vector();
        let right = direction.cross(up)?.normalized_nonzero()?.as_vector();
        if let Some([x, y, z]) = self.camera_axes {
            let (x, y, z) = (x.as_vector(), y.as_vector(), z.as_vector());
            if x.dot(y)?.abs() > 1e-8
                || y.dot(z)?.abs() > 1e-8
                || z.dot(x)?.abs() > 1e-8
                || x.cross(y)?.dot(z)? < 1. - 1e-8
            {
                return Err(GeometryError::Degenerate {
                    context: "cached camera frame",
                });
            }
            return Frame3::try_from_directions(
                self.camera_location,
                x,
                y,
                Tolerance::NUMERICAL_VALIDATION,
            );
        }
        Frame3::try_from_directions(
            self.camera_location,
            right,
            right.cross(direction)?,
            Tolerance::NUMERICAL_VALIDATION,
        )
    }
}
