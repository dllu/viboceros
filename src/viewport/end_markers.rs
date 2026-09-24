//! End Analysis locations shared by display and camera fitting.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndMarkerKind {
    Start,
    End,
    Seam,
    Joint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EndMarker {
    pub point: Point3,
    pub kind: EndMarkerKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EndMarkerOptions {
    pub starts: bool,
    pub ends: bool,
    pub seams: bool,
    pub joints: bool,
}

impl Default for EndMarkerOptions {
    fn default() -> Self {
        Self {
            starts: true,
            ends: true,
            seams: true,
            joints: true,
        }
    }
}

impl EndMarkerOptions {
    pub fn includes(self, kind: EndMarkerKind) -> bool {
        match kind {
            EndMarkerKind::Start => self.starts,
            EndMarkerKind::End => self.ends,
            EndMarkerKind::Seam => self.seams,
            EndMarkerKind::Joint => self.joints,
        }
    }
}

/// Missing/deleted sources are ignored; a failed endpoint evaluation rejects
/// the whole marker set so camera changes remain atomic.
pub(crate) fn collect_end_markers(
    document: &Document,
    sources: impl IntoIterator<Item = ObjectId>,
    options: EndMarkerOptions,
) -> Result<Vec<EndMarker>, &'static str> {
    let mut markers = Vec::new();
    for id in sources {
        let Some(object) = document.object(id) else {
            continue;
        };
        if !object.attributes().is_visible()
            || !document
                .layer(object.attributes().layer_id())
                .is_some_and(|layer| layer.is_visible())
        {
            continue;
        }
        let Some(curve) = object.geometry().curve_ref() else {
            continue;
        };
        let closed = curve
            .is_closed()
            .map_err(|_| "curve end cannot be evaluated")?;
        let mut push = |point: Point3, kind: EndMarkerKind| {
            if options.includes(kind) {
                markers.push(EndMarker { point, kind });
            }
        };
        if closed {
            push(
                curve
                    .start_point()
                    .map_err(|_| "curve end cannot be evaluated")?,
                EndMarkerKind::Seam,
            );
        } else {
            push(
                curve
                    .start_point()
                    .map_err(|_| "curve end cannot be evaluated")?,
                EndMarkerKind::Start,
            );
        }
        if let Geometry::PolyCurve(polycurve) = object.geometry() {
            for segment in polycurve.segments().iter().skip(1) {
                push(
                    segment
                        .as_ref()
                        .start_point()
                        .map_err(|_| "curve joint cannot be evaluated")?,
                    EndMarkerKind::Joint,
                );
            }
        }
        if !closed {
            push(
                curve
                    .end_point()
                    .map_err(|_| "curve end cannot be evaluated")?,
                EndMarkerKind::End,
            );
        }
    }
    Ok(markers)
}
