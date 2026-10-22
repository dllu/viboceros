//! Owned public extrusion archives and exact canonical B-rep transfer.
use std::ffi::{c_char, c_int};
use std::ptr::NonNull;

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("OpenNURBS extrusion: {0}")]
pub struct ExtrusionError(pub String);
#[derive(Debug)]
pub struct ExtrusionData {
    pub archive: Vec<u8>,
    pub brep: Vec<u8>,
}
pub struct ExtrusionProfile<'a> {
    pub degree: u32,
    pub controls: &'a [[f64; 4]],
    pub knots: &'a [f64],
}
#[repr(C)]
struct NativeResult {
    _opaque: [u8; 0],
}
unsafe extern "C" {
    fn vibo_extrusion_read(
        bytes: *const u8,
        count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_extrusion_create(
        degree: u32,
        count: usize,
        controls: *const f64,
        knot_count: usize,
        knots: *const f64,
        frame: *const f64,
        height: f64,
        cap: c_int,
        preserve_domain: c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_extrusion_transform(
        bytes: *const u8,
        count: usize,
        matrix: *const f64,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_extrusion_view(
        result: *const NativeResult,
        archive: *mut *const u8,
        archive_count: *mut usize,
        brep: *mut *const u8,
        brep_count: *mut usize,
    ) -> c_int;
    fn vibo_extrusion_free(result: *mut NativeResult);
}
struct Owner(NonNull<NativeResult>);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe { vibo_extrusion_free(self.0.as_ptr()) }
    }
}
fn call(
    run: impl FnOnce(*mut *mut NativeResult, *mut c_char, usize) -> c_int,
) -> Result<ExtrusionData, ExtrusionError> {
    let mut raw = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = run(&mut raw, error.as_mut_ptr().cast(), error.len());
    let owner = NonNull::new(raw).map(Owner);
    if status != 1 {
        let end = error.iter().position(|&x| x == 0).unwrap_or(error.len());
        return Err(ExtrusionError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| ExtrusionError("Missing owned result".into()))?;
    let (mut archive, mut brep) = (std::ptr::null(), std::ptr::null());
    let (mut archive_count, mut brep_count) = (0, 0);
    // Native buffers remain owned until both bounded copies complete.
    let status = unsafe {
        vibo_extrusion_view(
            owner.0.as_ptr(),
            &mut archive,
            &mut archive_count,
            &mut brep,
            &mut brep_count,
        )
    };
    if status != 1
        || archive.is_null()
        || brep.is_null()
        || !(17..=64 * 1024 * 1024).contains(&archive_count)
        || !(1..=256 * 1024 * 1024).contains(&brep_count)
    {
        return Err(ExtrusionError("Invalid owned transfer buffers".into()));
    }
    Ok(ExtrusionData {
        archive: unsafe { std::slice::from_raw_parts(archive, archive_count) }.to_vec(),
        brep: unsafe { std::slice::from_raw_parts(brep, brep_count) }.to_vec(),
    })
}
fn check_archive(bytes: &[u8]) -> Result<(), ExtrusionError> {
    if !(17..=64 * 1024 * 1024).contains(&bytes.len()) || !bytes.starts_with(b"VIBOEXTR") {
        return Err(ExtrusionError("Invalid archive envelope".into()));
    }
    Ok(())
}
pub fn extrusion_read(bytes: &[u8]) -> Result<ExtrusionData, ExtrusionError> {
    check_archive(bytes)?;
    call(|out, error, capacity| unsafe {
        vibo_extrusion_read(bytes.as_ptr(), bytes.len(), out, error, capacity)
    })
}
pub fn extrusion_transform(
    bytes: &[u8],
    matrix: [[f64; 4]; 4],
) -> Result<ExtrusionData, ExtrusionError> {
    check_archive(bytes)?;
    if matrix.as_flattened().iter().any(|x| !x.is_finite()) {
        return Err(ExtrusionError("Nonfinite transform".into()));
    }
    call(|out, error, capacity| unsafe {
        vibo_extrusion_transform(
            bytes.as_ptr(),
            bytes.len(),
            matrix.as_flattened().as_ptr(),
            out,
            error,
            capacity,
        )
    })
}
/// The frame holds origin/X/Y. The public factory projects the profile onto it.
pub fn extrusion_create(
    profile: ExtrusionProfile<'_>,
    frame: [f64; 9],
    height: f64,
    cap: bool,
) -> Result<ExtrusionData, ExtrusionError> {
    create_profile(profile, frame, height, cap, false)
}
/// Command construction keeps an uncapped closed profile's original interval.
pub fn extrusion_create_command(
    profile: ExtrusionProfile<'_>,
    frame: [f64; 9],
    height: f64,
    cap: bool,
) -> Result<ExtrusionData, ExtrusionError> {
    create_profile(profile, frame, height, cap, !cap)
}
fn create_profile(
    profile: ExtrusionProfile<'_>,
    frame: [f64; 9],
    height: f64,
    cap: bool,
    preserve_domain: bool,
) -> Result<ExtrusionData, ExtrusionError> {
    let n = profile.controls.len();
    if profile.degree == 0
        || profile.degree as usize >= n
        || n > 100_000
        || profile.knots.len() != n + profile.degree as usize + 1
        || !height.is_finite()
        || height == 0.
        || profile
            .controls
            .as_flattened()
            .iter()
            .chain(profile.knots)
            .chain(&frame)
            .any(|x| !x.is_finite())
    {
        return Err(ExtrusionError("Invalid profile, plane or height".into()));
    }
    call(|out, error, capacity| unsafe {
        vibo_extrusion_create(
            profile.degree,
            n,
            profile.controls.as_flattened().as_ptr(),
            profile.knots.len(),
            profile.knots.as_ptr(),
            frame.as_ptr(),
            height,
            cap.into(),
            preserve_domain.into(),
            out,
            error,
            capacity,
        )
    })
}

mod surface;
pub use surface::{ExtrusionSurfaceData, extrusion_surface};
