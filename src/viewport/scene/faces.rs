//! Standard and analysis face staging, including clipping and normal transforms.
use super::*;

impl Viewport {
    #[cfg(test)]
    pub(in crate::viewport) fn add_gpu_mesh_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        mesh: &TriangleMesh,
        color: Color32,
    ) {
        if self.display_mode == DisplayMode::Wireframe {
            return;
        }

        self.add_gpu_mesh_faces_with_normals(
            scene,
            mesh,
            mesh_normals::mesh_corner_normals(mesh),
            FaceAppearance {
                color,
                member_colors: true,
                draft: None,
                analysis_colors: None,
            },
            None,
        );
    }

    pub(super) fn add_gpu_mesh_faces_with_normals(
        &self,
        scene: &mut GpuSceneBuilder,
        mesh: &TriangleMesh,
        corner_normals: impl IntoIterator<Item = [NaVector3<Real>; 3]>,
        appearance: FaceAppearance,
        transform: Option<AffineMeshTransform>,
    ) {
        let face_color = if self.display_mode == DisplayMode::Ghosted {
            color_with_alpha(appearance.color, 35)
        } else {
            appearance.color
        };
        let gpu_color = color_to_gpu(face_color);
        for (triangle_index, normals) in corner_normals.into_iter().enumerate() {
            let Some(mut points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            let mut normals = normals;
            if let Some(transform) = transform {
                let [Ok(a), Ok(b), Ok(c)] =
                    points.map(|point| transform.transform.transform_point(point))
                else {
                    continue;
                };
                let [Ok(na), Ok(nb), Ok(nc)] = normals.map(|normal| {
                    transform
                        .normals
                        .transform_normal(Vector3::try_new(normal.x, normal.y, normal.z)?)
                }) else {
                    continue;
                };
                points = [a, b, c];
                normals = [na, nb, nc].map(|n| {
                    let n = n.as_vector();
                    NaVector3::new(n.x(), n.y(), n.z())
                });
                if transform.reverse_normals {
                    normals = normals.map(|n| -n);
                }
            }
            let clipped = self.clip_triangle(points);
            if clipped[0].is_none() {
                continue;
            }
            // Submit the original triangle so hardware clipping interpolates
            // smooth normals correctly. Only visible geometry sets the depth
            // range; behind-camera vertices must not move the near plane.
            let [Some(first), Some(second), Some(third)] =
                points.map(|point| self.gpu_position(point))
            else {
                continue;
            };
            let vertex_colors = if let Some(colors) = appearance.analysis_colors.as_ref() {
                Some(
                    colors[triangle_index]
                        .map(|[r, g, b]| color_to_gpu(Color32::from_rgb(r, g, b))),
                )
            } else if let Some(mapping) = appearance.draft {
                let colors = normals.map(|n| {
                    Vector3::try_new(n.x, n.y, n.z).map_or([0, 0, 0], |n| mapping.color(n))
                });
                Some(colors.map(|[r, g, b]| color_to_gpu(Color32::from_rgb(r, g, b))))
            } else if appearance.member_colors {
                mesh.vertex_colors().map(|colors| {
                    mesh.triangles()[triangle_index].map(|index| {
                        let [red, green, blue, transparency] = colors[index as usize];
                        let alpha = if self.display_mode == DisplayMode::Ghosted {
                            35
                        } else {
                            255 - transparency
                        };
                        color_to_gpu(Color32::from_rgba_unmultiplied(red, green, blue, alpha))
                    })
                })
            } else {
                None
            }
            .unwrap_or([gpu_color; 3]);
            let normals = normals.map(vector_to_gpu);
            let shading = if appearance.draft.is_some() || appearance.analysis_colors.is_some() {
                0.
            } else {
                1.
            };
            let vertex_depths = points.map(|point| self.view_depth(point));
            let depth_scale = vertex_depths.iter().map(|d| d.abs()).fold(0.0, Real::max);
            let mut depth = 0.0;
            let count = if clipped[1].is_some() { 4.0 } else { 3.0 };
            for point in clipped[0]
                .into_iter()
                .flatten()
                .chain(clipped[1].map(|triangle| triangle[2]))
            {
                let point_depth = self.view_depth(point);
                scene.include_depth(point_depth);
                if depth_scale > 0.0 {
                    depth += point_depth / depth_scale;
                }
            }
            let depth = (depth / count) * depth_scale;
            scene.triangles.push(DepthTriangle {
                depth,
                vertex_depths,
                vertices: [
                    GpuTriangleVertex {
                        position: first,
                        normal: normals[0],
                        color: vertex_colors[0],
                        shading,
                    },
                    GpuTriangleVertex {
                        position: second,
                        normal: normals[1],
                        color: vertex_colors[1],
                        shading,
                    },
                    GpuTriangleVertex {
                        position: third,
                        normal: normals[2],
                        color: vertex_colors[2],
                        shading,
                    },
                ],
            });
        }
    }
}

fn vector_to_gpu(vector: NaVector3<Real>) -> [f32; 3] {
    [vector.x as f32, vector.y as f32, vector.z as f32]
}
