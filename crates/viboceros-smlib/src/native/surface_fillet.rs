//! Rolling-ball surface fillets through the existing SMLib executive.
use super::*;
use viboceros_geometry::{AffineTransform3, Brep, BrepTrimType, SurfaceIso, SurfaceKnotDirection};
mod picks;

/// Treatment of private parent copies at the fillet contact rails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFilletParentMode {
    Split,
    Trim,
}

/// Owned fillet sheets and optional replacement parent faces. `None` retains
/// the original parent; `Some([])` indicates complete consumption. Replacement
/// parents retain the input surfaces' original UV charts.
#[derive(Clone, Debug)]
pub struct SurfaceFilletResult {
    pub fillet: Brep,
    pub parents: [Option<Vec<Brep>>; 2],
}

/// Offset direction relative to the surface's increasing-U cross increasing-V normal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceFilletSide {
    Positive,
    Negative,
}
impl SurfaceFilletSide {
    fn sign(self) -> c_int {
        match self {
            Self::Positive => 1,
            Self::Negative => -1,
        }
    }
}

/// A constant-radius circular fillet. Source surfaces are never trimmed or edited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceFilletOptions {
    pub radius: f64,
    pub sides: [SurfaceFilletSide; 2],
    /// Extend the fillet along the longer parent; otherwise use their overlap.
    pub extend: bool,
}
impl Default for SurfaceFilletOptions {
    fn default() -> Self {
        Self {
            radius: 1.,
            sides: [SurfaceFilletSide::Positive; 2],
            extend: true,
        }
    }
}

impl SurfaceFilletOptions {
    /// Select signed normal offsets from two UV picks in the original domains.
    /// Uses the normal at each bounded surface point nearest to the other
    /// surface pick. Singular projections and near-contact ambiguities are errors.
    /// This selects offset sides; it does not select
    /// among disconnected fillet components or apply source trimming.
    pub fn try_from_picks(
        first: &NurbsSurface,
        second: &NurbsSurface,
        picks: [[f64; 2]; 2],
        radius: f64,
        extend: bool,
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        picks::options(first, second, picks, radius, extend, tolerance)
    }
}

