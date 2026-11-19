//! Versioned annotation data shared with the OpenNURBS 3DM bridge.
use super::ThreeDmError;
use viboceros_geometry::{Point3, TextDot, TextDotStyle};

const VERSION: u32 = 1;
const TOP: u8 = 1;
const TRANSPARENT: u8 = 2;
const BOLD: u8 = 4;
const ITALIC: u8 = 8;

pub(super) fn encode(dot: &TextDot) -> Vec<u8> {
    let style = dot.style();
    let mut data = Vec::new();
    data.extend(VERSION.to_le_bytes());
    data.extend(style.height_points.to_le_bytes());
    data.push(
        u8::from(style.always_on_top) * TOP
            | u8::from(style.transparent) * TRANSPARENT
            | u8::from(style.bold) * BOLD
            | u8::from(style.italic) * ITALIC,
    );
    for text in [dot.text(), dot.secondary_text(), &style.font_face] {
        data.extend(text.as_bytes());
        data.push(0);
    }
    data
}

pub(super) fn decode(point: Point3, data: &[u8]) -> Result<TextDot, ThreeDmError> {
    let invalid = || ThreeDmError::MalformedBridge("invalid text dot payload");
    let (header, strings) = data.split_at_checked(9).ok_or_else(invalid)?;
    if u32::from_le_bytes(header[..4].try_into().unwrap()) != VERSION || header[8] & 0xf0 != 0 {
        return Err(invalid());
    }
    let mut fields = strings.splitn(4, |byte| *byte == 0);
    let mut text = || {
        let bytes = fields.next().ok_or_else(invalid)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid())
    };
    let primary = text()?;
    let secondary = text()?;
    let font_face = text()?;
    if fields.next() != Some(&[]) {
        return Err(invalid());
    }
    let flags = header[8];
    let style = TextDotStyle {
        font_face,
        height_points: i32::from_le_bytes(header[4..8].try_into().unwrap()),
        always_on_top: flags & TOP != 0,
        transparent: flags & TRANSPARENT != 0,
        bold: flags & BOLD != 0,
        italic: flags & ITALIC != 0,
    };
    Ok(TextDot::try_styled(point, primary, secondary, style)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file,
        read_3dm_file_in_units, write_3dm_file,
    };
    use viboceros_geometry::{LengthUnitSystem, Tolerance};

    #[test]
    fn text_dot_round_trips_through_opennurbs_and_scales_only_the_anchor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("dots.3dm");
        let dot = TextDot::try_styled(
            Point3::try_new(1000., -2000., 3000.).unwrap(),
            "  点 μ\rXYZ  ".into(),
            "  詳細 🚲\r\nCoordinates  ".into(),
            TextDotStyle {
                font_face: "DejaVu Sans".into(),
                height_points: 24,
                always_on_top: false,
                transparent: true,
                bold: true,
                italic: true,
            },
        )
        .unwrap();
        assert_eq!(dot.text(), "点 μ\nXYZ");
        assert_eq!(dot.secondary_text(), "詳細 🚲\nCoordinates");
        let mut model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Annotations".into(),
                color: [23, 68, 140],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(ThreeDmGeometry::TextDot(dot.clone()), 0)],
        );
        model.units = LengthUnitSystem::Millimeters;
        write_3dm_file(&path, &model).unwrap();
        let result = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(result.unsupported_object_count(), 0);
        assert_eq!(result.objects, model.objects);
        assert_eq!(result.layers, model.layers);

        let in_meters =
            read_3dm_file_in_units(&path, &LengthUnitSystem::Meters, Tolerance::DEFAULT).unwrap();
        let expected = dot.with_point(Point3::try_new(1., -2., 3.).unwrap());
        assert_eq!(
            in_meters.objects[0].geometry,
            ThreeDmGeometry::TextDot(expected)
        );
    }
}
