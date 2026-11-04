//! Native chain partitioning on an owned copy; source ordinals stay immutable.
use super::*;

/// Public OpenNURBS continuation constraints, independent of display density.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SubDEdgeChainType {
    Mixed = 1,
    EqualEdgeTag = 2,
    EqualEdgeAndVertexTag = 3,
    EqualEdgeTagAndOrdinary = 4,
    EqualEdgeAndVertexTagAndOrdinary = 5,
}

#[repr(C)]
struct NativeChains {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default)]
struct ChainsView {
    chain_count: usize,
    edge_count: usize,
    offsets: *const usize,
    directed_edges: *const usize,
}
unsafe extern "C" {
    fn vibo_subd_edge_chains(
        bytes: *const u8,
        count: usize,
        edges: *const usize,
        edge_count: usize,
        out: *mut *mut NativeChains,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_edge_chains_view(value: *const NativeChains, out: *mut ChainsView) -> c_int;
    fn vibo_subd_edge_loop(
        bytes: *const u8,
        count: usize,
        seed: usize,
        chain_type: u8,
        out: *mut *mut NativeChains,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn vibo_subd_edge_chains_free(value: *mut NativeChains);
}
struct ChainsOwner(NonNull<NativeChains>);
impl Drop for ChainsOwner {
    fn drop(&mut self) {
        unsafe { vibo_subd_edge_chains_free(self.0.as_ptr()) }
    }
}
/// Partitions a unique selection into connected directed chains. Each pair is
/// (source edge ordinal, reversed). The selection order does not affect sorting.
pub fn subd_edge_chains(
    bytes: &[u8],
    edges: &[usize],
) -> Result<Vec<Vec<(usize, bool)>>, SubDError> {
    check_archive(bytes)?;
    if edges.len() > 400_000 {
        return Err(SubDError("Excessive SubD chain selection".into()));
    }
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_edge_chains(
            bytes.as_ptr(),
            bytes.len(),
            edges.as_ptr(),
            edges.len(),
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    decode_chains(status, out, &error, Some(edges.len()))
}

/// Grow a seed through the public native chain-neighbor query. Work happens on
/// a private decoded source; cycles terminate when an edge is encountered twice.
pub fn subd_edge_loop(
    bytes: &[u8],
    seed: usize,
    chain_type: SubDEdgeChainType,
) -> Result<Vec<(usize, bool)>, SubDError> {
    check_archive(bytes)?;
    let mut out = std::ptr::null_mut();
    let mut error = [0u8; 1024];
    let status = unsafe {
        vibo_subd_edge_loop(
            bytes.as_ptr(),
            bytes.len(),
            seed,
            chain_type as u8,
            &mut out,
            error.as_mut_ptr().cast(),
            error.len(),
        )
    };
    let mut chains = decode_chains(status, out, &error, None)?;
    if chains.len() != 1 {
        return Err(SubDError("Invalid native SubD loop count".into()));
    }
    Ok(chains.pop().unwrap())
}

fn decode_chains(
    status: c_int,
    out: *mut NativeChains,
    error: &[u8],
    expected: Option<usize>,
) -> Result<Vec<Vec<(usize, bool)>>, SubDError> {
    let owner = NonNull::new(out).map(ChainsOwner);
    if status != 1 {
        let end = error.iter().position(|&b| b == 0).unwrap_or(error.len());
        return Err(SubDError(
            String::from_utf8_lossy(&error[..end]).into_owned(),
        ));
    }
    let owner = owner.ok_or_else(|| SubDError("Missing SubD chains".into()))?;
    let mut view = ChainsView::default();
    if unsafe { vibo_subd_edge_chains_view(owner.0.as_ptr(), &mut view) } != 1
        || view.chain_count > 400_000
        || view.edge_count > 400_000
        || expected.is_some_and(|n| view.chain_count > n || view.edge_count != n)
        || view.offsets.is_null()
        || (view.edge_count > 0 && view.directed_edges.is_null())
    {
        return Err(SubDError("Invalid native SubD chain view".into()));
    }
    // Native allocations remain owned for both copies, including the empty case.
    let offsets = unsafe { std::slice::from_raw_parts(view.offsets, view.chain_count + 1) };
    let directed = if view.edge_count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(view.directed_edges, view.edge_count) }
    };
    if offsets.first() != Some(&0)
        || offsets.last() != Some(&directed.len())
        || offsets
            .windows(2)
            .any(|p| p[0] >= p[1] || p[1] > directed.len())
        || directed.iter().any(|i| i / 2 >= 400_000)
    {
        return Err(SubDError("Invalid native SubD chain layout".into()));
    }
    Ok(offsets
        .windows(2)
        .map(|p| {
            directed[p[0]..p[1]]
                .iter()
                .map(|&i| (i / 2, i % 2 != 0))
                .collect()
        })
        .collect())
}
