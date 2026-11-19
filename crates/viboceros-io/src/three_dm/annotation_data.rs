//! Common archive payload for native text and leader annotations.
use super::ThreeDmError;
use viboceros_geometry::{AnnotationStyle, Frame3, Point3, Tolerance, Vector3};

pub(super) fn plane_coordinates(plane: Frame3) -> Vec<f64> {
    let mut values = Vec::with_capacity(9);
    values.extend(plane.origin().to_array());
    values.extend(plane.x_axis().as_vector().to_array());
    values.extend(plane.y_axis().as_vector().to_array());
    values
}

pub(super) fn decode_plane(coordinates: &[f64]) -> Result<Frame3, ThreeDmError> {
    if coordinates.len() < 9 {
        return Err(ThreeDmError::MalformedBridge("invalid annotation plane"));
    }
    let p = coordinates;
    Ok(Frame3::try_from_directions(
        Point3::try_new(p[0], p[1], p[2])?,
        Vector3::try_new(p[3], p[4], p[5])?,
        Vector3::try_new(p[6], p[7], p[8])?,
        Tolerance::NUMERICAL_VALIDATION,
    )?)
}

pub(super) fn encode(version: u32, style: &AnnotationStyle, plain: &str, rich: &str) -> Vec<u8> {
    let archive = style.archive();
    let mut data = Vec::with_capacity(10 + archive.len() + plain.len() + rich.len());
    data.extend(version.to_le_bytes());
    data.extend((archive.len() as u32).to_le_bytes());
    data.extend(archive);
    for text in [plain, rich] {
        data.extend(text.as_bytes());
        data.push(0);
    }
    data
}

pub(super) fn decode(
    data: &[u8],
    version: u32,
) -> Result<(AnnotationStyle, &str, &str), ThreeDmError> {
    let invalid = || ThreeDmError::MalformedBridge("invalid annotation payload");
    let (header, body) = data.split_at_checked(8).ok_or_else(invalid)?;
    if u32::from_le_bytes(header[..4].try_into().unwrap()) != version {
        return Err(invalid());
    }
    let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    let (archive, strings) = body.split_at_checked(size).ok_or_else(invalid)?;
    let style = AnnotationStyle::from_archive(archive).ok_or_else(invalid)?;
    let mut strings = strings.splitn(3, |byte| *byte == 0);
    let mut text =
        || std::str::from_utf8(strings.next().ok_or_else(invalid)?).map_err(|_| invalid());
    let plain = text()?;
    let rich = text()?;
    if strings.next() != Some(&[]) {
        return Err(invalid());
    }
    Ok((style, plain, rich))
}
