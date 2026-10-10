use std::{
    ffi::{CStr, c_char, c_int},
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};
use viboceros_geometry::{
    GeometryError, NurbsCurve, Point3, Tolerance, TriangleMesh, WeightedPoint3,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SMLib: {0}")]
    Kernel(String),
    #[error("SMLib returned inconsistent data")]
    InvalidOutput,
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

#[repr(C)]
struct NativeSolid {
    _private: [u8; 0],
}
#[repr(C)]
struct NativeCurve {
    _private: [u8; 0],
}
#[repr(C)]
struct NativeMesh {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_solid_primitive(
        kind: c_int,
        origin: *const f64,
        size: *const f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_boolean(
        a: *const NativeSolid,
        b: *const NativeSolid,
        operation: c_int,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_properties(
        solid: *const NativeSolid,
        accuracy: f64,
        volume: *mut f64,
        bounds: *mut f64,
        manifold: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_free(solid: *mut NativeSolid);
    fn vb_solid_mesh(
        solid: *const NativeSolid,
        quality: *const f64,
        out: *mut *mut NativeMesh,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_mesh_sizes(mesh: *const NativeMesh, vertices: *mut usize, triangles: *mut usize)
    -> c_int;
    fn vb_mesh_copy(
        mesh: *const NativeMesh,
        vertices: *mut f64,
        vertex_count: usize,
        triangles: *mut u32,
        triangle_count: usize,
    ) -> c_int;
    fn vb_mesh_free(mesh: *mut NativeMesh);
    fn vb_curve_create(
        degree: usize,
        points: *const f64,
        point_count: usize,
        knots: *const f64,
        knot_count: usize,
        out: *mut *mut NativeCurve,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_evaluate(
        curve: *const NativeCurve,
        parameter: f64,
        point: *mut f64,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_curve_sizes(
        curve: *const NativeCurve,
        degree: *mut usize,
        points: *mut usize,
        knots: *mut usize,
    ) -> c_int;
    fn vb_curve_copy(
        curve: *const NativeCurve,
        points: *mut f64,
        point_count: usize,
        knots: *mut f64,
        knot_count: usize,
    ) -> c_int;
    fn vb_curve_free(curve: *mut NativeCurve);
}

fn call(function: impl FnOnce(*mut c_char, usize) -> c_int) -> Result<(), Error> {
    let mut error = [0 as c_char; 512];
    if function(error.as_mut_ptr(), error.len()) == 0 {
        return Ok(());
    }
    // Every error buffer starts zeroed; the C boundary always writes a terminated message.
    Err(Error::Kernel(
        unsafe { CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    ))
}

/// An exact native solid, owned independently of any document.
///
/// ```compile_fail
/// # use viboceros_smlib::Solid;
/// fn must_send<T: Send>() {}
/// must_send::<Solid>();
/// ```
pub struct Solid {
    handle: NonNull<NativeSolid>,
    _thread_local: PhantomData<Rc<()>>,
}

#[derive(Clone, Copy, Debug)]
pub enum BooleanOperation {
    Union,
    Intersection,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolidProperties {
    pub volume: f64,
    pub bounds: [Point3; 2],
    pub manifold: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Tessellation {
    pub chord_height: f64,
    pub curve_angle_degrees: f64,
    pub surface_angle_degrees: f64,
}
impl Default for Tessellation {
    fn default() -> Self {
        Self {
            chord_height: 0.005,
            curve_angle_degrees: 15.,
            surface_angle_degrees: 15.,
        }
    }
}

impl Solid {
    fn primitive(kind: c_int, origin: Point3, size: [f64; 3]) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        let origin = origin.to_array();
        // The input arrays remain live; native code returns a unique owned handle.
        call(|error, capacity| unsafe {
            vb_solid_primitive(
                kind,
                origin.as_ptr(),
                size.as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }
    pub fn box_solid(origin: Point3, dimensions: [f64; 3]) -> Result<Self, Error> {
        Self::primitive(0, origin, dimensions)
    }
    pub fn sphere(center: Point3, radius: f64) -> Result<Self, Error> {
        Self::primitive(1, center, [radius, 0., 0.])
    }
    pub fn cylinder(base: Point3, radius: f64, height: f64) -> Result<Self, Error> {
        Self::primitive(2, base, [radius, height, 0.])
    }

    /// Returns a new result, preserving both inputs on success and failure.
    pub fn boolean(&self, other: &Self, operation: BooleanOperation) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        let operation = match operation {
            BooleanOperation::Union => 0,
            BooleanOperation::Intersection => 1,
            BooleanOperation::Difference => 2,
        };
        // Both handles stay borrowed; the native wrapper copies before consuming operands.
        call(|error, capacity| unsafe {
            vb_solid_boolean(
                self.handle.as_ptr(),
                other.handle.as_ptr(),
                operation,
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }

    pub fn properties(&self, relative_accuracy: f64) -> Result<SolidProperties, Error> {
        let mut volume = 0.;
        let mut bounds = [0.; 6];
        let mut manifold = 0;
        call(|error, capacity| unsafe {
            vb_solid_properties(
                self.handle.as_ptr(),
                relative_accuracy,
                &mut volume,
                bounds.as_mut_ptr(),
                &mut manifold,
                error,
                capacity,
            )
        })?;
        Ok(SolidProperties {
            volume,
            bounds: [
                Point3::try_new(bounds[0], bounds[1], bounds[2])?,
                Point3::try_new(bounds[3], bounds[4], bounds[5])?,
            ],
            manifold: manifold != 0,
        })
    }

    /// Converts a complete triangular tessellation into validated Rust mesh data.
    pub fn tessellate(
        &self,
        quality: Tessellation,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, Error> {
        let quality = [
            quality.chord_height,
            quality.curve_angle_degrees,
            quality.surface_angle_degrees,
        ];
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_solid_mesh(
                self.handle.as_ptr(),
                quality.as_ptr(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        let mesh = OwnedMesh(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
        let mut vertex_count = 0;
        let mut triangle_count = 0;
        if unsafe { vb_mesh_sizes(mesh.0.as_ptr(), &mut vertex_count, &mut triangle_count) } != 0
            || vertex_count > 1_000_000
            || triangle_count > 1_000_000
        {
            return Err(Error::InvalidOutput);
        }
        let mut points = vec![0.; vertex_count * 3];
        let mut triangles = vec![0; triangle_count * 3];
        if unsafe {
            vb_mesh_copy(
                mesh.0.as_ptr(),
                points.as_mut_ptr(),
                vertex_count,
                triangles.as_mut_ptr(),
                triangle_count,
            )
        } != 0
        {
            return Err(Error::InvalidOutput);
        }
        let points = points
            .chunks_exact(3)
            .map(|p| Point3::try_new(p[0], p[1], p[2]))
            .collect::<Result<Vec<_>, _>>()?;
        let triangles = triangles
            .chunks_exact(3)
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        Ok(TriangleMesh::try_new(points, triangles, tolerance)?)
    }
}
impl Drop for Solid {
    fn drop(&mut self) {
        unsafe { vb_solid_free(self.handle.as_ptr()) };
    }
}
struct OwnedMesh(NonNull<NativeMesh>);
impl Drop for OwnedMesh {
    fn drop(&mut self) {
        unsafe { vb_mesh_free(self.0.as_ptr()) };
    }
}

/// Native clamped NURBS with positive weights, transferred without fitting.
pub struct KernelCurve {
    handle: NonNull<NativeCurve>,
    _thread_local: PhantomData<Rc<()>>,
}
impl KernelCurve {
    pub fn from_nurbs(curve: &NurbsCurve) -> Result<Self, Error> {
        let points = curve
            .control_points()
            .iter()
            .flat_map(|p| {
                let xyz = p.point().to_array();
                [xyz[0], xyz[1], xyz[2], p.weight()]
            })
            .collect::<Vec<_>>();
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_curve_create(
                curve.degree(),
                points.as_ptr(),
                curve.control_points().len(),
                curve.knots().as_ptr(),
                curve.knots().len(),
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }
    pub fn evaluate(&self, parameter: f64) -> Result<Point3, Error> {
        let mut point = [0.; 3];
        call(|error, capacity| unsafe {
            vb_curve_evaluate(
                self.handle.as_ptr(),
                parameter,
                point.as_mut_ptr(),
                error,
                capacity,
            )
        })?;
        Ok(Point3::try_new(point[0], point[1], point[2])?)
    }
    pub fn to_nurbs(&self) -> Result<NurbsCurve, Error> {
        let mut degree = 0;
        let mut point_count = 0;
        let mut knot_count = 0;
        if unsafe {
            vb_curve_sizes(
                self.handle.as_ptr(),
                &mut degree,
                &mut point_count,
                &mut knot_count,
            )
        } != 0
            || point_count > 1_000_000
            || degree > 64
            || knot_count > 1_000_065
        {
            return Err(Error::InvalidOutput);
        }
        let mut points = vec![0.; point_count * 4];
        let mut knots = vec![0.; knot_count];
        if unsafe {
            vb_curve_copy(
                self.handle.as_ptr(),
                points.as_mut_ptr(),
                point_count,
                knots.as_mut_ptr(),
                knot_count,
            )
        } != 0
        {
            return Err(Error::InvalidOutput);
        }
        let points = points
            .chunks_exact(4)
            .map(|p| WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2])?, p[3]))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(NurbsCurve::try_new_rational(degree, points, knots)?)
    }
}
impl Drop for KernelCurve {
    fn drop(&mut self) {
        unsafe { vb_curve_free(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_boundary_clears_failed_outputs_and_terminates_small_error_buffers() {
        let origin = [0.; 3];
        let size = [1.; 3];
        let mut output = NonNull::<NativeSolid>::dangling().as_ptr();
        let mut error = [42 as c_char; 1];
        let status = unsafe {
            vb_solid_primitive(
                99,
                origin.as_ptr(),
                size.as_ptr(),
                &mut output,
                error.as_mut_ptr(),
                error.len(),
            )
        };
        assert_ne!(status, 0);
        assert!(output.is_null());
        assert_eq!(error, [0]);
        let status = unsafe {
            vb_solid_primitive(
                0,
                std::ptr::null(),
                size.as_ptr(),
                &mut output,
                std::ptr::null_mut(),
                0,
            )
        };
        assert_ne!(status, 0);
        assert!(output.is_null());
    }
}
