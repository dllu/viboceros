//! Copy native topology and NURBS payloads into the validated Rust B-rep model.
use super::*;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    NurbsCurve2, NurbsSurface, Point2, SurfaceIso, WeightedPoint2,
};

#[repr(C)]
struct NativeBrep {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct NurbsData {
    degree: [usize; 2],
    count: [usize; 2],
    knot_count: [usize; 2],
    knots: [*const f64; 2],
    points: *const f64,
}
#[repr(C)]
struct VertexData {
    point: [f64; 3],
    tolerance: f64,
}
#[repr(C)]
struct EdgeData {
    vertices: [usize; 2],
    interval: [f64; 2],
    tolerance: f64,
    curve: NurbsData,
}
#[repr(C)]
struct TrimData {
    vertices: [usize; 2],
    edge: usize,
    reversed: c_int,
    curve: NurbsData,
}
#[repr(C)]
struct LoopData {
    first_trim: usize,
    trim_count: usize,
    inner: c_int,
}
#[repr(C)]
struct FaceData {
    surface: NurbsData,
    first_loop: usize,
    loop_count: usize,
    reversed: c_int,
}
#[repr(C)]
struct BrepView {
    vertex_count: usize,
    edge_count: usize,
    trim_count: usize,
    loop_count: usize,
    face_count: usize,
    vertices: *const VertexData,
    edges: *const EdgeData,
    trims: *const TrimData,
    loops: *const LoopData,
    faces: *const FaceData,
}

#[repr(C)]
struct NativeParts {
    _private: [u8; 0],
}
unsafe extern "C" {
    fn vb_solid_copy(
        solid: *const NativeSolid,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_boundary_contact(
        a: *const NativeSolid,
        b: *const NativeSolid,
        tolerance: f64,
        contact: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_parts(
        solid: *const NativeSolid,
        out: *mut *mut NativeParts,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_parts_count(parts: *const NativeParts) -> usize;
    fn vb_parts_take(parts: *mut NativeParts, index: usize) -> *mut NativeSolid;
    fn vb_parts_free(parts: *mut NativeParts);
    fn vb_solid_empty(
        solid: *const NativeSolid,
        empty: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_census(
        solid: *const NativeSolid,
        counts: *mut usize,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_from_brep(
        view: *const BrepView,
        tolerance: f64,
        components: *const usize,
        component_count: usize,
        inward: *const c_int,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_solid_brep(
        solid: *const NativeSolid,
        out: *mut *mut NativeBrep,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_brep_view(brep: *const NativeBrep, view: *mut BrepView) -> c_int;
    fn vb_brep_free(brep: *mut NativeBrep);
}

struct Payload {
    points: Vec<f64>,
    knots: [Vec<f64>; 2],
    degree: [usize; 2],
    count: [usize; 2],
}
impl Payload {
    fn view(&self) -> NurbsData {
        NurbsData {
            degree: self.degree,
            count: self.count,
            knot_count: self.knots.each_ref().map(|k| k.len()),
            knots: self.knots.each_ref().map(|k| k.as_ptr()),
            points: self.points.as_ptr(),
        }
    }
    fn curve(curve: &NurbsCurve) -> Self {
        Self {
            points: curve
                .control_points()
                .iter()
                .flat_map(|p| {
                    let [x, y, z] = p.point().to_array();
                    [x, y, z, p.weight()]
                })
                .collect(),
            knots: [curve.knots().to_vec(), Vec::new()],
            degree: [curve.degree(), 0],
            count: [curve.control_points().len(), 1],
        }
    }
    fn uv(curve: &NurbsCurve2) -> Self {
        Self {
            points: curve
                .control_points()
                .iter()
                .flat_map(|p| [p.point().x(), p.point().y(), 0., p.weight()])
                .collect(),
            knots: [curve.knots().to_vec(), Vec::new()],
            degree: [curve.degree(), 0],
            count: [curve.control_points().len(), 1],
        }
    }
    fn surface(surface: &NurbsSurface) -> Self {
        Self {
            points: surface
                .control_points()
                .iter()
                .flat_map(|p| {
                    let [x, y, z] = p.point().to_array();
                    [x, y, z, p.weight()]
                })
                .collect(),
            knots: [surface.knots_u().to_vec(), surface.knots_v().to_vec()],
            degree: [surface.degree_u(), surface.degree_v()],
            count: [
                surface.control_point_count_u(),
                surface.control_point_count_v(),
            ],
        }
    }
}
struct OwnedBrep(NonNull<NativeBrep>);
impl Drop for OwnedBrep {
    fn drop(&mut self) {
        unsafe { vb_brep_free(self.0.as_ptr()) };
    }
}

fn copied<T: Copy>(_: &OwnedBrep, pointer: *const T, count: usize) -> Result<Vec<T>, Error> {
    if count > 4_000_000 || (count != 0 && pointer.is_null()) {
        return Err(Error::InvalidOutput);
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    // The immutable native snapshot owns every view buffer until OwnedBrep drops.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count) }.to_vec())
}
fn borrowed<T>(_: &OwnedBrep, pointer: *const T, count: usize) -> Result<&[T], Error> {
    if count > 1_000_000 || (count != 0 && pointer.is_null()) {
        return Err(Error::InvalidOutput);
    }
    if count == 0 {
        return Ok(&[]);
    }
    // Typed arrays are immutable and their lifetime is tied to the owning snapshot.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count) })
}
fn range<T>(values: &[T], start: usize, count: usize) -> Result<&[T], Error> {
    values
        .get(start..start.checked_add(count).ok_or(Error::InvalidOutput)?)
        .ok_or(Error::InvalidOutput)
}

fn classify_iso(surface: &NurbsSurface, curve: &NurbsCurve2) -> SurfaceIso {
    let point = curve.control_points()[0].point();
    let controls = curve.control_points();
    if controls.iter().all(|p| p.point().x() == point.x()) {
        if point.x() == *surface.domain_u().start() {
            SurfaceIso::West
        } else if point.x() == *surface.domain_u().end() {
            SurfaceIso::East
        } else {
            SurfaceIso::InteriorUConstant
        }
    } else if controls.iter().all(|p| p.point().y() == point.y()) {
        if point.y() == *surface.domain_v().start() {
            SurfaceIso::South
        } else if point.y() == *surface.domain_v().end() {
            SurfaceIso::North
        } else {
            SurfaceIso::InteriorVConstant
        }
    } else {
        SurfaceIso::NotIso
    }
}

fn close_pole_gaps(
    surface: &NurbsSurface,
    vertices: &[BrepVertex],
    trims: Vec<BrepTrim>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, Error> {
    let mut gaps = Vec::with_capacity(trims.len());
    for (index, trim) in trims.iter().enumerate() {
        let next = &trims[(index + 1) % trims.len()];
        let end = trim.curve().end_point()?;
        let start = next.curve().start_point()?;
        if (end.x() - start.x()).abs() <= tolerance.absolute()
            && (end.y() - start.y()).abs() <= tolerance.absolute()
        {
            gaps.push(None);
            continue;
        }
        let vertex = trim.vertices()[1];
        if vertex != next.vertices()[0] {
            return Err(Error::InvalidOutput);
        }
        let (boundary, iso) = if end.y() == start.y() && end.y() == *surface.domain_v().start() {
            (surface.isocurve_u(end.y())?, SurfaceIso::South)
        } else if end.y() == start.y() && end.y() == *surface.domain_v().end() {
            (surface.isocurve_u(end.y())?, SurfaceIso::North)
        } else if end.x() == start.x() && end.x() == *surface.domain_u().start() {
            (surface.isocurve_v(end.x())?, SurfaceIso::West)
        } else if end.x() == start.x() && end.x() == *surface.domain_u().end() {
            (surface.isocurve_v(end.x())?, SurfaceIso::East)
        } else {
            return Err(Error::Kernel(format!(
                "UV loop gap is not a pole boundary: {end:?} to {start:?}"
            )));
        };
        let vertex = vertices.get(vertex).ok_or(Error::InvalidOutput)?;
        let sign = boundary.control_points()[0].weight().is_sign_positive();
        let epsilon = tolerance.absolute().max(vertex.tolerance());
        if boundary.control_points().iter().any(|p| {
            p.weight().is_sign_positive() != sign
                || p.point()
                    .distance_to(vertex.point())
                    .map_or(true, |distance| distance > epsilon)
        }) {
            return Err(Error::Kernel(
                "UV gap boundary is not collapsed within the native vertex tolerance".into(),
            ));
        }
        let curve = NurbsCurve2::try_new_rational(
            1,
            vec![
                WeightedPoint2::try_new(end, 1.)?,
                WeightedPoint2::try_new(start, 1.)?,
            ],
            vec![0., 0., 1., 1.],
        )?;
        let index = trim.vertices()[1];
        gaps.push(Some(BrepTrim::try_new(
            [index, index],
            None,
            false,
            curve,
            BrepTrimType::Singular,
            iso,
            [0.; 2],
        )?));
    }
    let mut completed =
        Vec::with_capacity(trims.len() + gaps.iter().filter(|g| g.is_some()).count());
    for (trim, gap) in trims.into_iter().zip(gaps) {
        completed.push(trim);
        if let Some(gap) = gap {
            completed.push(gap);
        }
    }
    Ok(completed)
}
impl NurbsData {
    fn points(self, owner: &OwnedBrep) -> Result<Vec<WeightedPoint3>, Error> {
        let count = self.count[0]
            .checked_mul(self.count[1])
            .filter(|n| *n <= 1_000_000)
            .ok_or(Error::InvalidOutput)?;
        let coordinates = copied(owner, self.points, count * 4)?;
        coordinates
            .chunks_exact(4)
            .map(|p| {
                WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2])?, p[3])
                    .map_err(Error::from)
            })
            .collect()
    }
    fn curve(self, owner: &OwnedBrep) -> Result<NurbsCurve, Error> {
        if self.count[1] != 1 || self.degree[1] != 0 {
            return Err(Error::InvalidOutput);
        }
        Ok(NurbsCurve::try_new_rational(
            self.degree[0],
            self.points(owner)?,
            copied(owner, self.knots[0], self.knot_count[0])?,
        )?)
    }
    fn uv_curve(self, owner: &OwnedBrep) -> Result<NurbsCurve2, Error> {
        let curve = self.curve(owner)?;
        let controls = curve
            .control_points()
            .iter()
            .map(|p| {
                let [x, y, z] = p.point().to_array();
                if z != 0. {
                    return Err(Error::InvalidOutput);
                }
                Ok(WeightedPoint2::try_new(Point2::try_new(x, y)?, p.weight())?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(NurbsCurve2::try_new_rational(
            curve.degree(),
            controls,
            curve.knots().to_vec(),
        )?)
    }
    fn surface(self, owner: &OwnedBrep) -> Result<NurbsSurface, Error> {
        Ok(NurbsSurface::try_new_rational(
            self.degree[0],
            self.degree[1],
            self.count[0],
            self.count[1],
            self.points(owner)?,
            copied(owner, self.knots[0], self.knot_count[0])?,
            copied(owner, self.knots[1], self.knot_count[1])?,
        )?)
    }
}

impl Solid {
    pub fn try_clone(&self) -> Result<Self, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_solid_copy(self.handle.as_ptr(), &mut pointer, error, capacity)
        })?;
        Ok(Self {
            handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
            _thread_local: PhantomData,
        })
    }
    /// Detect a non-point intersection between the two exact boundary graphs.
    pub fn boundary_contact(&self, other: &Self, tolerance: Tolerance) -> Result<bool, Error> {
        let mut contact = 0;
        call(|error, capacity| unsafe {
            vb_solid_boundary_contact(
                self.handle.as_ptr(),
                other.handle.as_ptr(),
                tolerance.absolute(),
                &mut contact,
                error,
                capacity,
            )
        })?;
        Ok(contact != 0)
    }
    pub fn material_parts(&self) -> Result<Vec<Self>, Error> {
        struct Parts(NonNull<NativeParts>);
        impl Drop for Parts {
            fn drop(&mut self) {
                unsafe { vb_parts_free(self.0.as_ptr()) };
            }
        }
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_solid_parts(self.handle.as_ptr(), &mut pointer, error, capacity)
        })?;
        let parts = Parts(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
        let count = unsafe { vb_parts_count(parts.0.as_ptr()) };
        if count > 128 {
            return Err(Error::InvalidOutput);
        }
        (0..count)
            .map(|index| {
                let handle = NonNull::new(unsafe { vb_parts_take(parts.0.as_ptr(), index) })
                    .ok_or(Error::InvalidOutput)?;
                Ok(Self {
                    handle,
                    _thread_local: PhantomData,
                })
            })
            .collect()
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        let mut empty = 0;
        call(|error, capacity| unsafe {
            vb_solid_empty(self.handle.as_ptr(), &mut empty, error, capacity)
        })?;
        Ok(empty != 0)
    }
    pub fn material_census(&self) -> Result<[usize; 2], Error> {
        let mut counts = [0; 2];
        call(|error, capacity| unsafe {
            vb_solid_census(self.handle.as_ptr(), counts.as_mut_ptr(), error, capacity)
        })?;
        Ok(counts)
    }

    /// Import validated, closed NURBS topology without fitting or healing.
    pub fn from_brep(brep: &Brep, tolerance: Tolerance) -> Result<Self, Error> {
        if !brep.is_solid() {
            return Err(Error::Kernel(
                "Import requires a closed manifold B-rep".into(),
            ));
        }
        let parts = brep.edge_connected_face_components();
        let inward = parts
            .iter()
            .map(|faces| {
                let shell = brep.sub_brep(faces, tolerance)?;
                let volume = shell.signed_volume(tolerance)?;
                if volume == 0. {
                    return Err(Error::Kernel("Zero-volume shell cannot be imported".into()));
                }
                Ok(c_int::from(volume < 0.))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut components = vec![0; brep.faces().len()];
        for (index, faces) in parts.iter().enumerate() {
            for &face in faces {
                components[face] = index;
            }
        }
        let vertices = brep
            .vertices()
            .iter()
            .map(|v| VertexData {
                point: v.point().to_array(),
                tolerance: v.tolerance(),
            })
            .collect::<Vec<_>>();
        let edge_geometry = brep
            .edges()
            .iter()
            .map(|e| Payload::curve(e.curve()))
            .collect::<Vec<_>>();
        let edges = brep
            .edges()
            .iter()
            .zip(&edge_geometry)
            .map(|(e, g)| EdgeData {
                vertices: e.vertices(),
                interval: [*e.curve().domain().start(), *e.curve().domain().end()],
                tolerance: e.tolerance(),
                curve: g.view(),
            })
            .collect::<Vec<_>>();
        let surface_geometry = brep
            .faces()
            .iter()
            .map(|f| Payload::surface(f.surface()))
            .collect::<Vec<_>>();
        let trim_geometry = brep
            .faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .map(|t| {
                let curve = if t.is_reversed_3d() {
                    t.curve().reversed()?
                } else {
                    t.curve().clone()
                };
                if let Some(edge) = t.edge() {
                    let lifted = NurbsCurve::try_new_rational(
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
                            .collect::<Result<Vec<_>, GeometryError>>()?,
                        curve.knots().to_vec(),
                    )?;
                    let aligned =
                        lifted.try_reparameterized(brep.edges()[edge].curve().domain())?;
                    Ok(Payload::curve(&aligned))
                } else {
                    Ok(Payload::uv(&curve))
                }
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let mut faces = Vec::new();
        let mut loops = Vec::new();
        let mut trims = Vec::new();
        for (face, g) in brep.faces().iter().zip(&surface_geometry) {
            let first_loop = loops.len();
            for boundary in face.loops() {
                let first_trim = trims.len();
                for trim in boundary.trims() {
                    trims.push(TrimData {
                        vertices: trim.vertices(),
                        edge: trim.edge().unwrap_or(usize::MAX),
                        reversed: c_int::from(trim.is_reversed_3d()),
                        curve: trim_geometry[trims.len()].view(),
                    });
                }
                loops.push(LoopData {
                    first_trim,
                    trim_count: trims.len() - first_trim,
                    inner: c_int::from(boundary.loop_type() == BrepLoopType::Inner),
                });
            }
            faces.push(FaceData {
                surface: g.view(),
                first_loop,
                loop_count: loops.len() - first_loop,
                reversed: c_int::from(face.is_reversed()),
            });
        }
        let view = BrepView {
            vertex_count: vertices.len(),
            edge_count: edges.len(),
            trim_count: trims.len(),
            loop_count: loops.len(),
            face_count: faces.len(),
            vertices: vertices.as_ptr(),
            edges: edges.as_ptr(),
            trims: trims.as_ptr(),
            loops: loops.as_ptr(),
            faces: faces.as_ptr(),
        };
        let mut pointer = std::ptr::null_mut();
        // All view arrays and owned NURBS buffers remain live throughout native construction.
        call(|error, capacity| unsafe {
            vb_solid_from_brep(
                &view,
                tolerance.absolute(),
                components.as_ptr(),
                parts.len(),
                inward.as_ptr(),
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

    /// Export owned NURBS geometry and shared topology into a validated Rust B-rep.
    pub fn to_brep(&self, tolerance: Tolerance) -> Result<Brep, Error> {
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_solid_brep(self.handle.as_ptr(), &mut pointer, error, capacity)
        })?;
        let owner = OwnedBrep(NonNull::new(pointer).ok_or(Error::InvalidOutput)?);
        let mut view = std::mem::MaybeUninit::<BrepView>::uninit();
        if unsafe { vb_brep_view(owner.0.as_ptr(), view.as_mut_ptr()) } != 0 {
            return Err(Error::InvalidOutput);
        }
        let view = unsafe { view.assume_init() };
        let vertices = borrowed(&owner, view.vertices, view.vertex_count)?
            .iter()
            .map(|v| {
                Ok(BrepVertex::try_new(
                    Point3::try_new(v.point[0], v.point[1], v.point[2])?,
                    v.tolerance,
                )?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let edge_data = borrowed(&owner, view.edges, view.edge_count)?;
        let edges = edge_data
            .iter()
            .map(|e| {
                let curve = e.curve.curve(&owner)?;
                let curve = if curve.domain() == (e.interval[0]..=e.interval[1]) {
                    curve
                } else {
                    curve.try_trimmed(e.interval[0]..=e.interval[1])?
                };
                Ok(BrepEdge::try_new(e.vertices, curve, e.tolerance)?)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let trim_data = borrowed(&owner, view.trims, view.trim_count)?;
        let loop_data = borrowed(&owner, view.loops, view.loop_count)?;
        let face_data = borrowed(&owner, view.faces, view.face_count)?;
        let mut uses = vec![Vec::new(); edges.len()];
        for (face_index, face) in face_data.iter().enumerate() {
            for loop_data in range(loop_data, face.first_loop, face.loop_count)? {
                for trim in range(trim_data, loop_data.first_trim, loop_data.trim_count)? {
                    uses.get_mut(trim.edge)
                        .ok_or(Error::InvalidOutput)?
                        .push(face_index);
                }
            }
        }
        let mut faces = Vec::new();
        for face in face_data {
            let surface = face.surface.surface(&owner)?;
            let mut loops = Vec::new();
            for loop_data in range(loop_data, face.first_loop, face.loop_count)? {
                let mut trims = Vec::new();
                for trim in range(trim_data, loop_data.first_trim, loop_data.trim_count)? {
                    let curve = match surface
                        .try_pullback_exact_curve(edges[trim.edge].curve(), tolerance)
                    {
                        Ok(curve) => curve,
                        Err(_) => trim.curve.uv_curve(&owner)?,
                    };
                    let curve = if trim.reversed != 0 {
                        curve.reversed()?
                    } else {
                        curve
                    };
                    let incidence = &uses[trim.edge];
                    let kind = match incidence.as_slice() {
                        [_] => BrepTrimType::Boundary,
                        [a, b] if a == b => BrepTrimType::Seam,
                        [_, _] => BrepTrimType::Mated,
                        _ => return Err(Error::InvalidOutput),
                    };
                    let iso = classify_iso(&surface, &curve);
                    trims.push(BrepTrim::try_new(
                        trim.vertices,
                        Some(trim.edge),
                        trim.reversed != 0,
                        curve,
                        kind,
                        iso,
                        [0.; 2],
                    )?);
                }
                let trims = close_pole_gaps(&surface, &vertices, trims, tolerance)?;
                loops.push(BrepLoop::try_new(
                    if loop_data.inner != 0 {
                        BrepLoopType::Inner
                    } else {
                        BrepLoopType::Outer
                    },
                    trims,
                )?);
            }
            loops.sort_by_key(|loop_data| loop_data.loop_type() != BrepLoopType::Outer);
            faces.push(BrepFace::try_new(surface, face.reversed != 0, loops)?);
        }
        Ok(Brep::try_new(vertices, edges, faces, tolerance)?)
    }
}
