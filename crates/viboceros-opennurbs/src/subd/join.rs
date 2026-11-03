//! One owned native assembly after Rust has chosen contacts and orientation.
use super::*;
unsafe extern "C" {
    fn vibo_subd_join_assemble(
        bytes: *const u8,
        byte_count: usize,
        sizes: *const usize,
        source_count: usize,
        maps: *const u32,
        map_count: usize,
        flips: *const u8,
        crease: c_int,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}
pub fn subd_join_assemble(
    sources: &[(&[u8], &[u32], bool)],
    crease: bool,
) -> Result<SubDData, SubDError> {
    let mut bytes = Vec::new();
    let mut sizes = Vec::new();
    let mut maps = Vec::new();
    let mut flips = Vec::new();
    for &(archive, mapping, flip) in sources {
        check_archive(archive)?;
        if bytes.len().saturating_add(archive.len()) > 64 * 1024 * 1024
            || maps.len().saturating_add(mapping.len()) > 100_000
        {
            return Err(SubDError("SubD join assembly limit exceeded".into()));
        }
        sizes.push(archive.len());
        bytes.extend_from_slice(archive);
        maps.extend_from_slice(mapping);
        flips.push(u8::from(flip));
    }
    call(|out, error, capacity| unsafe {
        vibo_subd_join_assemble(
            bytes.as_ptr(),
            bytes.len(),
            sizes.as_ptr(),
            sizes.len(),
            maps.as_ptr(),
            maps.len(),
            flips.as_ptr(),
            c_int::from(crease),
            out,
            error,
            capacity,
        )
    })
}
