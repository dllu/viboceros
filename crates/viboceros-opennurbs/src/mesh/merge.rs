//! Safe typed inputs for the public OpenNURBS face-set merge operation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshMergeComponent {
    /// Original raw vertex index, resolved before any welding or compaction.
    Vertex(usize),
    /// A topology edge identified by two original raw endpoint indices.
    Edge([usize; 2]),
    /// Original stored face index. A member facet identifies its entire n-gon.
    Face(usize),
}
#[repr(C)]
struct Component {
    kind: u32,
    first: u32,
    second: u32,
}
unsafe extern "C" {
    fn vibo_mesh_merge_components(
        input: *const MeshView,
        selected: *const Component,
        count: usize,
        out: *mut *mut NativeResult,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// Merge connected face sets selected by faces, vertices or edges. OpenNURBS
/// partitions the selected sets and retains stored channels. Each new n-gon
/// requires one boundary loop; a region with a hole cannot become one face.
/// Explicit face selections may weld coincident seam vertices, as upstream does.
/// The second result is the decrease in logical face count (ungrouped facets
/// plus n-gons).
/// The input is unchanged on success, rejection and empty selection.
pub fn merge_mesh_components(
    data: &MeshData,
    selection: &[MeshMergeComponent],
) -> Result<(MeshData, usize), MeshError> {
    if selection.len() > 4_000_000 {
        return Err(MeshError("Excessive merge component count".into()));
    }
    if selection.is_empty() {
        return Ok((data.clone(), 0));
    }
    let index = |i: usize, limit: usize| -> Result<u32, MeshError> {
        if i >= limit {
            return Err(MeshError("Merge component index is out of range".into()));
        }
        u32::try_from(i).map_err(|_| MeshError("Merge component index is excessive".into()))
    };
    let selected = selection
        .iter()
        .map(|component| -> Result<Component, MeshError> {
            Ok(match *component {
                MeshMergeComponent::Vertex(i) => Component {
                    kind: 0,
                    first: index(i, data.vertices.len())?,
                    second: 0,
                },
                MeshMergeComponent::Edge([a, b]) => Component {
                    kind: 1,
                    first: index(a, data.vertices.len())?,
                    second: index(b, data.vertices.len())?,
                },
                MeshMergeComponent::Face(i) => Component {
                    kind: 2,
                    first: index(i, data.faces.len())?,
                    second: 0,
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    with_mesh_view(data, |view| {
        let mut raw = std::ptr::null_mut();
        let mut error = [0; 512];
        let status = unsafe {
            vibo_mesh_merge_components(
                view,
                selected.as_ptr(),
                selected.len(),
                &mut raw,
                error.as_mut_ptr().cast(),
                error.len(),
            )
        };
        // This callback transfers one fresh uniquely owned bridge result.
        let result = unsafe { read_mesh_result(raw, status, &error, 16) }?;
        Ok((result.mesh, result.changed))
    })?
}
