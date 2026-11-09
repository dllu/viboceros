//! Authored matching policies shared by one-sided and averaging comparisons.
use viboceros_geometry::*;
pub fn side(s: &str) -> SurfaceExtensionEdge {
    match s {
        "West" => SurfaceExtensionEdge::West,
        "South" => SurfaceExtensionEdge::South,
        "East" => SurfaceExtensionEdge::East,
        "North" => SurfaceExtensionEdge::North,
        _ => panic!(),
    }
}
pub fn options(v: &serde_json::Value) -> SurfaceMatchOptions {
    SurfaceMatchOptions {
        continuity: match v["continuity"].as_str().unwrap() {
            "Position" => SurfaceBlendContinuity::Position,
            "Tangency" => SurfaceBlendContinuity::Tangency,
            "Curvature" => SurfaceBlendContinuity::Curvature,
            _ => panic!(),
        },
        other_end: match v["other_end"].as_str().unwrap() {
            "None" => CurveMatchPreserveEnd::None,
            "Position" => CurveMatchPreserveEnd::Position,
            "Tangency" => CurveMatchPreserveEnd::Tangency,
            "Curvature" => CurveMatchPreserveEnd::Curvature,
            _ => panic!(),
        },
        iso: match v["iso"].as_str().unwrap() {
            "Automatic" => SurfaceMatchIso::Automatic,
            "MatchTarget" => SurfaceMatchIso::MatchTarget,
            "Perpendicular" => SurfaceMatchIso::Perpendicular,
            "Preserve" => SurfaceMatchIso::Preserve,
            _ => panic!(),
        },
        reverse_match: v["reverse_match"].as_bool().unwrap(),
        closest_points: v["closest_points"].as_bool().unwrap(),
        refine: v["refine"].as_bool().unwrap(),
    }
}
