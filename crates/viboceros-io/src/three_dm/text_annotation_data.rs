use super::{ThreeDmError, annotation_data};
use viboceros_geometry::{TextAnnotation, TextAnnotationLayout};

const VERSION: u32 = 1;

pub(super) fn coordinates(text: &TextAnnotation) -> Vec<f64> {
    let mut values = annotation_data::plane_coordinates(text.plane());
    let layout = text.layout();
    values.extend([
        layout.rotation,
        layout.wrapping_width.unwrap_or(0.0),
        layout.height_factor,
    ]);
    values.extend(layout.ink_bounds.into_iter().flatten());
    values
}

pub(super) fn encode(text: &TextAnnotation) -> Vec<u8> {
    annotation_data::encode(VERSION, text.style(), text.text(), text.rich_text())
}

pub(super) fn decode(values: &[f64], data: &[u8]) -> Result<TextAnnotation, ThreeDmError> {
    let [.., rotation, width, height_factor, left, bottom, right, top] = values else {
        return Err(ThreeDmError::MalformedBridge(
            "invalid model text coordinates",
        ));
    };
    if values.len() != 16 {
        return Err(ThreeDmError::MalformedBridge(
            "invalid model text coordinates",
        ));
    }
    let (style, plain, rich) = annotation_data::decode(data, VERSION)?;
    Ok(TextAnnotation::try_new(
        annotation_data::decode_plane(values)?,
        plain,
        rich,
        style,
        TextAnnotationLayout {
            rotation: *rotation,
            wrapping_width: (*width != 0.0).then_some(*width),
            height_factor: *height_factor,
            ink_bounds: [[*left, *bottom], [*right, *top]],
        },
    )?)
}

#[cfg(test)]
mod tests {
    use crate::{
        ThreeDmGeometry, ThreeDmModel, ThreeDmObject, read_3dm_file_with_blocks, write_3dm_file,
    };
    use viboceros_geometry::{TextAnnotation, Tolerance};

    fn marker(model: &ThreeDmModel) -> &TextAnnotation {
        let ThreeDmGeometry::Text(text) = &model.definitions[0].members[0].geometry else {
            panic!("expected native text in the arrowhead block");
        };
        text
    }

    #[test]
    fn model_text_survives_a_native_3dm_round_trip_inside_and_outside_blocks() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/text-leader-arrow.3dm");
        let mut source = read_3dm_file_with_blocks(fixture, Tolerance::DEFAULT).unwrap();
        let text = marker(&source).clone();
        assert_eq!(text.text(), "MARK");
        assert_eq!(text.plane().origin().to_array(), [-2., 2., 0.]);
        assert_eq!(text.layout().rotation, 0.25);
        let [[left, bottom], [right, top]] = text.layout().ink_bounds;
        assert!(right > left && top > bottom);

        let ThreeDmGeometry::Leader(leader) = &source.objects[0].geometry else {
            panic!("expected a leader referencing the text block");
        };
        let posed = text.transformed(leader.arrow_transform().unwrap()).unwrap();
        source
            .objects
            .push(ThreeDmObject::new(ThreeDmGeometry::Text(posed.clone()), 0));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("text.3dm");
        write_3dm_file(&path, &source).unwrap();
        let reopened = read_3dm_file_with_blocks(path, Tolerance::DEFAULT).unwrap();
        assert_eq!(reopened.definitions[0].id, source.definitions[0].id);
        let ThreeDmGeometry::Text(placed) = &reopened.objects[1].geometry else {
            panic!("expected the transformed top-level text");
        };
        for (expected, actual) in [(&text, marker(&reopened)), (&posed, placed)] {
            assert_eq!(actual.text(), expected.text());
            assert_eq!(actual.rich_text(), expected.rich_text());
            assert_eq!(actual.layout().rotation, expected.layout().rotation);
            for (a, b) in actual.quad().into_iter().zip(expected.quad()) {
                assert!(a.distance_to(b).unwrap() < 1e-9, "{a:?} vs {b:?}");
            }
        }
    }
}
