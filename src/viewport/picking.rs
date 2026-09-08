//! Screen capture and depth ordering for mesh and tessellated surface hits.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct PickHit {
    pub distance: f32,
    priority: u8,
    depth: Real,
}

impl PickHit {
    pub(super) fn screen(priority: u8, distance: f32) -> Self {
        Self {
            distance,
            priority,
            depth: Real::INFINITY,
        }
    }

    pub(super) fn is_better_than(self, other: Self) -> bool {
        self.distance < other.distance
            || (self.distance == other.distance
                && (self.priority < other.priority
                    || (self.priority == other.priority && self.depth < other.depth)))
    }
}

pub(super) fn signed_area(start: Pos2, end: Pos2, target: Pos2) -> Real {
    (Real::from(end.x) - Real::from(start.x)).mul_add(
        Real::from(target.y) - Real::from(start.y),
        -(Real::from(end.y) - Real::from(start.y)) * (Real::from(target.x) - Real::from(start.x)),
    )
}

/// Perspective projection interpolates reciprocal view depth in screen space.
/// Inputs are a clipped triangle and a cursor already accepted inside it.
fn triangle_depth(
    pointer: Pos2,
    screen: [Pos2; 3],
    depths: [Real; 3],
    perspective: bool,
) -> Option<Real> {
    let area = signed_area(screen[0], screen[1], screen[2]);
    if !area.is_finite() || area.abs() <= Real::EPSILON {
        return None;
    }
    let weights = [
        signed_area(screen[1], screen[2], pointer) / area,
        signed_area(screen[2], screen[0], pointer) / area,
        signed_area(screen[0], screen[1], pointer) / area,
    ];
    let depth = if perspective {
        if depths.iter().any(|depth| *depth <= 0.0) {
            return None;
        }
        1.0 / weights
            .into_iter()
            .zip(depths)
            .map(|(w, d)| w / d)
            .sum::<Real>()
    } else {
        weights.into_iter().zip(depths).map(|(w, d)| w * d).sum()
    };
    (depth.is_finite() && (!perspective || depth > 0.0)).then_some(depth)
}

impl Viewport {
    pub(super) fn mesh_pick(
        &self,
        pointer: Pos2,
        rect: Rect,
        mesh: &TriangleMesh,
        tolerance: Tolerance,
    ) -> PickHit {
        if self.display_mode == DisplayMode::Wireframe {
            let distance = mesh
                .wireframe_lines(tolerance)
                .map(|lines| {
                    lines
                        .into_iter()
                        .filter_map(|line| self.project_segment(line.start(), line.end(), rect))
                        .map(|[start, end]| point_segment_distance(pointer, start, end))
                        .fold(f32::INFINITY, f32::min)
                })
                .unwrap_or(f32::INFINITY);
            return PickHit::screen(2, distance);
        }
        let mut nearest = PickHit::screen(2, f32::INFINITY);
        for triangle_index in 0..mesh.triangles().len() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            for points in self.clip_triangle(points).into_iter().flatten() {
                let [Some(first), Some(second), Some(third)] =
                    points.map(|point| self.project(point, rect))
                else {
                    continue;
                };
                let hit = if point_in_triangle(pointer, first, second, third) {
                    let Some(depth) = triangle_depth(
                        pointer,
                        [first, second, third],
                        points.map(|p| self.view_depth(p)),
                        !self.kind.is_parallel(),
                    ) else {
                        continue;
                    };
                    PickHit {
                        distance: 0.0,
                        priority: 2,
                        depth,
                    }
                } else {
                    PickHit::screen(
                        2,
                        point_segment_distance(pointer, first, second)
                            .min(point_segment_distance(pointer, second, third))
                            .min(point_segment_distance(pointer, third, first)),
                    )
                };
                if hit.is_better_than(nearest) {
                    nearest = hit;
                }
            }
        }
        nearest
    }

    pub(super) fn nurbs_surface_pick(
        &self,
        pointer: Pos2,
        rect: Rect,
        surface: &NurbsSurface,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> PickHit {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick(pointer, rect, &mesh, tolerance);
        }
        PickHit::screen(
            2,
            surface
                .wireframe_curves(wire_density)
                .map(|curves| {
                    curves
                        .iter()
                        .map(|curve| self.nurbs_pick_distance(pointer, rect, curve))
                        .fold(f32::INFINITY, f32::min)
                })
                .unwrap_or(f32::INFINITY),
        )
    }

    pub(super) fn brep_pick(
        &self,
        pointer: Pos2,
        rect: Rect,
        brep: &Brep,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> PickHit {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick(pointer, rect, &mesh, tolerance);
        }
        PickHit::screen(
            2,
            brep.wireframe_curves(wire_density, tolerance)
                .map(|curves| {
                    curves
                        .iter()
                        .map(|curve| self.nurbs_pick_distance(pointer, rect, curve))
                        .fold(f32::INFINITY, f32::min)
                })
                .unwrap_or(f32::INFINITY),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_face_hit_uses_nearest_triangle_even_when_it_is_last() {
        let mut viewport = Viewport::new(ViewKind::Top);
        viewport.display_mode = DisplayMode::Shaded;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.0));
        let vertices = [1.0, 5.0]
            .into_iter()
            .flat_map(|z| {
                [(-1.0, -1.0), (1.0, -1.0), (0.0, 1.0)]
                    .map(|(x, y)| Point3::try_new(x, y, z).unwrap())
            })
            .collect::<Vec<_>>();
        for triangles in [vec![[0, 1, 2], [3, 4, 5]], vec![[3, 4, 5], [0, 1, 2]]] {
            let mesh =
                TriangleMesh::try_new(vertices.clone(), triangles, Tolerance::DEFAULT).unwrap();
            let hit = viewport.mesh_pick(rect.center(), rect, &mesh, Tolerance::DEFAULT);
            assert_eq!(hit.distance, 0.0);
            assert_eq!(hit.depth, -5.0);
        }
    }

    #[test]
    fn face_depth_interpolation_is_perspective_correct_and_winding_independent() {
        let screen = [
            Pos2::new(0.0, 0.0),
            Pos2::new(2.0, 0.0),
            Pos2::new(0.0, 2.0),
        ];
        let depths = [2.0, 4.0, 8.0];
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let screen = order.map(|i| screen[i]);
            let depths = order.map(|i| depths[i]);
            assert_eq!(
                triangle_depth(Pos2::new(0.5, 0.5), screen, depths, false),
                Some(4.0)
            );
            assert!(
                (triangle_depth(Pos2::new(0.5, 0.5), screen, depths, true).unwrap() - 32.0 / 11.0)
                    .abs()
                    < 1e-14
            );
        }
        assert!(triangle_depth(Pos2::ZERO, [Pos2::ZERO; 3], depths, true).is_none());
        assert!(triangle_depth(Pos2::ZERO, screen, [-1.0, 4.0, 8.0], true).is_none());
    }
}