unsafe extern "C" {
    fn vb_surface_fillet_parents(
        first: *const brep::NurbsData,
        second: *const brep::NurbsData,
        radius: f64,
        sides: *const c_int,
        extend: c_int,
        extension_factor: f64,
        accuracy: f64,
        trim: c_int,
        out: *mut *mut NativeSolid,
        changed: *mut c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vb_surface_fillet(
        first: *const brep::NurbsData,
        second: *const brep::NurbsData,
        radius: f64,
        sides: *const c_int,
        extend: c_int,
        extension_factor: f64,
        accuracy: f64,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Build independently owned fillet sheets using native offset intersections,
/// rolling-ball tracing and exact rational circular cross sections. `None`
/// means the native producer returned no fillet. Kernel failures and
/// unsupported preparation propagate as errors, preserving both sources.
///
/// Inputs share the intersection adapter's local spatial frame, unit parameter
/// charts, common weight conditioning and private active-domain clamping.
/// Returned geometry uses world coordinates and native output parameterization.
/// This producer is the geometric foundation for FilletSrf; it does not apply
/// command picking, source trimming, joining, attributes or history policies.
pub fn fillet_surfaces(
    first: &NurbsSurface,
    second: &NurbsSurface,
    options: SurfaceFilletOptions,
    tolerance: Tolerance,
) -> Result<Option<Brep>, Error> {
    Ok(construct(first, second, options, None, tolerance)?.map(|r| r.fillet))
}

/// Construct the fillet and trim or split private parent copies with the native
/// rail insertion machinery. Inputs remain unchanged on success and failure.
/// This accepts two support surfaces, not preexisting trimmed B-rep faces.
pub fn fillet_surfaces_with_parents(
    first: &NurbsSurface,
    second: &NurbsSurface,
    options: SurfaceFilletOptions,
    mode: SurfaceFilletParentMode,
    tolerance: Tolerance,
) -> Result<Option<SurfaceFilletResult>, Error> {
    construct(first, second, options, Some(mode), tolerance)
}

fn construct(
    first: &NurbsSurface,
    second: &NurbsSurface,
    options: SurfaceFilletOptions,
    mode: Option<SurfaceFilletParentMode>,
    tolerance: Tolerance,
) -> Result<Option<SurfaceFilletResult>, Error> {
    let (prepared, origin, scale) = prepared_sources(first, second, options.radius)?;
    let mut boundaries = Vec::new();
    if options.extend {
        for source in &prepared {
            for axis in [SurfaceKnotDirection::U, SurfaceKnotDirection::V] {
                for parameter in [0., 1.] {
                    boundaries.push(super::surface_isocurve(source, axis, parameter)?);
                }
            }
        }
    }
    let a = knot_refinement::NurbsInput::surface(&prepared[0]);
    let b = knot_refinement::NurbsInput::surface(&prepared[1]);
    let sides = options.sides.map(SurfaceFilletSide::sign);
    let accuracy = tolerance.absolute() / scale;
    let local_tolerance = Tolerance::try_new(accuracy, tolerance.relative(), tolerance.angular())?;
    let transform = AffineTransform3::try_new(
        [[scale, 0., 0.], [0., scale, 0.], [0., 0., scale]],
        Vector3::try_from(origin.to_array())?,
    )?;
    for attempt in 0..32 {
        let extension_factor = if options.extend {
            0.5 * 2_f64.powi(attempt)
        } else {
            0.
        };
        let mut pointers = [std::ptr::null_mut(); 3];
        let mut changed = [0; 2];
        call(|error, capacity| unsafe {
            if let Some(mode) = mode {
                return vb_surface_fillet_parents(
                    &a.view(),
                    &b.view(),
                    options.radius / scale,
                    sides.as_ptr(),
                    c_int::from(options.extend),
                    extension_factor,
                    accuracy,
                    c_int::from(mode == SurfaceFilletParentMode::Trim),
                    pointers.as_mut_ptr(),
                    changed.as_mut_ptr(),
                    error,
                    capacity,
                );
            }
            vb_surface_fillet(
                &a.view(),
                &b.view(),
                options.radius / scale,
                sides.as_ptr(),
                c_int::from(options.extend),
                extension_factor,
                accuracy,
                &mut pointers[0],
                error,
                capacity,
            )
        })?;
        // Own every transferred handle before any fallible export.
        let mut shapes = pointers.map(|p| {
            NonNull::new(p).map(|handle| Shape {
                model: Solid {
                    handle,
                    _thread_local: PhantomData,
                },
            })
        });
        let Some(shape) = shapes[0].take() else {
            return Ok(None);
        };
        let output = shape
            .model
            .to_brep_with_closed_uv_joints(local_tolerance, Some(accuracy * 0.25))?;
        // A fixed half-span search can stop while a fillet end is still inside
        // the longer parent. Enlarge the private native search until every
        // boundary end cap reaches an actual original parent boundary.
        if !options.extend || caps_reach_parent_boundaries(&output, &boundaries, local_tolerance)? {
            let mut parents = [None, None];
            for (i, shape) in shapes[1..].iter().enumerate() {
                if changed[i] != 0 {
                    parents[i] = Some(Vec::new());
                }
                if let Some(shape) = shape {
                    let parent = shape
                        .model
                        .to_brep_with_closed_uv_joints(local_tolerance, Some(accuracy * 0.25))?
                        .transformed(transform, tolerance)?;
                    let source = if i == 0 { first } else { second };
                    let domains = [source.domain_u(), source.domain_v()];
                    let parent = if parent.faces().iter().all(|face| {
                        [face.surface().domain_u(), face.surface().domain_v()] == domains
                    }) {
                        parent
                    } else {
                        parent.try_reparameterized_face_uv(
                            (0..parent.faces().len()).map(|face| (face, domains.clone())),
                            tolerance,
                        )?
                    };
                    parents[i] = Some(parent.explode_faces(tolerance)?);
                }
            }
            return Ok(Some(SurfaceFilletResult {
                fillet: output.transformed(transform, tolerance)?,
                parents,
            }));
        }
    }
    Err(Error::Kernel(
        "Surface fillet extension search limit".into(),
    ))
}

fn prepared_sources(
    first: &NurbsSurface,
    second: &NurbsSurface,
    radius: f64,
) -> Result<([NurbsSurface; 2], Point3, f64), Error> {
    if !radius.is_finite() || radius <= 0. {
        return Err(Error::Kernel(
            "Surface fillet radius must be finite and positive".into(),
        ));
    }
    let origin = first.evaluate(*first.domain_u().start(), *first.domain_v().start())?;
    let mut scale = radius;
    for p in first.control_points().iter().chain(second.control_points()) {
        scale = scale.max(origin.distance_to(p.point())?);
    }
    let prepare = |source| {
        super::curve_surface_intersections::prepared_surface(source, origin, scale)?
            .ok_or_else(|| Error::Kernel("Unsupported surface fillet input preparation".into()))
    };
    Ok(([prepare(first)?, prepare(second)?], origin, scale))
}

fn caps_reach_parent_boundaries(
    output: &Brep,
    boundaries: &[NurbsCurve],
    tolerance: Tolerance,
) -> Result<bool, Error> {
    for face in output.faces() {
        for boundary in face.loops() {
            for trim in boundary.trims() {
                if trim.trim_type() != BrepTrimType::Boundary
                    || matches!(
                        trim.iso(),
                        SurfaceIso::South | SurfaceIso::North | SurfaceIso::InteriorVConstant
                    )
                {
                    continue;
                }
                let domain = face.surface().domain_v();
                let epsilon = (domain.end() - domain.start()) * (256. * f64::EPSILON);
                let endpoints = [trim.curve().start_point()?, trim.curve().end_point()?];
                for (vertex, uv) in trim.vertices().into_iter().zip(endpoints) {
                    // An end cap can contain intermediate topology vertices.
                    // Only its junctions with the two rails bound extension.
                    if (uv.y() - domain.start())
                        .abs()
                        .min((uv.y() - domain.end()).abs())
                        > epsilon
                    {
                        continue;
                    }
                    let vertex = output.vertices()[vertex];
                    let mut reached = false;
                    for curve in boundaries {
                        let t = curve.closest_parameter(vertex.point(), tolerance)?;
                        let gap = curve.evaluate(t)?.distance_to(vertex.point())?;
                        if gap <= tolerance.absolute().max(vertex.tolerance()) {
                            reached = true;
                            break;
                        }
                    }
                    if !reached {
                        return Ok(false);
                    }
                }
            }
        }
    }
    Ok(true)
}
