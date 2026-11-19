//! Live cutting input and plane guides paired with solved intersections.
use super::*;
use viboceros_geometry::{BoundingBox3, UnitVector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CutDraft {
    Section {
        start: Point3,
        extend: bool,
    },
    Contour {
        base: Point3,
        direction: Option<Point3>,
        spacing_start: Option<Point3>,
        spacing: Real,
        range: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CutPlan {
    Section {
        frame: Frame3,
        vertical: UnitVector3,
        length: Real,
        extend: bool,
    },
    Contour(Vec<Frame3>),
}

impl CutPlan {
    pub(crate) fn is_empty(&self) -> bool {
        matches!(self, Self::Contour(frames) if frames.is_empty())
    }

    pub(crate) fn shares_reference(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Section { vertical: a, .. }, Self::Section { vertical: b, .. }) => a == b,
            (Self::Contour(_), Self::Contour(_)) => true,
            _ => false,
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct CutResult {
    pub lines: Vec<[Point3; 2]>,
    pub curves: Vec<viboceros_geometry::NurbsCurve>,
    pub points: Vec<Point3>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CuttingPreview<'a> {
    pub draft: CutDraft,
    pub bounds: &'a [BoundingBox3],
    pub extent: BoundingBox3,
    pub result: Option<(&'a CutPlan, &'a CutResult)>,
}

impl CutDraft {
    fn plan(
        self,
        cursor: Point3,
        cplane: Frame3,
        preview: CuttingPreview<'_>,
        tolerance: Tolerance,
    ) -> Option<CutPlan> {
        Some(match self {
            Self::Section { start, extend } => {
                let (frame, length) =
                    viboceros_command::section::section_frame(start, cursor, cplane, tolerance)
                        .ok()?;
                CutPlan::Section {
                    frame,
                    vertical: cplane.z_axis(),
                    length,
                    extend,
                }
            }
            Self::Contour {
                base,
                direction,
                spacing_start,
                spacing,
                range,
            } => {
                let spacing = match spacing_start {
                    Some(start) => start.distance_to(cursor).ok()?,
                    None => spacing,
                };
                const PREVIEW_PLANE_LIMIT: usize = 12;
                CutPlan::Contour(
                    viboceros_command::contour::preview_frames(
                        preview.bounds,
                        base,
                        direction.unwrap_or(cursor),
                        spacing,
                        range,
                        tolerance,
                        PREVIEW_PLANE_LIMIT,
                    )
                    .ok()?,
                )
            }
        })
    }
}

impl Viewport {
    pub(super) fn paint_cutting_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: CuttingPreview<'_>,
        cursor: Point3,
        tolerance: Tolerance,
    ) -> Option<CutPlan> {
        let plan = preview
            .draft
            .plan(cursor, self.construction_plane(), preview, tolerance)?;
        let guides = Stroke::new(1.25, Color32::from_rgb(83, 138, 149));
        let draw = |[start, end]: [Point3; 2], stroke| {
            if let Some(projected) = self.project_segment(start, end, rect) {
                painter.line_segment(projected, stroke);
            }
        };
        let solved = preview
            .result
            .filter(|(solved, _)| !plan.is_empty() && solved.shares_reference(&plan));
        let guide_plan = solved.map_or(&plan, |(solved, _)| solved);
        match guide_plan {
            CutPlan::Section {
                frame,
                length,
                extend,
                ..
            } => {
                if let Some(corners) =
                    plane_outline(*frame, preview.extent, (!extend).then_some(*length))
                {
                    for i in 0..4 {
                        draw([corners[i], corners[(i + 1) % 4]], guides);
                    }
                }
            }
            CutPlan::Contour(frames) => {
                for frame in frames {
                    if let Some(corners) = plane_outline(*frame, preview.extent, None) {
                        for i in 0..4 {
                            draw([corners[i], corners[(i + 1) % 4]], guides);
                        }
                    }
                }
            }
        }
        if let CutPlan::Section { frame, length, .. } = &plan {
            if let Ok(end) = frame.point_at([*length, 0., 0.]) {
                draw([frame.origin(), end], guides);
            }
        } else if let CutDraft::Contour {
            base,
            direction,
            spacing_start,
            ..
        } = preview.draft
            && let Some(start) = spacing_start.or(direction.is_none().then_some(base))
        {
            draw([start, cursor], guides);
        }
        if let Some((_, result)) = solved {
            let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
            for &line in &result.lines {
                draw(line, stroke);
            }
            let mut projected = ProjectedPrimitives::default();
            for curve in &result.curves {
                self.add_projected_nurbs_curve(&mut projected, rect, curve);
            }
            for segment in projected.segments {
                painter.line_segment(segment, stroke);
            }
            self.paint_draft_points(painter, rect, &result.points);
        }
        Some(plan)
    }
}

fn plane_outline(frame: Frame3, bounds: BoundingBox3, length: Option<Real>) -> Option<[Point3; 4]> {
    let min = bounds.min().to_array();
    let max = bounds.max().to_array();
    let mut lo = [Real::INFINITY; 2];
    let mut hi = [Real::NEG_INFINITY; 2];
    for mask in 0..8 {
        let corner = Point3::try_from(std::array::from_fn(|axis| {
            if mask & (1 << axis) == 0 {
                min[axis]
            } else {
                max[axis]
            }
        }))
        .ok()?;
        let [x, y, _] = frame.coordinates_of(corner).ok()?;
        for (axis, value) in [x, y].into_iter().enumerate() {
            lo[axis] = lo[axis].min(value);
            hi[axis] = hi[axis].max(value);
        }
    }
    let pad = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.04).max(0.25);
    let [x0, x1] = length.map_or([lo[0] - pad, hi[0] + pad], |length| [0., length]);
    let [y0, y1] = [lo[1] - pad, hi[1] + pad];
    Some([
        frame.point_at([x0, y0, 0.]).ok()?,
        frame.point_at([x1, y0, 0.]).ok()?,
        frame.point_at([x1, y1, 0.]).ok()?,
        frame.point_at([x0, y1, 0.]).ok()?,
    ])
}
