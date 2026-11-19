//! Plain and rich text are retained alongside the native leader plane and style.
use super::{ThreeDmError, annotation_data};
use viboceros_geometry::{Leader, Point3};

const VERSION: u32 = 1;
const MAX_POINTS: usize = 100_000;

pub(super) fn coordinates(leader: &Leader) -> Vec<f64> {
    let mut values = annotation_data::plane_coordinates(leader.plane());
    values.extend(
        leader
            .path()
            .vertices()
            .iter()
            .flat_map(|point| point.to_array()),
    );
    values
}

pub(super) fn encode(leader: &Leader) -> Vec<u8> {
    annotation_data::encode(VERSION, leader.style(), leader.text(), leader.rich_text())
}

pub(super) fn decode(coordinates: &[f64], data: &[u8]) -> Result<Leader, ThreeDmError> {
    let invalid = || ThreeDmError::MalformedBridge("invalid leader payload");
    if coordinates.len() < 15
        || !coordinates.len().is_multiple_of(3)
        || (coordinates.len() - 9) / 3 > MAX_POINTS
    {
        return Err(invalid());
    }
    let (style, plain_text, rich_text) = annotation_data::decode(data, VERSION)?;
    let point = |values: &[f64]| Point3::try_new(values[0], values[1], values[2]);
    let plane = annotation_data::decode_plane(coordinates)?;
    let path = coordinates[9..]
        .chunks_exact(3)
        .map(point)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Leader::try_styled(
        plane, path, plain_text, rich_text, style,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ThreeDmGeometry, ThreeDmModel, read_3dm_file, write_3dm_file};
    use viboceros_geometry::{CurveRef, Tolerance};

    fn leader_named<'a>(model: &'a ThreeDmModel, label: &str) -> &'a Leader {
        model
            .objects
            .iter()
            .find_map(|object| match &object.geometry {
                ThreeDmGeometry::Leader(leader) if leader.text() == label => Some(leader),
                _ => None,
            })
            .expect("styled leader in fixture")
    }

    #[test]
    fn leader_appearance_uses_native_spline_arrow_and_landing_after_round_trip() {
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/leader-appearance.3dm");
        let directory = tempfile::tempdir().unwrap();
        let saved = directory.path().join("leaders.3dm");
        let model = read_3dm_file(source, Tolerance::DEFAULT).unwrap();
        write_3dm_file(&saved, &model).unwrap();
        let reopened = read_3dm_file(&saved, Tolerance::DEFAULT).unwrap();
        for document in [&model, &reopened] {
            let spline = leader_named(document, "Spline dot");
            let Some(CurveRef::NurbsCurve(body)) = spline.body() else {
                panic!("expected a spline leader");
            };
            let domain = body.domain();
            let mid = (*domain.start() + *domain.end()) * 0.5;
            // From ON_Leader::Curve for the fixture's four control points.
            assert_eq!(body.evaluate(mid).unwrap().to_array(), [4.0, 3.0, 0.0]);
            assert_eq!(
                spline.landing().unwrap().map(Point3::to_array),
                [[8.0, 0.0, 0.0], [8.0, -1.0, 0.0]]
            );
            assert_eq!(spline.arrow_triangles().count(), 22);
            let (text, direction) = spline.text_attachment().unwrap();
            assert_eq!(text.to_array(), [8.0, -1.5, 0.0]);
            assert_eq!(direction.as_vector().to_array(), [0.0, -1.0, 0.0]);

            let text_only = leader_named(document, "Text only");
            assert!(text_only.body().is_none());
            assert!(text_only.landing().is_none());
            assert_eq!(text_only.arrow_triangles().count(), 0);

            let open = leader_named(document, "Open arrow");
            assert_eq!(open.arrow_triangles().count(), 4);
            let (_, direction) = open.text_attachment().unwrap();
            let [x, y, z] = direction.as_vector().to_array();
            assert!((x - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
            assert!((y - x).abs() < 1e-12);
            assert_eq!(z, 0.0);
        }
    }
}
