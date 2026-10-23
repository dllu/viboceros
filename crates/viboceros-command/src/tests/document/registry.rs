//! document registry command regressions.
use super::*;

#[test]
fn dispatches_case_insensitively_and_accepts_rhino_prefix() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "_pT 1,2,3").unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn creates_lines_from_comma_or_space_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 3,4,0").unwrap();
    registry.execute(&mut document, "Line 0 0 0 0 0 2").unwrap();
    assert_eq!(document.objects().len(), 2);
}

#[test]
fn failed_command_does_not_mutate_document() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(registry.execute(&mut document, "Line 0,0,0 0,0,0").is_err());
    assert_eq!(document.objects().len(), 0);
}

#[test]
fn reports_sorted_help() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert_eq!(
        registry.execute(&mut document, "Help").unwrap(),
        "Commands: 3DFace, AddNextU, AddNextV, AddNgonsToMesh, AddObjectsToBlock, AddPrevU, AddPrevV, AddToGroup, Align, AlignVertices, Angle, ApplyCrv, Arc, Area, AreaCentroid, Array, ArrayCrv, ArrayCrvOnSrf, ArrayLinear, ArrayPolar, ArraySrf, Bend, Blend, BlendEdge, BlendSrf, Block, BlockEdit, BlockResetScale, Boolean2Objects, BooleanDifference, BooleanIntersection, BooleanSplit, BooleanUnion, BoundingBox, Box, Cap, Catenary, Chamfer, ChamferEdge, ChangeDegree, ChangeLayer, Check, Circle, Clear, CloseCrv, CollapseMeshEdge, CombineIdenticalMeshVertices, Cone, Conic, Connect, Contour, ControlPointCurve, ConvertToBeziers, ConvertToSingleSpans, Copy, CopyToLayer, Crease, CreateUniqueBlock, CreateUVCrv, CrvDeviation, CrvEnd, CrvSeam, CrvStart, CullDegenerateMeshFaces, CullUnusedMeshVertices, Curvature, CurvatureGraph, CurvatureGraphOff, Curve, CurveThroughPolyline, CurveThroughPt, Cylinder, Delete, DeleteFaces, DeleteMeshNgons, DeleteSubCrv, Diameter, Dir, Distance, Distribute, Divide, DivideAlongCreases, Domain, DupBorder, DupEdge, DupFaceBorder, DupMeshEdge, DupMeshHoleBoundary, EdgeSrf, EditPtOn, Ellipse, Ellipsoid, EvaluatePt, EvaluateUVPt, Explode, ExplodeBlock, Export3dm, ExportStep, ExportStl, Extend, ExtendSrf, ExtractConnectedMeshFaces, ExtractControlPolygon, ExtractCurvatureGraph, ExtractDuplicateMeshFaces, ExtractIsocurve, ExtractMeshEdges, ExtractMeshFaces, ExtractMeshFacesByArea, ExtractMeshFacesByAspectRatio, ExtractMeshFacesByDraftAngle, ExtractMeshFacesByEdgeLength, ExtractMeshPart, ExtractNonManifoldMeshEdges, ExtractPt, ExtractSrf, ExtractSubCrv, ExtractWireframe, ExtrudeCrv, ExtrudeCrvAlongCrv, ExtrudeCrvTapered, ExtrudeCrvToPoint, ExtrudeMesh, ExtrudeSrf, ExtrudeSrfAlongCrv, ExtrudeSrfTapered, ExtrudeSrfToPoint, Fillet, FilletCorners, FilletEdge, FilletSrf, FillMeshHole, FillMeshHoles, FitCrv, Flip, Flow, FlowAlongSrf, GCon, GetUserText, Group, Helix, Hide, HidePt, HideSwap, Hyperbola, Import3dm, ImportStep, ImportStl, Insert, InsertControlPoint, InsertEditPoint, InsertKink, InsertKnot, InsertPoint, InterpCrv, InterpCrvOnSrf, Intersect, IntersectSelf, IntersectTwoSets, Invert, InvertPt, IPlane, Isolate, IsolateLock, Join, JoinCopy, Layer, Length, Line, LineThroughPt, Lock, LockSwap, Loft, Maelstrom, MakeNonPeriodic, MakePeriodic, MakeUniform, MakeUniformUV, MarkFoci, Match, MatchCrvDir, MatchMeshEdge, MergeAllCoplanarFaces, MergeAllEdges, MergeCoplanarFace, MergeEdge, Mesh, MeshBox, MeshCone, MeshCylinder, MeshEllipsoid, MeshFromLines, MeshPatch, MeshPlane, MeshPolyline, MeshSphere, MeshToNURB, MeshTorus, MeshTruncatedCone, Mirror, ModifyRadius, Move, NextU, NextV, Offset, OffsetCrvOnSrf, OffsetMesh, OffsetMultiple, OffsetSrf, Open3dm, Orient, Orient3Pt, OrientCrvToEdge, OrientOnCrv, OrientOnSrf, Parabola, Parabola3Pt, Paraboloid, PatchSingleFace, Pipe, PlanarDifference, PlanarIntersection, PlanarMesh, PlanarSrf, PlanarUnion, PlaneThroughPt, Point, PointCloud, PointDeviation, PointGrid, Points, PointsOff, PointsOn, Polygon, PolygonCount, Polyline, PolylineOnMesh, PrevU, PrevV, Project, ProjectToCPlane, Pull, Pyramid, QuadrangulateMesh, Radius, RailRevolve, Rebuild, RebuildMesh, RebuildMeshNormals, Rectangle, Redo, ReducePointCloud, ReducePolyline, RemapCPlane, RememberCopyOptions, RemoveControlPoint, RemoveCrease, RemoveFromGroup, RemoveKnot, RemoveMultiKnot, Reparameterize, ReplaceBlock, Revolve, Rotate, Rotate3D, SaveAs, Scale, Scale1D, Scale2D, ScaleByPlane, ScaleNU, ScalePositions, Section, SelAll, SelBadObjects, SelBox, SelClosedCrv, SelClosedMesh, SelClosedPolysrf, SelClosedSrf, SelClosedSubD, SelColor, SelCrv, SelDup, SelDupAll, SelGroup, SelID, SelKey, SelKeyValue, SelLast, SelLayer, SelLayerNumber, SelLine, SelMesh, SelName, SelNone, SelNonManifold, SelOpenCrv, SelOpenMesh, SelOpenPolysrf, SelOpenSrf, SelOpenSubD, SelPlanarCrv, SelPlanarSrf, SelPolyline, SelPolysrf, SelPrev, SelPt, SelPtCloud, SelSelfIntersectingCrv, SelShortCrv, SelSmall, SelSrf, SelSubD, SelTrimmedSrf, SelU, SelUntrimmedSrf, SelUV, SelV, SelValue, SelVolumeObject, SelVolumePipe, SelVolumeSphere, SetObjectColor, SetObjectName, SetPt, SetUserText, Shear, Shell, ShortPath, Show, ShowEdges, ShowEdgesOff, ShowPt, ShowSelected, ShrinkTrimmedSrf, ShrinkTrimmedSrfToEdge, Silhouette, SimplifyCrv, Smooth, SoftEditCrv, SoftEditSrf, Sphere, Spiral, Split, SplitDisjointMesh, SplitEdge, SplitMeshEdge, Splop, SrfControlPtGrid, SrfPt, SrfPtGrid, SrfSeam, Stretch, SubCrv, SubDBox, SubDCone, SubDCrease, SubDCylinder, SubDEllipsoid, SubDivide, SubDPlane, SubDSphere, SubDSpinEdge, SubDTorus, SubDTruncatedCone, SwapMeshEdge, Sweep1, Sweep2, Taper, Tolerance, ToNURBS, Torus, ToSubD, TriangulateMesh, TriangulateNonPlanarQuads, Trim, TruncatedCone, TruncatedPyramid, Tube, TweenCurves, TweenSurfaces, Twist, Undo, Ungroup, UngroupAll, UnifyMeshNormals, Unisolate, UnisolateLock, Units, UnjoinEdge, Unlock, UnlockSelected, Untrim, UntrimAll, UntrimBorder, UntrimHoles, Unweld, UnweldEdge, UnweldVertex, Volume, VolumeCentroid, Weight, Weld, WeldEdge, WeldVertices, ZoomNaked, ZoomNonManifold"
    );
}
