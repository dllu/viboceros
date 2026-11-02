//! Mouse navigation and native camera-vector reset policy.

use super::*;
use nalgebra::{Unit, UnitQuaternion};

impl Viewport {
    pub(super) fn apply_navigation_gesture(
        &mut self,
        button: PointerButton,
        modifiers: egui::Modifiers,
        delta: Vec2,
    ) {
        // Evaluate from the gesture's initial camera. In particular, a skew
        // locked-up frame changes at the first orbit; later events must keep
        // rotating about the original gesture axis.
        if let Some(start) = self.navigation_drag_start {
            self.restore_camera(start);
        }
        self.apply_navigation_drag(button, modifiers, delta);
    }

    pub(super) fn apply_navigation_drag(
        &mut self,
        button: PointerButton,
        modifiers: egui::Modifiers,
        delta: Vec2,
    ) {
        if !delta.is_finite() || delta == Vec2::ZERO {
            return;
        }
        if button == PointerButton::Middle
            || (button == PointerButton::Secondary && (self.kind.is_parallel() || modifiers.shift))
        {
            if self.kind == ViewKind::Perspective {
                let Some(rect) = self
                    .last_rect
                    .filter(|rect| rect.is_finite() && rect.is_positive())
                else {
                    return;
                };
                let (right, up, _) = self.perspective_basis();
                let scale =
                    self.perspective_camera_distance / self.perspective_focal_length_pixels(rect);
                let target = self.target - right * (Real::from(delta.x) * scale)
                    + up * (Real::from(delta.y) * scale);
                if target.iter().all(|value| value.is_finite()) {
                    self.target = target;
                }
            } else {
                let pan = self.pan + delta;
                if pan.is_finite() {
                    self.pan = pan;
                }
            }
        } else if button == PointerButton::Secondary && self.two_point_perspective {
            let Some(rect) = self
                .last_rect
                .filter(|rect| rect.is_finite() && rect.is_positive())
            else {
                return;
            };
            let (_, up, forward) = self.perspective_basis();
            // Six owned Rhino 8 drags calibrate yaw to 0.3 degrees/pixel and
            // vertical travel to eight target-plane pixels per mouse pixel.
            let yaw = -Real::from(delta.x) * std::f64::consts::PI / 600.0;
            let vertical = 8.0 * Real::from(delta.y) * self.perspective_camera_distance
                / self.perspective_focal_length_pixels(rect);
            let target = self.target + up * vertical;
            let shift = self.perspective_lens_shift[1]
                - 16.0 * Real::from(delta.y) / Real::from(rect.height().max(1.0));
            if !target.iter().all(|value| value.is_finite()) || !shift.is_finite() {
                return;
            }
            if self.perspective_frame.is_some() {
                let rotation = UnitQuaternion::from_axis_angle(&Unit::new_normalize(up), yaw);
                // Rhino normalizes the direction at the first orbit, while
                // the locked raw up vector remains unchanged. Rebuilding the
                // frame gives that up vector priority, including skew inputs.
                let direction = rotation.transform_vector(
                    &NaVector3::from(self.camera_direction().to_array()).normalize(),
                );
                let locked_up = self.camera_up_hint.unwrap_or(up).normalize();
                let right = direction.cross(&locked_up).normalize();
                let Ok(frame) = Frame3::try_from_directions(
                    Point3::try_from([target.x, target.y, target.z]).expect("finite target"),
                    Vector3::try_from([right.x, right.y, right.z]).expect("finite camera axis"),
                    Vector3::try_from([locked_up.x, locked_up.y, locked_up.z])
                        .expect("finite camera axis"),
                    Tolerance::DEFAULT,
                ) else {
                    return;
                };
                // The locked-up frame may no longer point along the stored
                // direction. Preserve the rotated camera location and retain
                // its target independently of the orthonormal viewing axis.
                let location = target
                    + rotation.transform_vector(&(-forward * self.perspective_camera_distance));
                let new_forward = -NaVector3::from(frame.z_axis().as_vector().to_array());
                let distance = (target - location).dot(&new_forward);
                let axis_target = location + new_forward * distance;
                if !distance.is_finite()
                    || distance <= 0.0
                    || !axis_target.iter().all(|x| x.is_finite())
                {
                    return;
                }
                self.perspective_frame = Some(frame);
                self.camera_direction_hint = Some(direction);
                self.perspective_camera_distance = distance;
                self.camera_target_offset = target - axis_target;
                self.target = axis_target;
            } else {
                self.orbit_yaw += yaw;
                self.target = target;
                self.camera_target_offset = NaVector3::zeros();
            }
            self.perspective_lens_shift[1] = shift;
            self.cplane_direction = None;
        } else if button == PointerButton::Secondary {
            if let Some(frame) = self.perspective_frame {
                let (right, up, _) = self.perspective_basis();
                let yaw = UnitQuaternion::from_axis_angle(
                    &NaVector3::z_axis(),
                    -Real::from(delta.x) * std::f64::consts::PI / 600.0,
                );
                let pitch = UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(yaw.transform_vector(&right)),
                    -Real::from(delta.y) * std::f64::consts::PI / 600.0,
                );
                let rotation = pitch * yaw;
                let new_right = rotation.transform_vector(&right);
                let new_up = rotation.transform_vector(&up);
                if let Ok(camera_frame) = Frame3::try_from_directions(
                    frame.origin(),
                    Vector3::try_from([new_right.x, new_right.y, new_right.z])
                        .expect("finite camera axis"),
                    Vector3::try_from([new_up.x, new_up.y, new_up.z]).expect("finite camera axis"),
                    Tolerance::DEFAULT,
                ) {
                    self.perspective_frame = Some(camera_frame);
                    self.camera_up_hint = None;
                    self.camera_direction_hint = None;
                    self.camera_target_offset = NaVector3::zeros();
                    self.cplane_direction = None;
                }
            } else {
                self.orbit_yaw -= Real::from(delta.x) * std::f64::consts::PI / 600.0;
                self.orbit_pitch = (self.orbit_pitch
                    + Real::from(delta.y) * std::f64::consts::PI / 600.0)
                    .clamp(-1.553_343_034_274_953_2, 1.553_343_034_274_953_2);
            }
        }
    }
}
