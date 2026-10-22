//! Owned, bounded samples of original control-edge limit curves.
use super::*;

#[repr(C)]
struct NativeSamples {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct SamplesView {
    edge_count: usize,
    point_count: usize,
    offsets: *const usize,
    points: *const [f64; 3],
}
unsafe extern "C" {
    fn vibo_subd_edge_samples(
        bytes: *const u8,
        count: usize,
        density: u32,
        out: *mut *mut NativeSamples,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_edge_samples_view(result: *const NativeSamples, out: *mut SamplesView) -> c_int;
    fn vibo_subd_edge_samples_free(result: *mut NativeSamples);
}
struct SamplesOwner(NonNull<NativeSamples>);
impl Drop for SamplesOwner {
    fn drop(&mut self) {
        unsafe { vibo_subd_edge_samples_free(self.0.as_ptr()) }
    }
}

/// Increasing source edge IDs; each polyline is oriented from vertex 0 to 1.
/// Vertices are exact limit evaluations; straight segments approximate the
/// intervening surface curve. Sampling never changes the input archive.
pub fn subd_edge_samples(bytes: &[u8], density: u32) -> Result<Vec<Vec<[f64; 3]>>, SubDError> {
    check_archive(bytes)?;
    if !(1..=6).contains(&density) {
        return Err(SubDError("Invalid edge sample density".into()));
    }
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_edge_samples(
            bytes.as_ptr(),
            bytes.len(),
            density,
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let owner = NonNull::new(out).map(SamplesOwner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing edge sample result".into()))?;
    let mut view = SamplesView::default();
    if unsafe { vibo_subd_edge_samples_view(owner.0.as_ptr(), &mut view) } != 1
        || view.edge_count > 400_000
        || view.point_count > 4_000_000
        || view.offsets.is_null()
        || (view.point_count > 0 && view.points.is_null())
    {
        return Err(SubDError("Invalid edge sample view".into()));
    }
    // Buffers remain owned for the entire copy; avoid a null zero-length slice.
    let offsets = unsafe { std::slice::from_raw_parts(view.offsets, view.edge_count + 1) };
    let points = if view.point_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(view.points, view.point_count) }
    };
    if offsets.first() != Some(&0)
        || offsets.last() != Some(&points.len())
        || offsets
            .windows(2)
            .any(|w| w[0] > w[1] || w[1] - w[0] != (1usize << density) + 1)
        || points.iter().flatten().any(|x| !x.is_finite())
    {
        return Err(SubDError("Invalid edge sample layout".into()));
    }
    Ok(offsets
        .windows(2)
        .map(|w| points[w[0]..w[1]].to_vec())
        .collect())
}
