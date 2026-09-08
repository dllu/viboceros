//! Per-frame GPU scene staging, object display dispatch, and depth encoding.

use super::*;
use std::collections::HashMap;
use viboceros_document::ColorRgb;

const SMOOTH_SHADING_COSINE: Real = std::f64::consts::FRAC_1_SQRT_2;

fn vector_to_gpu(vector: NaVector3<Real>) -> [f32; 3] {
    [vector.x as f32, vector.y as f32, vector.z as f32]
}

fn color_to_gpu(color: Color32) -> [f32; 4] {
    color
        .to_srgba_unmultiplied()
        .map(|component| f32::from(component) / 255.0)
}

fn color_with_alpha(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn resolved_display_color(attributes: &ObjectAttributes, layer_color: ColorRgb) -> Color32 {
    let color = attributes.display_color(layer_color);
    Color32::from_rgb(color.red, color.green, color.blue)
}

fn smooth_corner_normals(mesh: &TriangleMesh) -> Vec<[NaVector3<Real>; 3]> {
    let fallback = NaVector3::new(0.0, 0.0, 1.0);
    let face_normals = (0..mesh.triangles().len())
        .map(|index| {
            mesh.face_normal(index)
                .map(|normal| NaVector3::new(normal.x(), normal.y(), normal.z()))
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();

    // Surface tessellation intentionally duplicates vertices at knot-span
    // boundaries. Group exact coincident samples so continuous spans shade as
    // one surface, then use the crease angle below to keep analytic caps and
    // other genuinely sharp joins hard.
    let mut incident_faces: HashMap<[u64; 3], Vec<usize>> = HashMap::new();
    for (face_index, triangle) in mesh.triangles().iter().enumerate() {
        for &vertex_index in triangle {
            let point = mesh.vertices()[vertex_index as usize];
            incident_faces
                .entry(point_position_key(point))
                .or_default()
                .push(face_index);
        }
    }

    mesh.triangles()
        .iter()
        .enumerate()
        .map(|(face_index, triangle)| {
            let reference = face_normals[face_index];
            triangle.map(|vertex_index| {
                let point = mesh.vertices()[vertex_index as usize];
                let mut sum = NaVector3::zeros();
                for &incident in &incident_faces[&point_position_key(point)] {
                    let candidate = face_normals[incident];
                    if reference.dot(&candidate) >= SMOOTH_SHADING_COSINE {
                        sum += candidate;
                    }
                }
                sum.try_normalize(Real::EPSILON).unwrap_or(reference)
            })
        })
        .collect()
}

fn point_position_key(point: Point3) -> [u64; 3] {
    [point.x(), point.y(), point.z()].map(|value| {
        if value == 0.0 {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }
    })
}

#[derive(Clone, Copy, Debug)]
struct SurfaceDisplayStyle {
    color: Color32,
    width: f32,
    wire_density: i32,
}

#[derive(Clone, Copy)]
pub(super) struct DepthTriangle {
    pub(super) depth: Real,
    vertex_depths: [Real; 3],
    pub(super) vertices: [GpuTriangleVertex; 3],
}

pub(super) struct DepthPrimitive<T, const N: usize> {
    depths: [Real; N],
    instance: T,
}

pub(super) struct GpuSceneBuilder {
    pub(super) triangles: Vec<DepthTriangle>,
    pub(super) lines: Vec<DepthPrimitive<GpuLineInstance, 2>>,
    pub(super) points: Vec<DepthPrimitive<GpuPointInstance, 1>>,
    pub(super) min_depth: Real,
    max_depth: Real,
}

impl GpuSceneBuilder {
    pub(super) fn new() -> Self {
        Self {
            triangles: Vec::new(),
            lines: Vec::new(),
            points: Vec::new(),
            min_depth: Real::INFINITY,
            max_depth: Real::NEG_INFINITY,
        }
    }

    fn include_depth(&mut self, depth: Real) {
        if depth.is_finite() {
            self.min_depth = self.min_depth.min(depth);
            self.max_depth = self.max_depth.max(depth);
        }
    }

    pub(super) fn depth_range(&self) -> Option<(Real, Real)> {
        (self.min_depth.is_finite() && self.max_depth.is_finite())
            .then_some((self.min_depth, self.max_depth))
    }

    pub(super) fn finish(
        mut self,
        viewport: &Viewport,
        rect: Rect,
        transparent: bool,
    ) -> GpuViewportScene {
        let range = self.depth_range();
        let uniform = viewport.gpu_view_uniform(rect, range);
        if transparent {
            self.triangles
                .sort_by(|left, right| right.depth.total_cmp(&left.depth));
        }
        GpuViewportScene {
            uniform,
            triangles: self
                .triangles
                .into_iter()
                .flat_map(|mut triangle| {
                    for (vertex, depth) in triangle.vertices.iter_mut().zip(triangle.vertex_depths)
                    {
                        viewport.encode_gpu_depth(&mut vertex.position, depth, range);
                    }
                    triangle.vertices
                })
                .collect(),
            lines: self
                .lines
                .into_iter()
                .map(|mut line| {
                    viewport.encode_gpu_depth(
                        &mut line.instance.start_width,
                        line.depths[0],
                        range,
                    );
                    viewport.encode_gpu_depth(
                        &mut line.instance.end_padding,
                        line.depths[1],
                        range,
                    );
                    line.instance
                })
                .collect(),
            points: self
                .points
                .into_iter()
                .map(|mut point| {
                    viewport.encode_gpu_depth(
                        &mut point.instance.position_size,
                        point.depths[0],
                        range,
                    );
                    point.instance
                })
                .collect(),
            transparent,
        }
    }
}

impl Viewport {
    pub(super) fn paint_objects(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        viewport_index: usize,
    ) {
        let mut scene = GpuSceneBuilder::new();
        for object in document.objects() {
            let attributes = object.attributes();
            let Some(layer) = document.layer(attributes.layer_id()) else {
                continue;
            };
            if !attributes.is_visible() || !layer.is_visible() {
                continue;
            }

            let display_color = resolved_display_color(attributes, layer.color());
            let mut color = if attributes.is_locked() || layer.is_locked() {
                LOCKED_COLOR
            } else {
                display_color
            };
            if self.display_mode == DisplayMode::Ghosted {
                color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
            }
            let selected = document.is_selected(object.id());
            if selected {
                color = SELECTED_COLOR;
            }
            let mut width = match self.display_mode {
                DisplayMode::Wireframe => 1.5,
                DisplayMode::Shaded => 2.25,
                DisplayMode::Ghosted => 1.25,
            };
            if selected {
                width += 1.5;
            }

            match object.geometry() {
                Geometry::Point(point) => {
                    self.add_gpu_point(&mut scene, rect, *point, 4.5, color);
                }
                Geometry::PointCloud(cloud) => {
                    let radius = if selected { 3.5 } else { 2.5 };
                    for point in cloud.points() {
                        self.add_gpu_point(&mut scene, rect, *point, radius, color);
                    }
                }
                Geometry::Line(line) => {
                    self.add_gpu_line(&mut scene, rect, line.start(), line.end(), width, color);
                }
                Geometry::Circle(circle) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        CIRCLE_SAMPLES,
                        width,
                        color,
                        |parameter| circle.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Arc(arc) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        circular_arc_samples(*arc),
                        width,
                        color,
                        |parameter| arc.point_at(parameter),
                    );
                }
                Geometry::Ellipse(ellipse) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        CIRCLE_SAMPLES,
                        width,
                        color,
                        |parameter| ellipse.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Polyline(polyline) => {
                    for segment in polyline.segments() {
                        self.add_gpu_line(
                            &mut scene,
                            rect,
                            segment.start(),
                            segment.end(),
                            width,
                            color,
                        );
                    }
                }
                Geometry::NurbsCurve(curve) => {
                    self.add_gpu_nurbs_curve(&mut scene, rect, curve, width, color);
                }
                Geometry::PolyCurve(curve) => {
                    for segment in curve.segments() {
                        self.add_gpu_nurbs_curve(&mut scene, rect, segment, width, color);
                    }
                }
                Geometry::NurbsSurface(surface) => {
                    self.add_gpu_nurbs_surface(
                        &mut scene,
                        rect,
                        surface,
                        SurfaceDisplayStyle {
                            color,
                            width,
                            wire_density: attributes.wire_density(),
                        },
                        document.tolerance(),
                    );
                }
                Geometry::Brep(brep) => {
                    self.add_gpu_brep(
                        &mut scene,
                        rect,
                        brep,
                        SurfaceDisplayStyle {
                            color,
                            width,
                            wire_density: attributes.wire_density(),
                        },
                        document.tolerance(),
                    );
                }
                Geometry::Mesh(mesh) => {
                    self.add_gpu_mesh(&mut scene, rect, mesh, color, width, document.tolerance());
                }
            }
        }

        let transparent = self.display_mode == DisplayMode::Ghosted;
        crate::viewport_gpu::paint(
            painter,
            rect,
            viewport_index,
            scene.finish(self, rect, transparent),
        );
    }

    pub(super) fn add_gpu_point(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        point: Point3,
        radius: f32,
        color: Color32,
    ) {
        let Some(projected) = self.project(point, rect) else {
            return;
        };
        if !rect.expand(radius).contains(projected) {
            return;
        }
        let Some(position) = self.gpu_position(point) else {
            return;
        };
        let depth = self.view_depth(point);
        scene.include_depth(depth);
        scene.points.push(DepthPrimitive {
            depths: [depth],
            instance: GpuPointInstance {
                position_size: [position[0], position[1], position[2], radius],
                color: color_to_gpu(color),
            },
        });
    }

    pub(super) fn add_gpu_line(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        start: Point3,
        end: Point3,
        width: f32,
        color: Color32,
    ) {
        let Some([start, end]) = self.clip_segment(start, end) else {
            return;
        };
        if self.project(start, rect).is_none() || self.project(end, rect).is_none() {
            return;
        }
        let (Some(start_position), Some(end_position)) =
            (self.gpu_position(start), self.gpu_position(end))
        else {
            return;
        };
        let depths = [self.view_depth(start), self.view_depth(end)];
        for depth in depths {
            scene.include_depth(depth);
        }
        scene.lines.push(DepthPrimitive {
            depths,
            instance: GpuLineInstance {
                start_width: [
                    start_position[0],
                    start_position[1],
                    start_position[2],
                    width,
                ],
                end_padding: [end_position[0], end_position[1], end_position[2], 0.0],
                color: color_to_gpu(color),
            },
        });
    }

    fn add_gpu_nurbs_curve(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        curve: &impl ViewportCurve,
        width: f32,
        color: Color32,
    ) {
        let domain_end = *curve.domain().end();
        let samples = curve.samples_per_span();
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=samples {
                let fraction = sample as Real / samples as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                // At a fully multiple interior knot, the curve has distinct
                // left and right limits. Keep this span on its left side and
                // begin the next polyline separately on the right side.
                if sample == samples && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }

                let evaluated = curve.evaluate(parameter).ok();
                if let (Some(start), Some(end)) = (previous, evaluated) {
                    self.add_gpu_line(scene, rect, start, end, width, color);
                }
                previous = evaluated;
            }
        }
    }

    fn add_gpu_parametric_curve(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        samples: usize,
        width: f32,
        color: Color32,
        mut evaluate: impl FnMut(Real) -> Result<Point3, viboceros_geometry::GeometryError>,
    ) {
        let mut previous = None;
        for sample in 0..=samples {
            let evaluated = evaluate(sample as Real / samples as Real).ok();
            if let (Some(start), Some(end)) = (previous, evaluated) {
                self.add_gpu_line(scene, rect, start, end, width, color);
            }
            previous = evaluated;
        }
    }

    fn add_gpu_nurbs_surface(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        surface: &NurbsSurface,
        style: SurfaceDisplayStyle,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            self.add_gpu_mesh_faces(scene, &mesh, style.color);
        }

        if let Ok(curves) = surface.wireframe_curves(style.wire_density) {
            for curve in &curves {
                self.add_gpu_nurbs_curve(scene, rect, curve, style.width, style.color);
            }
        }
    }

    fn add_gpu_brep(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        brep: &Brep,
        style: SurfaceDisplayStyle,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            self.add_gpu_mesh_faces(scene, &mesh, style.color);
        }
        if let Ok(curves) = brep.wireframe_curves(style.wire_density, tolerance) {
            for curve in &curves {
                self.add_gpu_nurbs_curve(scene, rect, curve, style.width, style.color);
            }
        }
    }

    fn add_gpu_mesh(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        mesh: &TriangleMesh,
        color: Color32,
        width: f32,
        tolerance: Tolerance,
    ) {
        self.add_gpu_mesh_faces(scene, mesh, color);
        if let Ok(lines) = mesh.wireframe_lines(tolerance) {
            for line in lines {
                self.add_gpu_line(scene, rect, line.start(), line.end(), width, color);
            }
        }
    }

    pub(super) fn add_gpu_mesh_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        mesh: &TriangleMesh,
        color: Color32,
    ) {
        if self.display_mode == DisplayMode::Wireframe {
            return;
        }

        let corner_normals = smooth_corner_normals(mesh);
        let face_color = if self.display_mode == DisplayMode::Ghosted {
            color_with_alpha(color, 35)
        } else {
            color
        };
        let gpu_color = color_to_gpu(face_color);
        for (triangle_index, normals) in corner_normals.into_iter().enumerate() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
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
            let normals = normals.map(vector_to_gpu);
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
                        color: gpu_color,
                    },
                    GpuTriangleVertex {
                        position: second,
                        normal: normals[1],
                        color: gpu_color,
                    },
                    GpuTriangleVertex {
                        position: third,
                        normal: normals[2],
                        color: gpu_color,
                    },
                ],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn smooth_shading_keeps_ninety_degree_mesh_edges_hard() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 1, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let normals = smooth_corner_normals(&mesh);
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].z, 1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].y, 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].y, -1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].z, 0.0));
    }

    #[test]
    fn object_color_overrides_layer_color_only_for_object_source() {
        let object = ColorRgb::new(12, 34, 56);
        let layer = ColorRgb::new(78, 90, 123);
        let document = Document::default();
        let base =
            ObjectAttributes::on_layer(document.current_layer_id()).with_object_color(object);
        assert_eq!(
            resolved_display_color(&base, layer),
            Color32::from_rgb(12, 34, 56)
        );
        for source in [
            viboceros_document::ObjectColorSource::Layer,
            viboceros_document::ObjectColorSource::Material,
            viboceros_document::ObjectColorSource::Parent,
        ] {
            assert_eq!(
                resolved_display_color(&base.clone().with_color_source(source), layer),
                Color32::from_rgb(78, 90, 123)
            );
        }
    }
}
