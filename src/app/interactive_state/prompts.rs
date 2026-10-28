//! Stage-specific command-line prompts.
use super::*;

impl InteractiveCommand {
    pub(in crate::app) const fn prompt(self) -> &'static str {
        match self {
            Self::SweepPoint => "Sweep1: pick the point end (Esc cancels)",
            Self::LoftPoint => "Loft: pick the point end (Esc cancels)",
            Self::RemapCPlane => {
                "RemapCPlane: click a target viewport, or choose CPlane or View (Esc cancels)"
            }
            Self::OrientCurveToEdge => {
                "OrientCrvToEdge: select a curve near an end, then a surface edge"
            }
            Self::FaceCreate { .. } => {
                "3DFace: pick corners (Enter accepts, Undo removes a pick, Esc cancels)"
            }
            Self::PolylineOnMesh { .. } => {
                "PolylineOnMesh: select a mesh, then polyline points (Close Undo)"
            }
            Self::InterpolateCurveOnSurface { .. } => {
                "InterpCrvOnSrf: select a surface, then interpolation points"
            }
            Self::ShortPath { .. } => "ShortPath: select a surface, then start and end points",
            Self::OffsetCurveOnSurface => {
                "OffsetCrvOnSrf: select a curve and surface, then enter distance (ThroughPoint, Flip, Number, SubCrv)"
            }
            Self::OrientOnCurve => "OrientOnCrv: follow the curve placement prompt (Esc finishes)",
            Self::SurfaceArray { .. } => "ArraySrf: follow the surface array prompt (Esc cancels)",
            Self::CurveArray { .. } => "ArrayCrv: follow the curve array prompt (Esc cancels)",
            Self::ArrayCurveOnSurface => {
                "ArrayCrvOnSrf: follow the surface curve array prompt (Esc finishes)"
            }
            Self::SurfaceFlow { .. } => {
                "FlowAlongSrf: choose base and target surfaces; Enter applies; Esc cancels"
            }
            Self::Flow { .. } => {
                "Flow: choose base and target curves, then Enter applies (Esc cancels)"
            }
            Self::OrientOnSurface { .. } => {
                "OrientOnSrf: follow the surface placement prompt (Esc finishes)"
            }
            Self::Splop { .. } => {
                "Splop: draw a reference sphere, pick a surface, then size the destination sphere"
            }
            Self::OrientPoints { .. } => {
                "Pick the next orientation reference or target point (Esc cancels)"
            }
            Self::ProjectPlane { mode, step, .. } => mode.point_hint(step),
            Self::ProjectDirection { start: None } => {
                "Project: pick the custom direction start (Esc cancels)"
            }
            Self::ProjectDirection { start: Some(_) } => {
                "Project: pick the custom direction end (Undo revises start; Esc cancels)"
            }
            Self::Contour { base: None, .. } => {
                "Contour: pick the base point (Range sets the picked interval; Esc cancels)"
            }
            Self::Contour {
                direction: None, ..
            } => "Contour: pick a direction perpendicular to the planes (Esc cancels)",
            Self::Contour { .. } => {
                "Contour: enter spacing or pick two spacing points (Esc cancels)"
            }
            Self::Section { start: None, .. } => {
                "Section: pick the section plane start (Esc cancels)"
            }
            Self::Section { start: Some(_), .. } => {
                "Section: pick the section plane end (Esc cancels)"
            }
            Self::BlockEditAdd => {
                "BlockEdit: select external objects to copy; Enter accepts, Esc cancels"
            }
            Self::BlockEditBasePoint => "BlockEdit: pick a new base point (Esc cancels)",
            Self::Block { base: None } => "Block: pick a base point (Esc cancels)",
            Self::CreateUniqueBlock => {
                "CreateUniqueBlock: enter the new definition name (Esc cancels)"
            }
            Self::ReplaceBlock => {
                "ReplaceBlock: pick a replacement instance (SelectFromBlockDefinitionList, BlockDefinitionName, All, None; Esc cancels)"
            }
            Self::AddObjectsToBlock => {
                "AddObjectsToBlock: pick a target instance, then objects to add; Enter accepts, Esc cancels"
            }
            Self::Block { base: Some(_) } => "Block: enter the definition name (Esc cancels)",
            Self::Insert { has_name: false } => {
                "Insert: enter a block definition name (Esc cancels)"
            }
            Self::Insert { has_name: true } => {
                "Insert: pick an insertion point (Scale=n|x,y,z Rotation=degrees Axis=x,y,z; Esc cancels)"
            }
            Self::ScaleByPlane(prompt) => prompt.prompt(),
            Self::ScaleNu(prompt) => prompt.prompt(),
            Self::ScalePositions(prompt) => prompt.prompt(),
            Self::Angle {
                points: [None, _, _],
            } => "Angle: pick the first direction's start (TwoObjects; Esc cancels)",
            Self::Angle {
                points: [Some(_), None, _],
            } => "Angle: pick the first direction's end (Esc cancels)",
            Self::Angle {
                points: [Some(_), Some(_), None],
            } => "Angle: pick the second direction's start (Esc cancels)",
            Self::Angle { .. } => "Angle: pick the second direction's end (Esc cancels)",
            Self::Point => "Point: pick a location in the viewport (Esc to cancel)",
            Self::EvaluatePoint => {
                "EvaluatePt: pick a location to report world/CPlane coordinates (Esc cancels)"
            }
            Self::EvaluateUv { .. } => {
                "EvaluateUVPt: pick surface locations (Normalized=Yes|No; CreatePoint=Yes|No; Enter or Esc finishes)"
            }
            Self::DomainFace => {
                "Domain: pick a component surface on the selected polysurface (Esc cancels)"
            }
            Self::DomainSubCrv { start: None } => {
                "Domain SubCrv: pick the start on the selected curve (Esc cancels)"
            }
            Self::DomainSubCrv { start: Some(_) } => {
                "Domain SubCrv: pick the end on the selected curve (Esc cancels)"
            }
            Self::LengthSubCrv { start: None, .. } => {
                "Length SubCrv: pick the start on the selected curve (Units=name; Esc cancels)"
            }
            Self::LengthSubCrv { start: Some(_), .. } => {
                "Length SubCrv: pick the end on the selected curve (Units=name; Esc cancels)"
            }
            Self::Align { options, .. } if options.mode.is_none() => {
                "Align: choose Left/Right/Top/Bottom/HorizCenter/VertCenter/Concentric/ToLine/ToPlane/ToFitPlane/ToCurve; AlignTo=CPlane|World"
            }
            Self::Align { options, .. }
                if matches!(
                    options.mode,
                    Some(viboceros_command::AlignmentMode::ToCurve)
                ) =>
            {
                "Align: select the alignment curve or type CurveId=uuid (Esc cancels)"
            }
            Self::Align { options, .. } if options.reference_count() > 0 => {
                match options.references {
                    [None, _, _] => {
                        "Align: pick the first reference point (ToPlane supports 3Point; Esc cancels)"
                    }
                    [_, None, _] => "Align: pick the second reference point (Esc cancels)",
                    _ => "Align: pick the third plane point (Esc cancels)",
                }
            }
            Self::Align { .. } => {
                "Align: pick an alignment point, or Enter for automatic alignment (Esc cancels)"
            }
            Self::Points => "Points: pick locations; Undo removes the last; Enter or Esc finishes",
            Self::Line { start: None } => {
                "Line: pick the start point in the viewport (Esc to cancel)"
            }
            Self::Distance { start: None, .. } => {
                "Distance: pick the first point (Units=name; Esc cancels)"
            }
            Self::Distance { start: Some(_), .. } => {
                "Distance: pick the second point (Units=name; Undo revises the first; Esc cancels)"
            }
            Self::Line { start: Some(_) } => {
                "Line: pick the end point in the viewport (Esc to cancel)"
            }
            Self::Circle { center: None } => {
                "Circle: pick the center or enter Deformable/2Point/3Point/Vertical/AroundCurve/Tangent/FitPoints (Esc cancels)"
            }
            Self::Circle { center: Some(_) } => {
                "Circle: pick a radius point or type Radius/Diameter/Circumference/Area (Esc cancels)"
            }
            Self::ArcTangent { .. } => {
                "Arc Tangent: specify contacts and choose an arc (Esc cancels)"
            }
            Self::ArcExtension { .. } => {
                "Arc Extension: select a source, endpoint or Center (Esc cancels)"
            }
            Self::ArcTilted { .. } => {
                "Arc Tilted: specify start, orientation, and endpoint or angle (Esc cancels)"
            }
            Self::ArcDeform {
                setting: Some(viboceros_command::arc_deform_input::Setting::Degree),
            } => "Arc Deformable: enter degree (Esc cancels)",
            Self::ArcDeform {
                setting: Some(viboceros_command::arc_deform_input::Setting::PointCount),
            } => "Arc Deformable: enter point count (Esc cancels)",
            Self::ArcDeform { setting: None } => {
                "Arc Deformable: center or Degree/PointCount/StartPoint/Tangent/Extension (Esc cancels)"
            }
            Self::CircleTangent { .. } => {
                "Circle Tangent: pick contacts, Point, radius or FromFirstPoint (Esc cancels)"
            }
            Self::CircleDeform { .. } => "Circle Deformable: specify a circle (Esc cancels)",
            Self::CircleAround { .. } => {
                "Circle AroundCurve: select a guide, center and size (Esc cancels)"
            }
            Self::CircleSize { mode, .. } => mode.prompt(),
            Self::CircleVertical { center: None, .. } => {
                "Circle Vertical: pick the center (Esc cancels)"
            }
            Self::CircleVertical {
                center: Some(_),
                radius: None,
                ..
            } => "Circle Vertical: pick a radius point or enter a size (Esc cancels)",
            Self::CircleVertical {
                center: Some(_),
                radius: Some(_),
                ..
            } => "Circle Vertical: pick a direction point (Esc cancels)",
            Self::CircleOrientation {
                normal_point: None, ..
            } => "Circle Orientation: pick a perpendicular direction point (Esc cancels)",
            Self::CircleOrientation {
                normal_point: Some(_),
                ..
            } => "Circle Orientation: pick a radius point or enter a size (Esc cancels)",
            Self::CircleTwoPoint { first: None } => {
                "Circle 2Point: pick the first diameter end (Esc cancels)"
            }
            Self::CircleTwoPoint { first: Some(_) } => {
                "Circle 2Point: pick the second diameter end (Esc cancels)"
            }
            Self::CircleThreePoint { points: [None, _] } => {
                "Circle 3Point: pick the first point (Esc cancels)"
            }
            Self::CircleThreePoint {
                points: [Some(_), None],
            } => "Circle 3Point: pick the second point (Esc cancels)",
            Self::CircleThreePoint { .. } => "Circle 3Point: pick the third point (Esc cancels)",
            Self::CircleThreePointRadius { radius: None, .. } => {
                "Circle 3Point Radius: pick a radius location or enter a radius (Esc cancels)"
            }
            Self::CircleThreePointRadius {
                radius: Some(_), ..
            } => "Circle 3Point Radius: pick the center direction (Esc cancels)",
            Self::Sphere { center: None } => {
                "Sphere: pick the center or choose FitPoints (Esc to cancel)"
            }
            Self::Sphere { center: Some(_) } => {
                "Sphere: pick a point on the sphere in the viewport (Esc to cancel)"
            }
            Self::SelVolumeSphere { center: None, .. } => {
                "SelVolumeSphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::SelVolumeSphere {
                center: Some(_), ..
            } => "SelVolumeSphere: pick a radius point in the viewport (Esc to cancel)",
            Self::SelVolumePipe { source: None, .. } => {
                "SelVolumePipe: select a centerline curve (Esc to cancel)"
            }
            Self::SelVolumePipe {
                source: Some(_), ..
            } => "SelVolumePipe: pick a radius point near the curve (Esc to cancel)",
            Self::SelVolumeObject { .. } => {
                "SelVolumeObject: select a closed mesh or polysurface (Esc to cancel)"
            }
            Self::Pipe { source: None, .. } => "Pipe: select a rail curve (Esc to cancel)",
            Self::Pipe {
                source: Some(_),
                first_radius: Some(_),
                ..
            } => "Pipe: pick the second radius point near the rail (Esc to cancel)",
            Self::Pipe {
                source: Some(_),
                pick_second_radius: true,
                ..
            } => "Pipe: pick the first radius point near the rail (Esc to cancel)",
            Self::Pipe {
                source: Some(_), ..
            } => "Pipe: pick a radius point near the rail (Esc to cancel)",
            Self::Ellipsoid { points } => match points {
                [None, _, _] => "Ellipsoid: pick the center in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "Ellipsoid: pick the end of the first axis in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "Ellipsoid: pick the second-axis radius in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Ellipsoid: pick the third-axis radius in the viewport (Esc to cancel)"
                }
            },
            Self::Arc { points } => match points {
                [None, _] => "Arc StartPoint ThroughPoint: pick the start point (Esc to cancel)",
                [Some(_), None] => {
                    "Arc StartPoint ThroughPoint: pick a point on the arc (Esc to cancel)"
                }
                [Some(_), Some(_)] => {
                    "Arc StartPoint ThroughPoint: pick the endpoint (Esc to cancel)"
                }
            },
            Self::ArcStartPoint { start: None, .. } => {
                "Arc StartPoint: pick the start point (Esc to cancel)"
            }
            Self::ArcStartPoint { end: None, .. } => {
                "Arc StartPoint: pick the endpoint or choose Direction/ThroughPoint/Center (Esc to cancel)"
            }
            Self::ArcStartPoint { .. } => "Arc StartPoint: pick a point on the arc (Esc to cancel)",
            Self::ArcStartDirection { start: None, .. } => {
                "Arc Direction: pick the start point (Esc to cancel)"
            }
            Self::ArcStartDirection { tangent: None, .. } => {
                "Arc Direction: pick a tangent direction point (Esc to cancel)"
            }
            Self::ArcStartDirection { .. } => "Arc Direction: pick the endpoint (Esc to cancel)",
            Self::ArcStartCenter { start: None, .. } => {
                "Arc StartPoint Center: pick the start point (Esc to cancel)"
            }
            Self::ArcStartCenter { center: None, .. } => {
                "Arc StartPoint Center: pick the center (Esc to cancel)"
            }
            Self::ArcStartCenter { .. } => {
                "Arc StartPoint Center: pick the endpoint, enter an angle, or choose Length/Direction (Esc cancels)"
            }
            Self::ArcStartCenterLength { .. } => {
                "Arc StartPoint Center Length: enter a signed length or pick two distance points; Direction reverses it (Esc cancels)"
            }
            Self::ArcCenter { center: None, .. } => {
                "Arc: pick the center or choose Deformable/StartPoint/Tangent/Extension (Esc cancels)"
            }
            Self::ArcCenter { start: None, .. } => {
                "Arc Center: pick the start point or choose Tilted/Midpoint (Esc cancels)"
            }
            Self::ArcCenter { .. } => {
                "Arc Center: pick the endpoint, enter an angle, or choose Length/Direction (Esc cancels)"
            }
            Self::ArcCenterLength { .. } => {
                "Arc Center Length: enter a signed length or pick two distance points; Direction reverses it (Esc cancels)"
            }
            Self::ArcCenterMidpoint { center: None, .. } => {
                "Arc Center Midpoint: pick the center (Esc to cancel)"
            }
            Self::ArcCenterMidpoint { midpoint: None, .. } => {
                "Arc Center Midpoint: pick the arc midpoint (Esc to cancel)"
            }
            Self::ArcCenterMidpoint { .. } => {
                "Arc Center Midpoint: pick the endpoint, enter an angle, or choose Length/Direction (Esc cancels)"
            }
            Self::ArcCenterMidpointLength { .. } => {
                "Arc Center Midpoint Length: enter a signed length or pick two distance points; Direction reverses it (Esc cancels)"
            }
            Self::Ellipse { center: None, .. } => {
                "Ellipse: pick the center in the viewport (Esc to cancel)"
            }
            Self::Ellipse {
                center: Some(_),
                first_axis: None,
            } => "Ellipse: pick the end of the first axis in the viewport (Esc to cancel)",
            Self::Ellipse {
                first_axis: Some(_),
                ..
            } => "Ellipse: pick the second-axis radius in the viewport (Esc to cancel)",
            Self::Polyline => {
                "Polyline: pick vertices; Close closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::Curve { .. } => {
                "Curve: pick control points; Close/Sharp closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::InterpCrv { .. } => {
                "InterpCrv: pick curve points; Close/Sharp closes; Undo removes last point; Enter finishes (Esc cancels)"
            }
            Self::Catenary { .. } => "Catenary: follow the point prompts (Esc cancels)",
            Self::Spiral { .. } => "Helix/Spiral: follow the point prompts (Esc cancels)",
            Self::Hyperbola { .. } => "Hyperbola: follow the point prompts (Esc cancels)",
            Self::Parabola { .. } => "Parabola: follow the point prompts (Esc cancels)",
            Self::Conic { .. } => {
                "Conic: pick start, end, apex and curvature point or rho (Esc cancels)"
            }
            Self::Rectangle { first: None } => {
                "Rectangle: pick the first corner in the viewport (Esc to cancel)"
            }
            Self::Rectangle { first: Some(_) } => {
                "Rectangle: pick the opposite corner in the viewport (Esc to cancel)"
            }
            Self::MeshPlane { first: None, .. } => {
                "MeshPlane: pick the first corner in the viewport (Esc to cancel)"
            }
            Self::MeshPlane { first: Some(_), .. } => {
                "MeshPlane: pick the opposite corner in the viewport (Esc to cancel)"
            }
            Self::MeshBox { base: None, .. } => {
                "MeshBox: pick the first base corner in the viewport (Esc to cancel)"
            }
            Self::MeshBox {
                base: Some(_),
                opposite: None,
                ..
            } => "MeshBox: pick the opposite base corner in the viewport (Esc to cancel)",
            Self::MeshBox {
                opposite: Some(_), ..
            } => "MeshBox: pick the height in the viewport (Esc to cancel)",
            Self::PointGrid {
                base: None,
                options,
                ..
            } if options.centered() => "PointGrid: pick the base center (Esc to cancel)",
            Self::PointGrid { base: None, .. } => {
                "PointGrid: pick the first base corner (Esc to cancel)"
            }
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.three_point() || options.vertical() => {
                "PointGrid: pick the end of the first edge (Esc to cancel)"
            }
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.centered() => "PointGrid: pick a base corner (Esc to cancel)",
            Self::PointGrid {
                opposite: None,
                options,
                ..
            } if options.diagonal() => {
                "PointGrid: pick the opposite diagonal corner (Esc to cancel)"
            }
            Self::PointGrid { opposite: None, .. } => {
                "PointGrid: pick the opposite base corner (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                width: Some(_),
                options,
                ..
            } if options.three_point() || options.vertical() => {
                "PointGrid: pick which side of the edge contains the rectangle (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                options,
                ..
            } if options.vertical() => {
                "PointGrid: pick a vertical width point or enter a width (Esc to cancel)"
            }
            Self::PointGrid {
                third: None,
                options,
                ..
            } if options.three_point() => {
                "PointGrid: pick a point on the opposite side or enter a width (Esc to cancel)"
            }
            Self::PointGrid { options, .. } if options.diagonal() => {
                "PointGrid: pick a height point (Esc to cancel)"
            }
            Self::PointGrid { .. } => {
                "PointGrid: pick or enter height; Enter uses base width (Esc to cancel)"
            }
            Self::SubDTorus { .. } => "SubDTorus: pick a base circle and tube size; Esc cancels",
            Self::SubDEllipsoid { .. } => {
                "SubDEllipsoid: pick a base ellipse and third axis; Esc cancels"
            }
            Self::SubDSphere { .. } => "SubDSphere: pick sphere references; Esc cancels",
            Self::SubDTruncatedCone { .. } => {
                "SubDTruncatedCone: pick a base circle, height and end size; Esc cancels"
            }
            Self::SubDCone { .. } => "SubDCone: pick a base circle or cone end; Esc cancels",
            Self::SubDCylinder { .. } => {
                "SubDCylinder: pick a base circle or cylinder end; Esc cancels"
            }
            Self::SubDPrimitive {
                kind: subd_primitives::Kind::Box,
                ..
            } => "SubDBox: pick the base or height; Esc cancels",
            Self::SubDPrimitive {
                kind: subd_primitives::Kind::Plane,
                ..
            } => "SubDPlane: pick rectangle points; Esc cancels",
            Self::Box { base: None, .. } => "Box: pick the first base corner (Esc to cancel)",
            Self::Box {
                base: Some(_),
                opposite: None,
            } => "Box: pick the opposite base corner (Esc to cancel)",
            Self::Box {
                opposite: Some(_), ..
            } => "Box: pick a height point (Esc to cancel)",
            Self::SelBox { base: None, .. } => "SelBox: pick the first base corner (Esc to cancel)",
            Self::SelBox {
                base: Some(_),
                opposite: None,
                ..
            } => "SelBox: pick the opposite base corner (Esc to cancel)",
            Self::SelBox {
                opposite: Some(_), ..
            } => "SelBox: pick a height point (Esc to cancel)",
            Self::MeshCone { center: None, .. } => {
                "MeshCone: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshCone {
                center: Some(_),
                radius_point: None,
                ..
            } => "MeshCone: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshCone {
                radius_point: Some(_),
                ..
            } => "MeshCone: pick the apex height in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone { center: None, .. } => {
                "MeshTruncatedCone: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshTruncatedCone {
                center: Some(_),
                base_radius_point: None,
                ..
            } => "MeshTruncatedCone: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone {
                base_radius_point: Some(_),
                end_center: None,
                ..
            } => "MeshTruncatedCone: pick the end-circle center in the viewport (Esc to cancel)",
            Self::MeshTruncatedCone {
                end_center: Some(_),
                ..
            } => "MeshTruncatedCone: pick the end-circle radius in the viewport (Esc to cancel)",
            Self::MeshCylinder { center: None, .. } => {
                "MeshCylinder: pick the base center in the viewport (Esc to cancel)"
            }
            Self::MeshCylinder {
                center: Some(_),
                radius_point: None,
                ..
            } => "MeshCylinder: pick the base radius in the viewport (Esc to cancel)",
            Self::MeshCylinder {
                radius_point: Some(_),
                ..
            } => "MeshCylinder: pick the height in the viewport (Esc to cancel)",
            Self::MeshSphere { center: None, .. } => {
                "MeshSphere: pick the center in the viewport (Esc to cancel)"
            }
            Self::MeshSphere {
                center: Some(_), ..
            } => "MeshSphere: pick an equator radius in the viewport (Esc to cancel)",
            Self::MeshEllipsoid { points, .. } => match points {
                [None, _, _] => "MeshEllipsoid: pick the center in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "MeshEllipsoid: pick the end of the first axis in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "MeshEllipsoid: pick the second-axis radius in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "MeshEllipsoid: pick the third-axis radius in the viewport (Esc to cancel)"
                }
            },
            Self::MeshTorus { center: None, .. } => {
                "MeshTorus: pick the center in the viewport (Esc to cancel)"
            }
            Self::MeshTorus {
                center: Some(_),
                major_point: None,
                ..
            } => "MeshTorus: pick the major-circle radius in the viewport (Esc to cancel)",
            Self::MeshTorus {
                major_point: Some(_),
                ..
            } => "MeshTorus: pick the tube radius in the viewport (Esc to cancel)",
            Self::Polygon { center: None, .. } => {
                "Polygon: pick the center in the viewport (Esc to cancel)"
            }
            Self::Polygon {
                center: Some(_), ..
            } => "Polygon: pick the first vertex in the viewport (Esc to cancel)",
            Self::SrfPt { corners } => match corners {
                [None, _, _] => "SrfPt: pick the first corner in the viewport (Esc to cancel)",
                [Some(_), None, _] => {
                    "SrfPt: pick the second corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), None] => {
                    "SrfPt: pick the third corner in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "SrfPt: pick the fourth corner in the viewport (Esc to cancel)"
                }
            },
            Self::ExtractSrf { .. } => {
                "ExtractSrf: select faces; Enter extracts, Ctrl/Command removes (Esc cancels)"
            }
            Self::Curvature { .. } => {
                "Curvature: pick a location on the selected curve or surface (Esc to cancel)"
            }
            Self::Radius {
                diameter: false, ..
            } => {
                "Radius: pick a curve location (Units=name; preselection limits curves; Esc cancels)"
            }
            Self::Radius { diameter: true, .. } => {
                "Diameter: pick a curve location (Units=name; preselection limits curves; Esc cancels)"
            }
            Self::DupFaceBorder { .. } => {
                "DupFaceBorder: pick a face location on a selected surface or B-rep (Esc to cancel)"
            }
            Self::DupEdge { .. } => {
                "DupEdge: pick an edge location on a selected surface, B-rep, or mesh (Esc to cancel)"
            }
            Self::ExtractMeshFaces { .. } => {
                "ExtractMeshFaces: pick a face on a selected mesh (Esc to cancel)"
            }
            Self::ExtractConnectedMeshFaces { .. } => {
                "ExtractConnectedMeshFaces: pick a face on a selected mesh (Esc to cancel)"
            }
            Self::ExtractMeshPart { .. } => {
                "ExtractMeshPart: pick a face on a selected mesh (Esc to cancel)"
            }
            Self::DeleteFaces => {
                "DeleteFaces: pick a face on a selected mesh or B-rep (Esc to cancel)"
            }
            Self::SwapMeshEdge => {
                "SwapMeshEdge: pick an interior edge shared by two selected-mesh triangles (Esc to cancel)"
            }
            Self::CollapseMeshEdge => {
                "CollapseMeshEdge: pick a topology edge on a selected mesh (Esc to cancel)"
            }
            Self::SplitMeshEdge { edge_point: None } => {
                "SplitMeshEdge: pick a topology edge on a selected mesh (Esc to cancel)"
            }
            Self::SplitMeshEdge {
                edge_point: Some(_),
            } => "SplitMeshEdge: pick the split location along the edge (Esc to cancel)",
            Self::FillMeshHole { .. } => {
                "FillMeshHole: pick a closed naked boundary on a selected mesh (Esc to cancel)"
            }
            Self::WeldEdge => {
                "WeldEdge: pick a mesh topology edge or SubD boundary edge on a selected object (Esc to cancel)"
            }
            Self::WeldVertices => {
                "WeldVertices: pick a mesh topology vertex or SubD vertex on a selected object (Esc to cancel)"
            }
            Self::UnweldEdge { .. } => {
                "UnweldEdge: pick a mesh topology edge or SubD edge on a selected object (Esc to cancel)"
            }
            Self::UnweldVertex { .. } => {
                "UnweldVertex: pick a mesh topology vertex or SubD vertex on a selected object (Esc to cancel)"
            }
            Self::DupMeshEdge { .. } => {
                "DupMeshEdge: pick a logical edge on a selected mesh (Esc to cancel)"
            }
            Self::DupMeshHoleBoundary => {
                "DupMeshHoleBoundary: pick a closed naked mesh boundary (Esc to cancel)"
            }
            Self::ExtractIsocurve { .. } => {
                "ExtractIsocurve: pick a location on the selected surface or B-rep face (Esc to cancel)"
            }
            Self::CrvSeam => {
                "CrvSeam: pick a new seam location on the selected closed curve (Esc to cancel)"
            }
            Self::SrfSeam { .. } => {
                "SrfSeam: pick a new seam location on the selected closed surface (Esc to cancel)"
            }
            Self::Extend { .. } => {
                "Extend: pick the source curve near the end to extend to the other selected curve, surface, or B-rep boundaries (Esc to cancel)"
            }
            Self::ExtendSrf { .. } => {
                "ExtendSrf: pick a natural edge on the selected surface (Esc to cancel)"
            }
            Self::SubCrv {
                delete: true,
                start: None,
                ..
            } => "DeleteSubCrv: pick the deletion start (Esc cancels)",
            Self::SubCrv {
                delete: true,
                start: Some(_),
                ..
            } => "DeleteSubCrv: pick the deletion end or enter a length (Esc cancels)",
            Self::SubCrv { start: None, .. } => {
                "SubCrv: pick the subcurve start on the selected curve (Esc to cancel)"
            }
            Self::SubCrv { start: Some(_), .. } => {
                "SubCrv: pick the directed subcurve end on the selected curve (Esc to cancel)"
            }
            Self::SplitCurve => {
                "Split: pick curve split locations; press Enter to finish (Esc to cancel)"
            }
            Self::SplitCurveWithCutters => {
                "Split: pick the selected source curve or rectangular surface; the other selected curves, surfaces, and B-reps are cutters (Esc to cancel)"
            }
            Self::SplitSurfaceIsocurve { .. } => {
                "Split: pick an isocurve location on the selected surface (Esc to cancel)"
            }
            Self::TrimCurve => {
                "Trim: pick the interval to remove from one selected curve; the other selected curves, surfaces, and B-reps are cutters (Esc to cancel)"
            }
            Self::Move { start: None } => {
                "Move: pick the base point; Enter uses bounding-box center; Vertical constrains direction; Esc cancels"
            }
            Self::Move { start: Some(_) } => {
                "Move: pick the destination point in the viewport (Esc to cancel)"
            }
            Self::SetPoint { .. } => {
                "SetPt: pick a target point; XSet/YSet/ZSet=Yes|No, Alignment=World|CPlane, Copy=Yes|No (Enter finishes copies; Esc cancels)"
            }
            Self::Copy { start: None } => {
                "Copy: pick the base point; Enter uses bounding-box center; Vertical or InPlace; Esc cancels"
            }
            Self::Copy { start: Some(_) } => {
                "Copy: pick the destination point; Enter or Esc finishes"
            }
            Self::ArrayLinear { start: None, .. } => {
                "ArrayLinear: pick the first reference point in the viewport (Esc to cancel)"
            }
            Self::ArrayLinear { start: Some(_), .. } => {
                "ArrayLinear: pick the spacing point in the viewport (Esc to cancel)"
            }
            Self::Distribute { start: None, .. } => {
                "Distribute: pick the first direction point (Esc to cancel)"
            }
            Self::Distribute { start: Some(_), .. } => {
                "Distribute: pick the second direction point (Esc to cancel)"
            }
            Self::Array { start: None, .. } => {
                "Array: pick the first cell corner in the viewport (Esc to cancel)"
            }
            Self::Array {
                fill: false,
                start: Some(_),
                ..
            } => "Array: pick the opposite UnitCell corner in the viewport (Esc to cancel)",
            Self::Array {
                fill: true,
                start: Some(_),
                ..
            } => "Array: pick the opposite Fill corner in the viewport (Esc to cancel)",
            Self::ArrayPolar { .. } => {
                "ArrayPolar: pick the array center in the viewport (Esc to cancel)"
            }
            Self::Scale {
                kind, center: None, ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the center, or Enter for the bounding-box center (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the origin, or Enter for the bounding-box center (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the center, or Enter for the bounding-box center (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                center: Some(_),
                reference: None,
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the reference point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the reference point and direction (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the reference point in the viewport (Esc to cancel)"
                }
            },
            Self::Scale {
                kind,
                reference: Some(_),
                ..
            } => match kind {
                InteractiveScaleKind::Uniform => {
                    "Scale: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::OneDimensional => {
                    "Scale1D: pick the target point in the viewport (Esc to cancel)"
                }
                InteractiveScaleKind::TwoDimensional => {
                    "Scale2D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Rotate { center: None, .. } => {
                "Rotate: pick the center point in the viewport (Esc to cancel)"
            }
            Self::Rotate {
                center: Some(_),
                reference: None,
            } => "Rotate: pick the reference point in the viewport (Esc to cancel)",
            Self::Rotate {
                reference: Some(_), ..
            } => "Rotate: pick the target point in the viewport (Esc to cancel)",
            Self::Rotate3D { points } => match points {
                [None, _, _] => "Rotate3D: pick the axis start in the viewport (Esc to cancel)",
                [Some(_), None, _] => "Rotate3D: pick the axis end in the viewport (Esc to cancel)",
                [Some(_), Some(_), None] => {
                    "Rotate3D: pick the reference point in the viewport (Esc to cancel)"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Rotate3D: pick the target point in the viewport (Esc to cancel)"
                }
            },
            Self::Twist { points, .. } => match points {
                [None, _, _] => "Twist: pick the axis start (Esc to cancel)",
                [Some(_), None, _] => "Twist: pick the axis end (Esc to cancel)",
                [Some(_), Some(_), None] => {
                    "Twist: enter an angle or pick a reference point; Copy, Rigid, Infinite, PreserveStructure"
                }
                [Some(_), Some(_), Some(_)] => {
                    "Twist: pick the target reference point (Esc to cancel)"
                }
            },
            Self::Bend { points, options } => match points {
                [None, _] => "Bend: pick the spine start (Esc to cancel)",
                [Some(_), None] => "Bend: pick the spine end (Esc to cancel)",
                [Some(_), Some(_)] if matches!(options.angle, Some(angle) if angle != 0.) => {
                    "Bend: pick the bend direction; Copy, Rigid, Angle, Symmetric, PreserveStructure, NonAttenuated"
                }
                [Some(_), Some(_)] => {
                    "Bend: pick the through point; Copy, Rigid, LimitToSpine, Angle, Symmetric, PreserveStructure, NonAttenuated"
                }
            },
            Self::Maelstrom {
                center,
                initial,
                target,
                ..
            } => match (center, initial, target) {
                (None, _, _) => {
                    "Maelstrom: pick the center; Vertical, 2Point, 3Point (Esc to cancel)"
                }
                (Some(_), None, _) => {
                    "Maelstrom: enter or pick the first radius; Diameter, Orientation, Circumference, Area; Enter accepts the remembered radius"
                }
                (Some(_), Some(_), None) => {
                    "Maelstrom: enter or pick the second radius; Copy, Rigid"
                }
                (Some(_), Some(_), Some(_)) => {
                    "Maelstrom: enter or pick the coil angle; Copy, Rigid"
                }
            },
            Self::Stretch { points, factor, .. } => match (points, factor) {
                ([None, _], _) => "Stretch: pick the axis start; Copy, Rigid (Esc to cancel)",
                ([Some(_), None], None) => "Stretch: pick the axis end or enter a factor",
                ([Some(_), None], Some(_)) => "Stretch: pick the stretch direction",
                _ => "Stretch: enter a factor or pick the target; Length, Copy, Rigid",
            },
            Self::Taper {
                points, initial, ..
            } => match (points, initial) {
                ([None, _], _) => "Taper: pick the axis start (Esc to cancel)",
                ([Some(_), None], _) => "Taper: pick the axis end (Esc to cancel)",
                ([Some(_), Some(_)], None) => {
                    "Taper: enter or pick the start distance; Copy, Rigid, Flat, Infinite, PreserveStructure"
                }
                ([Some(_), Some(_)], Some(_)) => {
                    "Taper: enter or pick the end distance; Copy, Rigid, Flat, Infinite, PreserveStructure"
                }
            },
            Self::Mirror { start: None } => {
                "Mirror: pick the start, or choose 3Point, XAxis, YAxis, ZAxis, Object (Esc to cancel)"
            }
            Self::Mirror { start: Some(_) } => {
                "Mirror: pick the second axis point in the viewport (Esc to cancel)"
            }
            Self::MirrorThreePoint { points } => match points {
                [None, _] => "Mirror 3Point: pick the plane origin (Esc to cancel)",
                [Some(_), None] => "Mirror 3Point: pick the second plane point (Esc to cancel)",
                [Some(_), Some(_)] => "Mirror 3Point: pick the third plane point (Esc to cancel)",
            },
            Self::MirrorObject => "Mirror Object: pick a planar surface or face (Esc to cancel)",
            Self::Shear { origin: None, .. } => {
                "Shear: pick the fixed origin in the viewport (Esc to cancel)"
            }
            Self::Shear {
                origin: Some(_),
                reference: None,
            } => "Shear: pick the reference direction in the viewport (Esc to cancel)",
            Self::Shear {
                reference: Some(_), ..
            } => "Shear: pick the target angle in the viewport (Esc to cancel)",
            Self::ExtrudeCurve { base: None, .. } => {
                "ExtrudeCrv: pick the direction base point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurve { base: Some(_), .. } => {
                "ExtrudeCrv: pick the direction target point in the viewport (Esc to cancel)"
            }
            Self::ExtrudeCurveTapered { base: None, .. } => {
                "ExtrudeCrvTapered: enter a distance or pick two direction points (DraftAngle, Solid, Corners, DeleteInput, FlipAngle)"
            }
            Self::ExtrudeCurveTapered { base: Some(_), .. } => {
                "ExtrudeCrvTapered: pick the direction target (Esc to cancel)"
            }
            Self::ExtrudeSurface { base: None, .. } => {
                "ExtrudeSrf: enter a distance or pick two direction points (BothSides, Solid, DeleteInput, Direction)"
            }
            Self::ExtrudeSurface { base: Some(_), .. } => {
                "ExtrudeSrf: pick the direction target (Esc to cancel)"
            }
            Self::ExtrudeSurfaceTapered { base: None, .. } => {
                "ExtrudeSrfTapered: enter a distance or pick two direction points (DraftAngle, Solid, Corners, DeleteInput, FlipAngle)"
            }
            Self::ExtrudeSurfaceTapered { base: Some(_), .. } => {
                "ExtrudeSrfTapered: pick the direction target (Esc to cancel)"
            }
            Self::ExtrudeSurfaceToPoint { .. } => {
                "ExtrudeSrfToPoint: pick the apex (Solid, DeleteInput, SplitAtTangents; Esc to cancel)"
            }
            Self::ExtrudeCurveToPoint { .. } => {
                "ExtrudeCrvToPoint: pick the apex in the viewport (Esc to cancel)"
            }
            Self::Revolve {
                axis_start: None, ..
            } => "Revolve: pick the axis start in the viewport (Esc to cancel)",
            Self::RailRevolve(state) if state.axis_start.is_none() => {
                "RailRevolve: pick the axis start (ScaleHeight, SplitAtTangents; Esc cancels)"
            }
            Self::RailRevolve(_) => {
                "RailRevolve: pick the axis end or press Enter for CPlane Z (Esc cancels)"
            }
            Self::Revolve {
                axis_start: Some(_),
                ..
            } => "Revolve: pick the axis end in the viewport (Esc to cancel)",
        }
    }
}
