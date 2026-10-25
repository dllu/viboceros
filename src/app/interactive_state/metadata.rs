//! Command names and drafting anchors.
use super::*;

impl InteractiveCommand {
    pub(in crate::app) const fn name(self) -> &'static str {
        match self {
            Self::FaceCreate { .. } => "3DFace",
            Self::LoftPoint => "Loft",
            Self::RemapCPlane => "RemapCPlane",
            Self::PolylineOnMesh { .. } => "PolylineOnMesh",
            Self::InterpolateCurveOnSurface { .. } => "InterpCrvOnSrf",
            Self::OrientOnCurve => "OrientOnCrv",
            Self::OrientCurveToEdge => "OrientCrvToEdge",
            Self::OffsetCurveOnSurface => "OffsetCrvOnSrf",
            Self::ShortPath { .. } => "ShortPath",
            Self::ArrayCurveOnSurface => "ArrayCrvOnSrf",
            Self::CurveArray { .. } => "ArrayCrv",
            Self::SurfaceArray { .. } => "ArraySrf",
            Self::SurfaceFlow { .. } => "FlowAlongSrf",
            Self::Flow { .. } => "Flow",
            Self::OrientOnSurface { .. } => "OrientOnSrf",
            Self::Splop { .. } => "Splop",
            Self::OrientPoints { kind, .. } => kind.name(),
            Self::ProjectDirection { .. } => "Project",
            Self::ProjectPlane { command, .. } => command,
            Self::Section { .. } => "Section",
            Self::Contour { .. } => "Contour",
            Self::BlockEditAdd | Self::BlockEditBasePoint => "BlockEdit",
            Self::AddObjectsToBlock => "AddObjectsToBlock",
            Self::CreateUniqueBlock => "CreateUniqueBlock",
            Self::ReplaceBlock => "ReplaceBlock",
            Self::Block { .. } => "Block",
            Self::Insert { .. } => "Insert",
            Self::Angle { .. } => "Angle",
            Self::Point => "Point",
            Self::EvaluatePoint => "EvaluatePt",
            Self::EvaluateUv { .. } => "EvaluateUVPt",
            Self::DomainFace => "Domain",
            Self::DomainSubCrv { .. } => "Domain",
            Self::LengthSubCrv { .. } => "Length",
            Self::Align { .. } => "Align",
            Self::Points => "Points",
            Self::Line { .. } => "Line",
            Self::Distance { .. } => "Distance",
            Self::Circle { .. } => "Circle",
            Self::ArcTangent { .. }
            | Self::ArcTilted { .. }
            | Self::ArcExtension { .. }
            | Self::ArcDeform { .. } => "Arc",
            Self::CircleTangent { .. } | Self::CircleAround { .. } | Self::CircleDeform { .. } => {
                "Circle"
            }
            Self::CircleSize { .. } => "Circle",
            Self::CircleVertical { .. } => "Circle",
            Self::CircleOrientation { .. } => "Circle",
            Self::CircleTwoPoint { .. } => "Circle",
            Self::CircleThreePoint { .. } => "Circle",
            Self::CircleThreePointRadius { .. } => "Circle",
            Self::Sphere { .. } => "Sphere",
            Self::SelVolumeSphere { .. } => "SelVolumeSphere",
            Self::SelVolumePipe { .. } => "SelVolumePipe",
            Self::SelVolumeObject { .. } => "SelVolumeObject",
            Self::Pipe { .. } => "Pipe",
            Self::Ellipsoid { .. } => "Ellipsoid",
            Self::Arc { .. } => "Arc",
            Self::ArcStartPoint { .. } | Self::ArcStartDirection { .. } => "Arc",
            Self::ArcStartCenter { .. } | Self::ArcStartCenterLength { .. } => "Arc",
            Self::ArcCenter { .. } => "Arc",
            Self::ArcCenterLength { .. } => "Arc",
            Self::ArcCenterMidpoint { .. } | Self::ArcCenterMidpointLength { .. } => "Arc",
            Self::Ellipse { .. } => "Ellipse",
            Self::Polyline => "Polyline",
            Self::Curve { .. } => "Curve",
            Self::InterpCrv { .. } => "InterpCrv",
            Self::Catenary { .. } => "Catenary",
            Self::Spiral { kind, .. } => kind.name(),
            Self::Hyperbola { .. } => "Hyperbola",
            Self::Parabola { kind, .. } => kind.name(),
            Self::Conic { .. } => "Conic",
            Self::Rectangle { .. } => "Rectangle",
            Self::Box { .. } => "Box",
            Self::SubDSphere { .. } => "SubDSphere",
            Self::SubDTorus { .. } => "SubDTorus",
            Self::SubDEllipsoid { .. } => "SubDEllipsoid",
            Self::SubDCylinder { .. } => "SubDCylinder",
            Self::SubDCone { .. } => "SubDCone",
            Self::SubDTruncatedCone { .. } => "SubDTruncatedCone",
            Self::SubDPrimitive { kind, .. } => kind.name(),
            Self::SelBox { .. } => "SelBox",
            Self::PointGrid { .. } => "PointGrid",
            Self::MeshPlane { .. } => "MeshPlane",
            Self::MeshBox { .. } => "MeshBox",
            Self::MeshCone { .. } => "MeshCone",
            Self::MeshTruncatedCone { .. } => "MeshTruncatedCone",
            Self::MeshCylinder { .. } => "MeshCylinder",
            Self::MeshSphere { .. } => "MeshSphere",
            Self::MeshEllipsoid { .. } => "MeshEllipsoid",
            Self::MeshTorus { .. } => "MeshTorus",
            Self::Polygon { .. } => "Polygon",
            Self::SrfPt { .. } => "SrfPt",
            Self::ExtractSrf { .. } => "ExtractSrf",
            Self::Curvature { .. } => "Curvature",
            Self::Radius {
                diameter: false, ..
            } => "Radius",
            Self::Radius { diameter: true, .. } => "Diameter",
            Self::DupFaceBorder { .. } => "DupFaceBorder",
            Self::DupEdge { .. } => "DupEdge",
            Self::ExtractMeshFaces { .. } => "ExtractMeshFaces",
            Self::ExtractConnectedMeshFaces { .. } => "ExtractConnectedMeshFaces",
            Self::ExtractMeshPart { .. } => "ExtractMeshPart",
            Self::DeleteFaces => "DeleteFaces",
            Self::SwapMeshEdge => "SwapMeshEdge",
            Self::CollapseMeshEdge => "CollapseMeshEdge",
            Self::SplitMeshEdge { .. } => "SplitMeshEdge",
            Self::FillMeshHole { .. } => "FillMeshHole",
            Self::WeldEdge => "WeldEdge",
            Self::WeldVertices => "WeldVertices",
            Self::UnweldEdge { .. } => "UnweldEdge",
            Self::UnweldVertex { .. } => "UnweldVertex",
            Self::DupMeshEdge { .. } => "DupMeshEdge",
            Self::DupMeshHoleBoundary => "DupMeshHoleBoundary",
            Self::ExtractIsocurve { .. } => "ExtractIsocurve",
            Self::CrvSeam => "CrvSeam",
            Self::SrfSeam { .. } => "SrfSeam",
            Self::Extend { .. } => "Extend",
            Self::ExtendSrf { .. } => "ExtendSrf",
            Self::SubCrv { delete: true, .. } => "DeleteSubCrv",
            Self::SubCrv { .. } => "SubCrv",
            Self::SplitCurve | Self::SplitCurveWithCutters | Self::SplitSurfaceIsocurve { .. } => {
                "Split"
            }
            Self::TrimCurve => "Trim",
            Self::Move { .. } => "Move",
            Self::SetPoint { .. } => "SetPt",
            Self::Copy { .. } => "Copy",
            Self::ArrayLinear { .. } => "ArrayLinear",
            Self::Distribute { .. } => "Distribute",
            Self::Array { .. } => "Array",
            Self::ArrayPolar { .. } => "ArrayPolar",
            Self::Scale { kind, .. } => kind.name(),
            Self::ScaleByPlane(_) => "ScaleByPlane",
            Self::ScaleNu(_) => "ScaleNU",
            Self::ScalePositions(_) => "ScalePositions",
            Self::Rotate { .. } => "Rotate",
            Self::Rotate3D { .. } => "Rotate3D",
            Self::Twist { .. } => "Twist",
            Self::Bend { .. } => "Bend",
            Self::Stretch { .. } => "Stretch",
            Self::Taper { .. } => "Taper",
            Self::Maelstrom { .. } => "Maelstrom",
            Self::Mirror { .. } | Self::MirrorThreePoint { .. } | Self::MirrorObject => "Mirror",
            Self::Shear { .. } => "Shear",
            Self::ExtrudeCurve { .. } => "ExtrudeCrv",
            Self::ExtrudeCurveTapered { .. } => "ExtrudeCrvTapered",
            Self::ExtrudeSurface { .. } => "ExtrudeSrf",
            Self::ExtrudeSurfaceTapered { .. } => "ExtrudeSrfTapered",
            Self::ExtrudeSurfaceToPoint { .. } => "ExtrudeSrfToPoint",
            Self::ExtrudeCurveToPoint { .. } => "ExtrudeCrvToPoint",
            Self::Revolve { .. } => "Revolve",
            Self::RailRevolve(_) => "RailRevolve",
        }
    }

    pub(in crate::app) const fn anchor(self) -> Option<Point3> {
        match self {
            Self::SurfaceArray { base }
            | Self::CurveArray { base }
            | Self::ShortPath { start: base }
            | Self::InterpolateCurveOnSurface { anchor: base }
            | Self::PolylineOnMesh { anchor: base }
            | Self::FaceCreate { anchor: base }
            | Self::SubDTorus { anchor: base }
            | Self::SubDEllipsoid { anchor: base }
            | Self::SubDSphere { anchor: base }
            | Self::SubDTruncatedCone { anchor: base }
            | Self::SubDCone { anchor: base }
            | Self::SubDCylinder { anchor: base }
            | Self::SubDPrimitive { anchor: base, .. } => base,
            Self::LoftPoint | Self::RemapCPlane => None,
            Self::ProjectDirection { start }
            | Self::SurfaceFlow { start }
            | Self::Flow { start } => start,
            Self::OrientPoints { last, .. }
            | Self::OrientOnSurface { last }
            | Self::Splop { last } => last,
            Self::ProjectPlane { anchor, .. } => anchor,
            Self::Section { start, .. } => start,
            Self::Contour {
                base,
                direction,
                spacing_start,
                ..
            } => {
                if spacing_start.is_some() {
                    spacing_start
                } else if direction.is_some() {
                    direction
                } else {
                    base
                }
            }
            Self::ScaleByPlane(prompt) => match (prompt.origin, prompt.plane_points) {
                (Some(p), _) | (_, [_, Some(p)]) | (_, [Some(p), _]) => Some(p),
                _ => None,
            },
            Self::ScaleNu(prompt) => prompt.origin,
            Self::ScalePositions(prompt) => prompt.origin,
            Self::Angle {
                points: [start, None, _],
            } => start,
            Self::Angle {
                points: [_, Some(_), start],
            } => start,
            Self::Align { options, .. } => match options.references {
                [_, Some(point), _] => Some(point),
                [point, _, _] => point,
            },
            Self::Point
            | Self::BlockEditAdd
            | Self::BlockEditBasePoint
            | Self::AddObjectsToBlock
            | Self::CreateUniqueBlock
            | Self::ReplaceBlock
            | Self::Block { .. }
            | Self::Insert { .. }
            | Self::EvaluatePoint
            | Self::EvaluateUv { .. }
            | Self::DomainFace
            | Self::DomainSubCrv { start: None }
            | Self::LengthSubCrv { start: None, .. }
            | Self::Points
            | Self::Line { start: None }
            | Self::Distance { start: None, .. }
            | Self::Circle { center: None }
            | Self::CircleVertical { center: None, .. }
            | Self::CircleTwoPoint { first: None }
            | Self::CircleThreePoint { points: [None, _] }
            | Self::Sphere { center: None }
            | Self::SelVolumeSphere { center: None, .. }
            | Self::SelVolumePipe { .. }
            | Self::SelVolumeObject { .. }
            | Self::Pipe { .. }
            | Self::Ellipsoid {
                points: [None, _, _],
            }
            | Self::Arc { points: [None, _] }
            | Self::ArcDeform { .. }
            | Self::ArcStartPoint { start: None, .. }
            | Self::ArcStartDirection { start: None, .. }
            | Self::ArcStartCenter { start: None, .. }
            | Self::ArcCenter { center: None, .. }
            | Self::ArcCenterMidpoint { center: None, .. }
            | Self::Ellipse { center: None, .. }
            | Self::Polyline
            | Self::Curve { .. }
            | Self::InterpCrv { .. }
            | Self::Catenary { anchor: None }
            | Self::Spiral { anchor: None, .. }
            | Self::Hyperbola { anchor: None }
            | Self::Parabola { anchor: None, .. }
            | Self::Conic { anchor: None }
            | Self::Rectangle { first: None }
            | Self::Box { base: None, .. }
            | Self::SelBox { base: None, .. }
            | Self::PointGrid { base: None, .. }
            | Self::MeshPlane { first: None, .. }
            | Self::MeshBox { base: None, .. }
            | Self::MeshCone { center: None, .. }
            | Self::MeshTruncatedCone { center: None, .. }
            | Self::MeshCylinder { center: None, .. }
            | Self::MeshSphere { center: None, .. }
            | Self::MeshEllipsoid {
                points: [None, _, _],
                ..
            }
            | Self::MeshTorus { center: None, .. }
            | Self::Polygon { center: None, .. }
            | Self::SrfPt {
                corners: [None, _, _],
            }
            | Self::ExtractSrf { .. }
            | Self::Curvature { .. }
            | Self::Radius { .. }
            | Self::DupFaceBorder { .. }
            | Self::DupEdge { .. }
            | Self::ExtractMeshFaces { .. }
            | Self::ExtractConnectedMeshFaces { .. }
            | Self::ExtractMeshPart { .. }
            | Self::DeleteFaces
            | Self::SwapMeshEdge
            | Self::CollapseMeshEdge
            | Self::SplitMeshEdge { edge_point: None }
            | Self::FillMeshHole { .. }
            | Self::WeldEdge
            | Self::WeldVertices
            | Self::UnweldEdge { .. }
            | Self::UnweldVertex { .. }
            | Self::DupMeshEdge { .. }
            | Self::DupMeshHoleBoundary
            | Self::ExtractIsocurve { .. }
            | Self::CrvSeam
            | Self::SrfSeam { .. }
            | Self::Extend { .. }
            | Self::ExtendSrf { .. }
            | Self::SubCrv { start: None, .. }
            | Self::SplitCurve
            | Self::SplitCurveWithCutters
            | Self::SplitSurfaceIsocurve { .. }
            | Self::TrimCurve
            | Self::Move { start: None }
            | Self::OrientCurveToEdge
            | Self::OffsetCurveOnSurface
            | Self::OrientOnCurve
            | Self::ArrayCurveOnSurface
            | Self::SetPoint { .. }
            | Self::Copy { start: None }
            | Self::ArrayLinear { start: None, .. }
            | Self::Distribute { start: None, .. }
            | Self::Array { start: None, .. }
            | Self::ArrayPolar { .. }
            | Self::Scale { center: None, .. }
            | Self::Rotate { center: None, .. }
            | Self::Rotate3D {
                points: [None, _, _],
            }
            | Self::Twist {
                points: [None, _, _],
                ..
            }
            | Self::Bend {
                points: [None, _], ..
            }
            | Self::Stretch {
                points: [None, _], ..
            }
            | Self::Taper {
                points: [None, _], ..
            }
            | Self::Maelstrom { center: None, .. }
            | Self::Mirror { start: None }
            | Self::MirrorThreePoint { points: [None, _] }
            | Self::MirrorObject
            | Self::Shear { origin: None, .. }
            | Self::ExtrudeCurve { base: None, .. }
            | Self::ExtrudeCurveTapered { base: None, .. }
            | Self::ExtrudeSurface { base: None, .. }
            | Self::ExtrudeSurfaceTapered { base: None, .. }
            | Self::ExtrudeSurfaceToPoint { .. }
            | Self::ExtrudeCurveToPoint { .. }
            | Self::Revolve {
                axis_start: None, ..
            } => None,
            Self::Line { start }
            | Self::DomainSubCrv { start }
            | Self::LengthSubCrv { start, .. }
            | Self::Distance { start, .. }
            | Self::Circle { center: start }
            | Self::CircleTwoPoint { first: start }
            | Self::Sphere { center: start }
            | Self::SelVolumeSphere { center: start, .. }
            | Self::Catenary { anchor: start }
            | Self::Spiral { anchor: start, .. }
            | Self::Hyperbola { anchor: start }
            | Self::Parabola { anchor: start, .. }
            | Self::Conic { anchor: start }
            | Self::Rectangle { first: start }
            | Self::Box { base: start, .. }
            | Self::SelBox { base: start, .. }
            | Self::PointGrid { base: start, .. }
            | Self::MeshPlane { first: start, .. }
            | Self::MeshBox { base: start, .. }
            | Self::MeshCone { center: start, .. }
            | Self::MeshCylinder { center: start, .. }
            | Self::MeshSphere { center: start, .. }
            | Self::SubCrv { start, .. }
            | Self::Move { start }
            | Self::Copy { start }
            | Self::ArrayLinear { start, .. }
            | Self::Distribute { start, .. }
            | Self::Array { start, .. }
            | Self::Mirror { start }
            | Self::ExtrudeCurve { base: start, .. }
            | Self::ExtrudeCurveTapered { base: start, .. }
            | Self::ExtrudeSurface { base: start, .. }
            | Self::ExtrudeSurfaceTapered { base: start, .. }
            | Self::Revolve {
                axis_start: start, ..
            } => start,
            Self::RailRevolve(state) => state.axis_start,
            Self::MirrorThreePoint { points } => match points[1] {
                Some(point) => Some(point),
                None => points[0],
            },
            Self::CircleSize { center, .. } => Some(center),
            Self::CircleVertical {
                center: Some(center),
                ..
            } => Some(center),
            Self::CircleOrientation {
                center,
                normal_point: None,
                ..
            } => Some(center),
            Self::CircleOrientation {
                normal_point: Some(normal_point),
                ..
            } => Some(normal_point),
            Self::MeshTruncatedCone {
                center: Some(center),
                end_center: None,
                ..
            } => Some(center),
            Self::MeshTruncatedCone {
                end_center: Some(end_center),
                ..
            } => Some(end_center),
            Self::MeshTorus {
                center: Some(center),
                major_point: None,
                ..
            } => Some(center),
            Self::MeshTorus {
                major_point: Some(major_point),
                ..
            } => Some(major_point),
            Self::SplitMeshEdge { edge_point } => edge_point,
            Self::ArcExtension { anchor: center }
            | Self::ArcTilted { anchor: center }
            | Self::ArcTangent { anchor: center }
            | Self::CircleTangent { anchor: center }
            | Self::CircleDeform { anchor: center }
            | Self::CircleAround { center }
            | Self::Ellipse { center, .. }
            | Self::Polygon { center, .. }
            | Self::Ellipsoid {
                points: [center, _, _],
            }
            | Self::MeshEllipsoid {
                points: [center, _, _],
                ..
            } => center,
            Self::Arc {
                points: [_, Some(point)],
            }
            | Self::Arc {
                points: [Some(point), None],
            }
            | Self::ArcStartPoint {
                end: Some(point), ..
            }
            | Self::ArcStartPoint {
                start: Some(point),
                end: None,
            }
            | Self::ArcStartDirection {
                tangent: Some(point),
                ..
            }
            | Self::ArcStartDirection {
                start: Some(point),
                tangent: None,
            }
            | Self::ArcStartCenter {
                center: Some(point),
                ..
            }
            | Self::ArcStartCenter {
                start: Some(point),
                center: None,
                ..
            }
            | Self::ArcStartCenterLength { center: point, .. }
            | Self::CircleThreePoint {
                points: [_, Some(point)],
            }
            | Self::CircleThreePoint {
                points: [Some(point), None],
            } => Some(point),
            Self::ArcCenter {
                center: Some(center),
                start: None,
                ..
            } => Some(center),
            Self::ArcCenter {
                start: Some(start), ..
            } => Some(start),
            Self::ArcCenterLength { start, .. } => Some(start),
            Self::ArcCenterMidpoint {
                midpoint: Some(midpoint),
                ..
            } => Some(midpoint),
            Self::ArcCenterMidpoint {
                center: Some(center),
                midpoint: None,
                ..
            } => Some(center),
            Self::ArcCenterMidpointLength { midpoint, .. } => Some(midpoint),
            Self::CircleThreePointRadius { second, .. } => Some(second),
            Self::SrfPt {
                corners: [_, _, Some(corner)],
            }
            | Self::SrfPt {
                corners: [_, Some(corner), None],
            }
            | Self::SrfPt {
                corners: [Some(corner), None, None],
            } => Some(corner),
            Self::Scale {
                center: Some(center),
                ..
            }
            | Self::Rotate {
                center: Some(center),
                ..
            }
            | Self::Shear {
                origin: Some(center),
                ..
            } => Some(center),
            Self::Rotate3D {
                points: [Some(start), _, _],
            }
            | Self::Twist {
                points: [Some(start), _, _],
                ..
            } => Some(start),
            Self::Maelstrom {
                center: Some(center),
                ..
            } => Some(center),
            Self::Stretch {
                points: [Some(start), end],
                ..
            } => Some(match end {
                Some(end) => end,
                None => start,
            }),
            Self::Taper {
                points: [Some(start), end],
                initial,
                ..
            } => Some(match (end, initial) {
                (Some(end), Some(_)) => end,
                _ => start,
            }),
            Self::Bend {
                points: [Some(start), end],
                ..
            } => Some(match end {
                Some(end) => end,
                None => start,
            }),
        }
    }

    pub(in crate::app) const fn reference(self) -> Option<Point3> {
        match self {
            Self::ScaleByPlane(prompt) => prompt.reference,
            Self::ScaleNu(prompt) => prompt.reference,
            Self::ScalePositions(prompt) => prompt.reference,
            Self::Scale { reference, .. }
            | Self::Rotate { reference, .. }
            | Self::Shear { reference, .. } => reference,
            Self::Rotate3D {
                points: [_, _, reference],
            }
            | Self::Twist {
                points: [_, _, reference],
                ..
            } => reference,
            Self::Ellipse { first_axis, .. } => first_axis,
            Self::Ellipsoid { points } => match points {
                [_, _, Some(second_axis)] => Some(second_axis),
                [_, Some(first_axis), None] => Some(first_axis),
                _ => None,
            },
            Self::MeshEllipsoid { points, .. } => match points {
                [_, _, Some(second_axis)] => Some(second_axis),
                [_, Some(first_axis), None] => Some(first_axis),
                _ => None,
            },
            _ => None,
        }
    }

    pub(in crate::app) const fn collects_curve_points(self) -> bool {
        matches!(
            self,
            Self::Polyline | Self::Curve { .. } | Self::InterpCrv { .. } | Self::SplitCurve
        )
    }
}
