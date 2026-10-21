//! Guide identity, selection and source freshness for ellipse-owned construction.
use super::*;
impl VibocerosApp {
    pub(in crate::app) fn release_ellipse_picks(&mut self) {
        if let Some(s) = &self.ellipse_session
            && s.postselected
            && let Some(curve) = &s.curve
            && self.document.is_selected(curve.object_id())
        {
            let _ = self
                .document
                .select_object(curve.object_id(), SelectionMode::Remove);
        }
    }
    pub(in crate::app) fn picking_ellipse_curve(&self) -> bool {
        self.ellipse_session
            .as_ref()
            .is_some_and(|s| s.input.requests_curve() && s.curve.is_none())
    }
    pub(super) fn bind_ellipse_preselection(&mut self) {
        if !self.picking_ellipse_curve() {
            return;
        }
        let selected = self.document.selected_object_ids().collect::<Vec<_>>();
        if let [id] = selected.as_slice()
            && self
                .document
                .object(*id)
                .is_some_and(|o| o.geometry().curve_ref().is_some())
            && let Ok(curve) = CapturedCurve::capture(&self.document, *id, None)
        {
            self.ellipse_session.as_mut().unwrap().curve = Some(curve);
        }
    }
    pub(in crate::app) fn pick_ellipse_curve(
        &mut self,
        id: Option<ObjectId>,
        edge: Option<usize>,
    ) -> bool {
        if !self.picking_ellipse_curve() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a curve or surface edge".to_owned())
            .and_then(|id| {
                CapturedCurve::capture(&self.document, id, edge).map_err(|e| e.to_string())
            });
        match result {
            Ok(curve) => {
                let id = curve.object_id();
                let postselected = edge.is_none() && !self.document.is_selected(id);
                let s = self.ellipse_session.as_mut().unwrap();
                s.curve = Some(curve);
                s.postselected = postselected;
                if edge.is_none() {
                    self.document
                        .select_object(id, SelectionMode::Add)
                        .expect("selectable Ellipse guide");
                }
                self.component_selection.clear_choices();
                self.ellipse_prompt();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
    pub(in crate::app) fn validate_ellipse_curve(&mut self) {
        if self
            .ellipse_session
            .as_ref()
            .is_some_and(|s| s.subd_ellipsoid)
        {
            if !self.valid_subd_ellipsoid() {
                return;
            }
            if self
                .ellipse_session
                .as_ref()
                .and_then(|s| s.curve.as_ref())
                .is_some_and(|c| !c.current(&self.document))
            {
                self.cancel_interactive_command(false);
                self.push_log("SubDEllipsoid guide changed; restart the command".into());
                return;
            }
        }

        if self
            .ellipse_session
            .as_ref()
            .and_then(|s| s.curve.as_ref())
            .is_some_and(|c| !c.current(&self.document))
        {
            self.release_ellipse_picks();
            let s = self.ellipse_session.as_mut().unwrap();
            s.curve = None;
            s.cursor = None;
            s.postselected = false;
            let deformable = s.input.deformable();
            s.input = Input::new(
                s.input.options(),
                self.viewports[self.active_viewport].construction_plane(),
                self.document.tolerance(),
            );
            if deformable {
                s.input.option("Deformable").expect("Ellipse output option");
            }
            s.input.option("AroundCurve").expect("first Ellipse option");
            self.point_filter = None;
            self.point_constraint = None;
            self.component_selection.clear_choices();
            self.push_log("Ellipse guide changed or became unavailable; select it again".into());
            self.ellipse_prompt();
        }
    }
    pub(in crate::app) fn resolve_ellipse_point(
        &mut self,
        point: Point3,
    ) -> Result<Point3, viboceros_geometry::GeometryError> {
        self.validate_ellipse_curve();
        let Some(s) = &self.ellipse_session else {
            return Ok(point);
        };
        if let Some(curve) = &s.curve
            && s.input.requests_curve()
        {
            return viboceros_command::curve_reference::perpendicular_frame(
                curve.curve_ref(),
                point,
            )
            .map(|frame| frame.origin());
        }
        s.input.resolve_point(point)
    }
    pub(in crate::app) fn accept_ellipse_parameter(&mut self, parameter: f64) -> bool {
        let point = self
            .ellipse_session
            .as_ref()
            .and_then(Session::cursor_curve)
            .and_then(|c| c.evaluate(parameter).ok());
        if let Some(point) = point {
            self.accept_filtered_drafting_point(point, false);
            true
        } else {
            false
        }
    }
}
