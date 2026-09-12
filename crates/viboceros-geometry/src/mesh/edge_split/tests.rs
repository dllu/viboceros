use super::*;

#[test]
fn output_vertex_limits_match_wide_integer_reference_without_allocating_meshes() {
    for retained in [0, 1, 7, u32::MAX as usize, usize::MAX] {
        for generated in [0, 1, 2, usize::MAX / 3, usize::MAX] {
            for welded in [false, true] {
                let count = retained as u128 + if welded { 1 } else { 3 * generated as u128 };
                let expected = if count <= usize::MAX as u128 && count <= u32::MAX as u128 + 1 {
                    Ok(count as usize)
                } else {
                    Err(GeometryError::TooManyMeshVertices)
                };
                assert_eq!(
                    output_vertex_count(retained, generated, welded),
                    expected,
                    "retained={retained}, generated={generated}, welded={welded}"
                );
            }
        }
    }
}
