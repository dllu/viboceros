//! Shared command failures, kept outside the dispatcher and command implementations.
use super::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("select one editable mesh or SubD for point insertion")]
    InsertPointUnavailable,
    #[error("insertion source, editability or tolerance changed; select again")]
    InsertPointStale,
    #[error("edge point insertion exceeds the staged resource budget")]
    InsertPointResourceLimit,
    #[error("select an available interior SubD edge")]
    SpinEdgeUnavailable,
    #[error("SubD spin source, editability or tolerance changed; select again")]
    SpinEdgeStale,
    #[error("SubD edge spin exceeds the staged session resource budget")]
    SpinEdgeResourceLimit,
    #[error("SubD spin edge {edge} is outside a source with {count} edges")]
    SpinEdgeOutOfRange { edge: usize, count: usize },
    #[error("select mesh edges or SubD edges and vertices")]
    CreaseUnavailable,
    #[error("crease input geometry, tolerance or editability changed; select again")]
    CreaseStale,
    #[error("crease edit exceeds the component or geometry resource budget")]
    CreaseResourceLimit,
    #[error("crease {component} {index} is outside its source table of {count} components")]
    CreaseComponentOutOfRange {
        component: &'static str,
        index: usize,
        count: usize,
    },
    #[error("selected input is not a convertible mesh, surface, extrusion or B-rep face")]
    ToSubDUnavailable,
    #[error("ToSubD output exceeds the conversion resource budget")]
    ToSubDResourceLimit,
    #[error("ToSubD source geometry, tolerance or editability changed; select again")]
    ToSubDStale,
    #[error("select available mesh or SubD objects or faces")]
    SubdivisionUnavailable,
    #[error("subdivision source or tolerance changed; select the targets again")]
    SubdivisionStale,
    #[error("subdivision exceeded its total vertex or face budget")]
    SubdivisionResourceLimit,
    #[error("subdivision face {face} is outside a source with {face_count} faces")]
    SubdivisionFaceIndexOutOfRange { face: usize, face_count: usize },
    #[error("FilletSrf: {0}")]
    SurfaceFillet(&'static str),
    #[error(
        "Flow Local requires LocalRadii=inner,outer; interactive input can enter or pick both radii"
    )]
    FlowLocalRadiiRequired,
    #[error("FlowAlongSrf references must be NURBS surfaces or explicit B-rep faces")]
    SurfaceFlowNotSurface,
    #[error(
        "FlowAlongSrf ConstrainNormal requires Normal=x,y,z; interactive input can pick a direction or use CPlane Z"
    )]
    SurfaceFlowNormalRequired,
    #[error("Flow TargetSurface requires a NURBS surface or an explicit B-rep face")]
    FlowTargetSurfaceNotSurface,
    #[error("Silhouette requires the native kernel; build with --features native-smlib")]
    SilhouetteRequiresNativeKernel,
    #[error("Mesh Boolean commands require the native kernel; build with --features native-smlib")]
    MeshBooleanRequiresNativeKernel,
    #[error("MeshIntersect requires the native kernel; build with --features native-smlib")]
    MeshIntersectRequiresNativeKernel,
    #[error("MeshSelfIntersect requires the native kernel; build with --features native-smlib")]
    MeshSelfIntersectRequiresNativeKernel,
    #[error("MeshSplit requires the native kernel; build with --features native-smlib")]
    MeshSplitRequiresNativeKernel,
    #[error("MeshTrim requires the native kernel; build with --features native-smlib")]
    MeshTrimRequiresNativeKernel,
    #[error("Mesh Boolean commands require meshes, surfaces or polysurfaces in the requested sets")]
    MeshBooleanRequiresObjects,
    #[error("Mesh Boolean operation produced no result")]
    MeshBooleanNoResult,
    #[error("ExtrudeCrvTapered requires the native kernel; build with --features native-smlib")]
    TaperedExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrf requires the native kernel; build with --features native-smlib")]
    SurfaceExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfAlongCrv requires the native kernel; build with --features native-smlib")]
    SurfacePathExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfToPoint requires the native-smlib kernel feature")]
    SurfaceApexExtrusionRequiresNativeKernel,
    #[error("ExtrudeSrfTapered requires the native-smlib kernel feature")]
    SurfaceTaperedExtrusionRequiresNativeKernel,
    #[error("Shell requires the native-smlib kernel feature")]
    ShellRequiresNativeKernel,
    #[error("FilletEdge requires the native kernel; build with --features native-smlib")]
    FilletEdgeRequiresNativeKernel,
    #[error("ChamferEdge requires the native kernel; build with --features native-smlib")]
    ChamferEdgeRequiresNativeKernel,
    #[error("Coplanar B-rep face merging requires the native-smlib feature")]
    CoplanarMergeRequiresNativeKernel,
    #[error("Coplanar face merging requires meshes, surfaces or polysurfaces")]
    UnsupportedCoplanarMergeGeometry,
    #[error("BlendEdge requires the native-smlib feature")]
    BlendEdgeRequiresNativeKernel,
    #[cfg(feature = "native-smlib")]
    #[error(transparent)]
    NativeSmlib(#[from] viboceros_smlib::Error),
    #[error("Contour exceeds the limit of 100000 planes; increase spacing or restrict Range")]
    ContourPlaneLimit,
    #[error("Contour plane origins cannot be represented distinctly as finite coordinates")]
    ContourUnrepresentablePlane,
    #[error("block definition '{0}' was not found")]
    BlockNameNotFound(String),
    #[error("Boolean difference produced no changes")]
    NothingSubtracted,

    #[error("BooleanSplit did not split any target")]
    NothingSplit,
    #[error(
        "BooleanIntersection requires a first set and either a second set or at least two intersecting objects"
    )]
    BooleanIntersectionRequiresObjects,
    #[error("Boolean intersection produced no changes")]
    NothingIntersected,
    #[error("BooleanUnion requires at least two surfaces or polysurfaces")]
    BooleanUnionRequiresTwoObjects,
    #[error("Boolean union produced no changes")]
    NothingUnioned,
    #[error("Match requires exactly two selected open curves")]
    MatchRequiresTwoOpenCurves,
    #[error("GCon requires exactly two selected open curves")]
    GConRequiresTwoOpenCurves,
    #[error("PointCloud creation requires point or mesh sources")]
    PointCloudRequiresSources,
    #[error("select one point cloud or specify Target=<id>")]
    PointCloudTargetRequired,
    #[error("a selected point cloud requires PointCloud Add or PointCloud Remove")]
    PointCloudEditActionRequired,
    #[error("multiple point clouds selected; specify Target=<id>")]
    PointCloudTargetAmbiguous,
    #[error("PointCloud Add requires selected point or point cloud sources")]
    PointCloudRequiresAddSources,
    #[error("PointCloud Remove index is outside the target cloud")]
    PointCloudIndexOutOfRange,
    #[error("Curvature requires only curve or surface objects selected")]
    CurvatureRequiresCurveOrSurface,
    #[error("the curvature location is outside the selected trimmed faces")]
    CurvatureOutsideTrimmedFaces,
    #[error("Loft requires only curve objects selected in profile order")]
    LoftRequiresCurves,
    #[error("Flow guides must be curves or explicit surface edges")]
    FlowGuideNotCurve,
    #[error(
        "Flow spatial guides require explicit Roadlike axes; TargetSurface can supply target orientation"
    )]
    FlowSpatialFramesNotSupported,
    #[error(
        "Flow Roadlike requires explicit BaseUp and TargetUp directions; interactive input can pick axes or use CPlaneZ"
    )]
    FlowRoadlikeAxesRequired,
    #[error("Flow option {0} is not implemented yet")]
    FlowOptionNotSupported(&'static str),
    #[error("enter a command")]
    EmptyInput,

    #[error("unknown command '{0}'; enter Help to list commands")]
    UnknownCommand(String),

    #[error("command name or alias '{0}' is already registered")]
    DuplicateCommand(String),

    #[error("usage: {0}")]
    Usage(&'static str),

    #[error("alignment target must be a curve")]
    AlignmentTargetNotCurve,

    #[error("alignment target must not be one of the objects being aligned")]
    AlignmentTargetSelected,

    #[error("'{0}' is not a valid finite number")]
    InvalidNumber(String),

    #[error("'{0}' is not a valid non-negative integer")]
    InvalidInteger(String),

    #[error("'{0}' is not a valid positive mesh-plane face count")]
    InvalidMeshPlaneFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-box face count")]
    InvalidMeshBoxFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-cylinder vertical face count")]
    InvalidMeshCylinderVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-cylinder around face count of at least 3")]
    InvalidMeshCylinderAroundFaceCount(String),

    #[error("'{0}' is not a valid positive mesh-cone vertical face count")]
    InvalidMeshConeVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-cone around face count of at least 3")]
    InvalidMeshConeAroundFaceCount(String),

    #[error("'{0}' is not a valid positive mesh truncated-cone vertical face count")]
    InvalidMeshTruncatedConeVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh truncated-cone around face count of at least 3")]
    InvalidMeshTruncatedConeAroundFaceCount(String),

    #[error("mesh-sphere style '{0}' is not supported; use UV, Quads, or Triangles")]
    UnsupportedMeshSphereStyle(String),

    #[error("'{0}' is not a valid mesh-sphere vertical face count of at least 2")]
    InvalidMeshSphereVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-sphere around face count of at least 3")]
    InvalidMeshSphereAroundFaceCount(String),

    #[error("'{0}' is not a valid mesh-ellipsoid vertical face count of at least 2")]
    InvalidMeshEllipsoidVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-ellipsoid around face count of at least 3")]
    InvalidMeshEllipsoidAroundFaceCount(String),

    #[error("'{value}' is not a valid mesh-sphere subdivision count from 0 through {maximum}")]
    InvalidMeshSphereSubdivisionCount { value: String, maximum: usize },

    #[error("'{0}' is not a valid mesh-torus vertical face count of at least 3")]
    InvalidMeshTorusVerticalFaceCount(String),

    #[error("'{0}' is not a valid mesh-torus around face count of at least 3")]
    InvalidMeshTorusAroundFaceCount(String),

    #[error("'{0}' is not a valid r,g,b color with components from 0 through 255")]
    InvalidColor(String),

    #[error("'{0}' is not a valid finite, non-zero scale factor")]
    InvalidScaleFactor(String),

    #[error("'{0}' is not a valid array item count of 2 or more")]
    InvalidArrayItemCount(String),

    #[error("'{0}' is not a valid curve-array item count of 1 or more")]
    InvalidCurveArrayItemCount(String),

    #[error("'{0}' is not a valid finite, strictly positive curve-array distance")]
    InvalidCurveArrayDistance(String),

    #[error("'{0}' is not a valid surface-array dimension count of 1 or more")]
    InvalidSurfaceArrayCount(String),

    #[error("no object named '{0}' was found for the surface-array target")]
    SurfaceArrayTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-array target")]
    AmbiguousSurfaceArrayTarget(String),

    #[error("ArraySrf requires a named target surface or a selected surface as the last object")]
    SurfaceArrayTargetRequired,

    #[error("the ArraySrf target must be an untrimmed NURBS surface")]
    SurfaceArrayTargetNotSurface,

    #[error("ArraySrf requires at least one selected source object besides the target surface")]
    SurfaceArraySourcesRequired,

    #[error("no object named '{0}' was found for the surface-orient target")]
    SurfaceOrientTargetNotFound(String),

    #[error("more than one object named '{0}' could be the surface-orient target")]
    AmbiguousSurfaceOrientTarget(String),

    #[error("OrientOnSrf requires a named target surface or a selected surface as the last object")]
    SurfaceOrientTargetRequired,

    #[error("the OrientOnSrf target must be a NURBS surface or an explicit B-rep face")]
    SurfaceOrientTargetNotSurface,

    #[error("OrientOnSrf requires at least one selected source object besides the target surface")]
    SurfaceOrientSourcesRequired,

    #[error("no object named '{0}' was found for the curve-array path")]
    CurveArrayPathNotFound(String),

    #[error("more than one object named '{0}' could be the curve-array path")]
    AmbiguousCurveArrayPath(String),

    #[error("ArrayCrv requires a named path or a selected path as the last selected object")]
    CurveArrayPathRequired,

    #[error("the curve-array path must be a line, analytic curve, polyline, or NURBS curve")]
    CurveArrayPathNotCurve,

    #[error("ArrayCrv requires at least one selected source object besides the path")]
    CurveArraySourcesRequired,

    #[error("no object named '{0}' was found for the spiral rail")]
    SpiralPathNotFound(String),

    #[error("more than one object named '{0}' could be the spiral rail")]
    AmbiguousSpiralPath(String),

    #[error("Spiral AroundCurve requires a named rail or a selected rail")]
    SpiralPathRequired,

    #[error("the Spiral AroundCurve rail must be a line, analytic curve, polyline, or NURBS curve")]
    SpiralPathNotCurve,

    #[error("'{0}' is not a valid rectangular-array dimension count of 1 or more")]
    InvalidArrayDimensionCount(String),

    #[error("'{0}' is not a valid non-zero polar-array angle")]
    InvalidPolarArrayAngle(String),

    #[error("'{0}' is not a valid finite, strictly positive maximum curve length")]
    InvalidMaximumCurveLength(String),

    #[error("no layer named '{0}' was found")]
    NamedLayerNotFound(String),

    #[error("no group named '{0}' was found")]
    NamedGroupNotFound(String),

    #[error("no objects are selected")]
    NoObjectsSelected,

    #[error("CurveThroughPt requires at least two distinct selected point objects")]
    NotEnoughCurveThroughPoints,

    #[error("CurveThroughPolyline requires at least one selected polyline")]
    NoCurveThroughPolylines,

    #[error("TweenCurves requires exactly two selected curves, got {actual}")]
    TweenCurvesRequiresTwoCurves { actual: usize },

    #[error("FitCrv requires at least one selected curve")]
    NoFitCurves,

    #[error("Rebuild requires at least one selected curve")]
    NoRebuildCurves,

    #[error(
        "surface Rebuild requires single-face surfaces without a mixed curve/surface selection"
    )]
    UnsupportedSurfaceRebuild,

    #[error("Rebuild sources or document settings changed; select the surfaces again")]
    StaleSurfaceRebuild,
    #[error("Rebuild curve sources or settings changed; select the curves again")]
    StaleCurveRebuild,

    #[error("ChangeDegree degree {actual} must be between 1 and {maximum}")]
    InvalidChangeDegree { actual: usize, maximum: usize },

    #[error(
        "ChangeDegree requires at least one selected curve or untrimmed NURBS surface whose degree differs"
    )]
    NoChangeDegreeObjects,

    #[error("MakeUniform requires at least one selected curve or untrimmed NURBS surface")]
    NoMakeUniformObjects,

    #[error("MakeUniformUV requires at least one selected untrimmed NURBS surface")]
    NoMakeUniformUvSurfaces,

    #[error(
        "MakePeriodic requires at least one selected closed, non-periodic curve or NURBS surface direction of degree two or higher"
    )]
    NoMakePeriodicObjects,

    #[error("MakeNonPeriodic requires at least one selected periodic NURBS curve or surface")]
    NoPeriodicObjects,

    #[error(
        "InsertControlPoint requires exactly one selected curve or single surface, got {actual}"
    )]
    InsertControlPointRequiresOneObject { actual: usize },

    #[error("CrvSeam requires exactly one selected closed curve, got {actual}")]
    CurveSeamRequiresOneClosedCurve { actual: usize },

    #[error("SrfSeam requires exactly one selected untrimmed NURBS surface, got {actual}")]
    SurfaceSeamRequiresOneSurface { actual: usize },

    #[error("SrfSeam requires a surface with at least one closed parameter direction")]
    SurfaceSeamHasNoClosedDirection,

    #[error("SrfSeam Direction=Both requires a u,v Parameter value")]
    SurfaceSeamBothDirectionsRequireUv,

    #[error(
        "Reparameterize requires selectable curves or surface faces, got {actual}; use Face=index for a polysurface"
    )]
    ReparameterizeRequiresOneObject { actual: usize },
    #[error("Can only reparameterize curves or surfaces, not both")]
    ReparameterizeMixedGeometry,

    #[error("SubCrv requires exactly one selected curve, got {actual}")]
    SubcurveRequiresOneCurve { actual: usize },

    #[error("FilletCorners requires selected polylines, NURBS curves, or polycurves")]
    FilletCornersRequiresSupportedCurves,
    #[error("Fillet requires exactly two selected curves")]
    FilletRequiresTwoCurves,
    #[error("Chamfer requires exactly two selected curves")]
    ChamferRequiresTwoCurves,
    #[error("Connect requires exactly two selected curves")]
    ConnectRequiresTwoCurves,
    #[error("Blend requires exactly two selected curves")]
    BlendRequiresTwoCurves,

    #[error(
        "OffsetSrf requires planar surfaces or exact canonical spheres, cylinders, and tori, standalone or as supported single-face B-reps"
    )]
    OffsetSurfaceRequiresPlanarFaces,
    #[error("OffsetSrf Solid=Yes requires a single closed boundary loop per surface")]
    OffsetSurfaceSolidRequiresOneLoop,
    #[error("OffsetSrf sphere offset collapses or inverts the sphere")]
    OffsetSurfaceCollapsedSphere,
    #[error("OffsetSrf cylinder offset collapses or inverts the cylinder")]
    OffsetSurfaceCollapsedCylinder,
    #[error("OffsetSrf torus offset collapses or self-intersects the torus")]
    OffsetSurfaceCollapsedTorus,
    #[error("OffsetMesh requires selected meshes only")]
    UnsupportedOffsetMeshGeometry,
    #[error("mesh n-gon commands require selected meshes only")]
    UnsupportedMeshNgonGeometry,

    #[error("Extend requires exactly one selected curve, got {actual}")]
    ExtendRequiresOneCurve { actual: usize },

    #[error(
        "boundary Extend requires a picked open source curve and at least one other selected curve, surface, or B-rep boundary, got {actual} supported selected objects"
    )]
    ExtendRequiresSourceAndBoundaryObjects { actual: usize },

    #[error("boundary Extend could not find an open selected source curve")]
    NoOpenCurveToExtendToBoundary,

    #[error("ExtendSrf requires exactly one selected untrimmed NURBS surface, got {actual}")]
    ExtendSurfaceRequiresOneSurface { actual: usize },

    #[error("the selected surface has no non-singular open edge that ExtendSrf can extend")]
    NoExtendableSurfaceEdges,

    #[error("Split currently requires exactly one selected curve, got {actual}")]
    SplitRequiresOneCurve { actual: usize },

    #[error(
        "cutting-object Split requires a selected curve or rectangular surface source and at least one selected curve, surface, or B-rep cutter, got {actual} supported selected object(s)"
    )]
    SplitRequiresSourceAndCuttingObjects { actual: usize },

    #[error(
        "cutting-object Split requires a selected curve, NURBS surface, or rectangular single-face B-rep to use as the source"
    )]
    SplitRequiresCurveOrRectangularSurfaceSource,

    #[error(
        "surface cutting-object Split requires complete simple cuts on a supported rectangular surface parameterization"
    )]
    UnsupportedSurfaceCuttingIntersection,

    #[error(
        "isocurve Split requires exactly one selected NURBS surface or rectangular single-face B-rep, got {actual} supported target(s)"
    )]
    SplitIsocurveRequiresOneSurface { actual: usize },

    #[error("the isocurve Split point projects outside the selected face trim")]
    SplitIsocurvePointOutsideFaceTrim,

    #[error(
        "Intersect currently supports selected curves, untrimmed NURBS surfaces, and B-reps only"
    )]
    UnsupportedIntersectGeometry,

    #[error("Intersect requires at least two selected objects, got {actual}")]
    IntersectRequiresAtLeastTwoObjects { actual: usize },

    #[error("Intersect does not yet support surface/B-rep or B-rep/B-rep pairs")]
    UnsupportedIntersectObjectPair,

    #[error("Intersect supports at most {maximum} selected object pairs")]
    TooManyIntersectPairs { maximum: usize },

    #[error("Intersect supports at most {maximum} output objects")]
    TooManyIntersectOutputs { maximum: usize },

    #[error(
        "Trim requires a selected source curve and at least one selected curve, surface, or B-rep cutter, got {actual} supported selected object(s)"
    )]
    TrimRequiresSourceAndCuttingObjects { actual: usize },

    #[error("Trim requires at least one selected curve to use as the source")]
    TrimRequiresCurveSource,

    #[error("Trim supports at most {maximum} cutting intersections")]
    TooManyTrimIntersections { maximum: usize },

    #[error(
        "InsertKnot requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    InsertKnotRequiresOneObject { actual: usize },

    #[error("InsertKnot on a curve requires one scalar parameter, not a u,v pair")]
    InsertKnotCurveRequiresParameter,

    #[error("InsertKnot Direction=Both on a surface requires a u,v parameter pair")]
    InsertKnotBothDirectionsRequireUv,

    #[error(
        "RemoveKnot requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    RemoveKnotRequiresOneObject { actual: usize },

    #[error("RemoveKnot on a curve requires one scalar parameter, not a u,v pair")]
    RemoveKnotCurveRequiresParameter,

    #[error(
        "RemoveControlPoint requires exactly one selected curve or untrimmed NURBS surface, got {actual}"
    )]
    RemoveControlPointRequiresOneObject { actual: usize },

    #[error("no qualifying multiple knots were removed from the selected curves or surfaces")]
    NoMultipleKnotsRemoved,

    #[error("Join requires an open curve or a mesh")]
    NoOpenCurvesToJoin,

    #[error("Join requires an open surface or polysurface")]
    NoOpenSurfacesToJoin,

    #[error("no objects were joined")]
    NothingJoined,

    #[error(
        "Join requires one object family: curves, meshes, or surfaces/polysurfaces; mixed-family joining is not implemented"
    )]
    UnsupportedJoinGeometry,
    #[error("Cap supports selected surfaces, B-reps, and meshes")]
    UnsupportedCapGeometry,

    #[error("MergeAllEdges requires selected surfaces or polysurfaces")]
    UnsupportedMergeAllEdgesGeometry,

    #[error("MergeEdge requires an accessible surface or polysurface edge")]
    MergeEdgeUnavailable,
    #[error("MergeEdge source or tolerance changed; pick the edge again")]
    MergeEdgeStale,
    #[error("No certified merge is available for this edge and choice")]
    MergeEdgeChoiceUnavailable,

    #[error("SplitEdge requires an accessible surface or polysurface edge")]
    SplitEdgeUnavailable,
    #[error("SplitEdge source or tolerance changed; pick the edge again")]
    SplitEdgeStale,
    #[error("pick a point on the edge before entering a distance constraint")]
    SplitEdgeDistanceAnchor,

    #[error("UntrimHoles requires an accessible face with All=Yes or edge with All=No")]
    UntrimHolesUnavailable,
    #[error("Untrim requires an accessible surface or polysurface edge")]
    UntrimUnavailable,
    #[error("Untrim source or tolerance changed; pick the edge again")]
    UntrimStale,
    #[error("select exactly one eligible hole component")]
    UntrimHolesMultipleComponents,
    #[error("UntrimHoles source or tolerance changed; pick the component again")]
    UntrimHolesStale,
    #[error("UnjoinEdge requires accessible surface or polysurface edge components")]
    UnjoinEdgeUnavailable,
    #[error("UnjoinEdge source or tolerance changed; select the edges again")]
    UnjoinEdgeStale,
    #[error("ShrinkTrimmedSrf requires accessible surface or polysurface faces")]
    ShrinkTrimmedUnavailable,
    #[error("shrink source or tolerance changed; select surfaces or faces again")]
    ShrinkTrimmedStale,

    #[error("none of the selected objects can be exploded")]
    NoExplodableObjects,

    #[error("ExtractSubCrv supports selected polycurves and polylines only")]
    UnsupportedExtractSubcurveGeometry,

    #[error(
        "ExtractSubCrv segment index {index} is outside the selected object's {segment_count} segments"
    )]
    ExtractSubcurveIndexOutOfRange { index: usize, segment_count: usize },

    #[error("DupBorder supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedDuplicateBorderGeometry,

    #[error("none of the selected objects has an open border")]
    NoDuplicateBorders,

    #[error("DupEdge supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedDuplicateEdgeGeometry,

    #[error("DupEdge edge index {edge} is outside the selected object's edge count {edge_count}")]
    DuplicateEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("the requested location does not identify a duplicable edge")]
    NoDuplicateEdges,

    #[error("DupMeshEdge supports selected polygon meshes only")]
    UnsupportedDuplicateMeshEdgeGeometry,

    #[error("none of the selected meshes has a duplicable mesh edge")]
    NoDuplicateMeshEdges,

    #[error("DupMeshHoleBoundary supports selected polygon meshes only")]
    UnsupportedDuplicateMeshHoleBoundaryGeometry,

    #[error(
        "DupMeshHoleBoundary boundary index {boundary} is outside the selected mesh's closed boundary count {boundary_count}"
    )]
    DuplicateMeshHoleBoundaryIndexOutOfRange {
        boundary: usize,
        boundary_count: usize,
    },

    #[error("none of the selected meshes has a closed naked boundary")]
    NoDuplicateMeshHoleBoundaries,

    #[error("DupFaceBorder supports selected NURBS surfaces and B-reps only")]
    UnsupportedDuplicateFaceBorderGeometry,

    #[error(
        "DupFaceBorder face index {face} is outside the selected object's face count {face_count}"
    )]
    DuplicateFaceBorderFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested face has no duplicable border")]
    NoDuplicateFaceBorders,

    #[error("ExtractControlPolygon supports curves, single surfaces, and SubDs")]
    UnsupportedExtractControlPolygonGeometry,

    #[error("none of the selected objects has an extractable control polygon")]
    NoExtractableControlPolygons,

    #[error("control polygon input changed; select the source again")]
    ControlPolygonStale,

    #[error("ExtractSrf supports selected NURBS surfaces and B-reps only")]
    UnsupportedExtractSurfaceGeometry,
    #[error(
        "ExtractSrf sources, source order, attributes, tolerance, or output layer changed; select faces again"
    )]
    ExtractSurfaceStale,

    #[error("UntrimAll requires a standalone surface; whole polysurfaces are not accepted")]
    UnsupportedUntrimAllGeometry,

    #[error("UntrimBorder requires a standalone surface; whole polysurfaces are not accepted")]
    UnsupportedUntrimBorderGeometry,

    #[error(
        "ExtractSrf face index {face} is outside the selected object's face count {face_count}"
    )]
    ExtractSurfaceFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested location does not identify an extractable surface face")]
    NoExtractableSurfaces,

    #[error("ExtractIsocurve supports selected NURBS surfaces and B-reps only")]
    UnsupportedExtractIsocurveGeometry,

    #[error(
        "ExtractIsocurve face index {face} is outside the selected object's face count {face_count}"
    )]
    ExtractIsocurveFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("the requested surface location has no non-degenerate isocurve")]
    NoExtractableIsocurves,

    #[error(
        "EvaluateUVPt face index {face} is outside the selected object's face count {face_count}"
    )]
    EvaluateUvFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("ExtractWireframe supports selected NURBS surfaces, B-reps, and triangle meshes only")]
    UnsupportedExtractWireframeGeometry,

    #[error("none of the selected objects has an extractable wireframe")]
    NoExtractableWireframe,

    #[error("ConvertToSingleSpans requires a selected surface or single-face B-rep")]
    UnsupportedConvertToSingleSpansGeometry,

    #[error("ConvertToBeziers requires a selected curve or surface")]
    UnsupportedConvertToBeziersGeometry,

    #[error("{command} would create more than {maximum} output objects")]
    TooManySpanOutputObjects {
        command: &'static str,
        maximum: usize,
    },

    #[error("Length supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedLengthGeometry,

    #[error("Area supports selected closed planar curves, NURBS surfaces, B-reps, and meshes only")]
    UnsupportedAreaGeometry,

    #[error("Volume supports selected meshes, NURBS surfaces, and exact B-reps only")]
    UnsupportedVolumeGeometry,

    #[error(
        "open objects only define volume when they jointly enclose it; confirm with Continue=Yes or decline with Continue=No"
    )]
    OpenVolumeConfirmationRequired,

    #[error("operation declined")]
    OperationDeclined,

    #[error("Divide supports selected lines, analytic curves, polylines, and NURBS curves only")]
    UnsupportedDivideGeometry,

    #[error("the requested division creates no point objects")]
    NoCurveDivisionPoints,

    #[error("the requested Dir mode is unsupported for one or more selected object types")]
    UnsupportedDirectionEditGeometry,

    #[error("UnifyMeshNormals requires selected mesh objects")]
    UnsupportedUnifyMeshNormalsGeometry,
    #[error("RebuildMeshNormals requires selected mesh objects")]
    UnsupportedRebuildMeshNormalsGeometry,
    #[error("RebuildMesh requires selected mesh objects")]
    UnsupportedRebuildMeshGeometry,

    #[error("Weld supports selected meshes only")]
    UnsupportedWeldGeometry,

    #[error("none of the selected meshes contains vertices to weld or compact")]
    NoMeshVerticesToWeld,

    #[error("WeldEdge supports mesh and SubD edges only")]
    UnsupportedWeldEdgeGeometry,

    #[error("WeldEdge edge index {edge} is outside the selected mesh's edge count {edge_count}")]
    WeldEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges requires welding or compaction")]
    NoSelectedMeshEdgesToWeld,

    #[error("WeldVertices supports mesh and SubD vertices only")]
    UnsupportedWeldVerticesGeometry,

    #[error(
        "WeldVertices vertex index {vertex} is outside the selected object's vertex count {vertex_count}"
    )]
    WeldVertexIndexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("none of the selected mesh vertices requires welding or compaction")]
    NoSelectedMeshVerticesToWeld,

    #[error("Unweld supports selected meshes only")]
    UnsupportedUnweldGeometry,

    #[error("none of the selected meshes contains edges to unweld or unused vertices to compact")]
    NoMeshEdgesToUnweld,

    #[error("UnweldEdge supports mesh and SubD edges only")]
    UnsupportedUnweldEdgeGeometry,

    #[error("UnweldEdge edge index {edge} is outside the selected mesh's edge count {edge_count}")]
    UnweldEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges requires unwelding or compaction")]
    NoSelectedMeshEdgesToUnweld,

    #[error("UnweldVertex supports mesh and SubD vertices only")]
    UnsupportedUnweldVertexGeometry,

    #[error(
        "UnweldVertex vertex index {vertex} is outside the selected object's vertex count {vertex_count}"
    )]
    UnweldVertexIndexOutOfRange { vertex: usize, vertex_count: usize },

    #[error("none of the selected mesh vertices requires unwelding or compaction")]
    NoSelectedMeshVerticesToUnweld,

    #[error("vertex seam geometry, tolerance or editability changed; select again")]
    VertexSeamStale,

    #[error("vertex seam edit exceeds the component or geometry resource budget")]
    VertexSeamResourceLimit,

    #[error("edge seam geometry, tolerance or editability changed; select again")]
    EdgeSeamStale,

    #[error("edge seam edit exceeds the component or geometry resource budget")]
    EdgeSeamResourceLimit,

    #[error("CombineIdenticalMeshVertices supports selected meshes only")]
    UnsupportedCombineIdenticalMeshVerticesGeometry,

    #[error("none of the selected meshes contains identical vertex locations")]
    NoIdenticalMeshVertices,

    #[error("CullUnusedMeshVertices supports selected meshes only")]
    UnsupportedCullUnusedMeshVerticesGeometry,

    #[error("none of the selected meshes contains unused vertices")]
    NoUnusedMeshVertices,

    #[error("SplitDisjointMesh supports selected meshes only")]
    UnsupportedSplitDisjointMeshGeometry,

    #[error("none of the selected meshes contains multiple edge-connected pieces")]
    NoDisjointMeshes,

    #[error("ExtractMeshEdges supports selected polygon meshes only")]
    UnsupportedExtractMeshEdgesGeometry,

    #[error("none of the selected meshes has an edge matching the extraction filter")]
    NoExtractableMeshEdges,

    #[error("ExtractMeshFaces supports selected meshes only")]
    UnsupportedExtractMeshFacesGeometry,

    #[error("ExtractConnectedMeshFaces supports selected meshes only")]
    UnsupportedExtractConnectedMeshFacesGeometry,

    #[error("none of the selected meshes has connected faces to extract")]
    NoConnectedMeshFaces,

    #[error("the connected mesh faces have no border curves")]
    NoConnectedMeshFaceBorders,

    #[error("ExtractMeshPart supports selected meshes only")]
    UnsupportedExtractMeshPartGeometry,

    #[error("the selected mesh parts have no border curves")]
    NoMeshPartBorders,

    #[error("ExtractMeshFacesByArea supports selected meshes only")]
    UnsupportedExtractMeshFacesByAreaGeometry,

    #[error("no selected mesh faces have area in the requested range")]
    NoMeshFacesInAreaRange,

    #[error("the selected mesh faces have no border curves")]
    NoMeshFaceAreaBorders,

    #[error("ExtractMeshFacesByAspectRatio supports selected meshes only")]
    UnsupportedExtractMeshFacesByAspectRatioGeometry,

    #[error("no selected mesh faces exceed the requested aspect ratio")]
    NoMeshFacesAboveAspectRatio,

    #[error("the selected mesh faces have no aspect-ratio border curves")]
    NoMeshFaceAspectRatioBorders,

    #[error("ExtractMeshFacesByDraftAngle supports selected meshes only")]
    UnsupportedExtractMeshFacesByDraftAngleGeometry,

    #[error("no selected mesh faces have a draft angle in the requested range")]
    NoMeshFacesInDraftAngleRange,

    #[error("the selected mesh faces have no draft-angle border curves")]
    NoMeshFaceDraftAngleBorders,

    #[error("ExtractMeshFacesByEdgeLength supports selected meshes only")]
    UnsupportedExtractMeshFacesByEdgeLengthGeometry,

    #[error("no selected mesh faces have an edge in the requested length range")]
    NoMeshFacesInEdgeLengthRange,

    #[error("the selected mesh faces have no edge-length border curves")]
    NoMeshFaceEdgeLengthBorders,

    #[error(
        "ExtractMeshFaces face index {face} is outside the selected mesh's face count {face_count}"
    )]
    ExtractMeshFacesFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("DeleteFaces supports selected meshes and B-reps only")]
    UnsupportedDeleteFacesGeometry,

    #[error(
        "DeleteFaces face index {face} is outside the selected object's face count {face_count}"
    )]
    DeleteFacesFaceIndexOutOfRange { face: usize, face_count: usize },

    #[error("no deletable face was found at the requested location")]
    NoDeleteFaceAtLocation,

    #[error("TriangulateMesh supports selected meshes only")]
    UnsupportedTriangulateMeshGeometry,
    #[error("AlignVertices supports selected meshes only")]
    UnsupportedAlignVerticesGeometry,
    #[error("MatchMeshEdge supports selected meshes only")]
    UnsupportedMatchMeshEdgeGeometry,
    #[error("TriangulateNonPlanarQuads supports selected meshes only")]
    UnsupportedTriangulateNonPlanarQuadsGeometry,

    #[error("QuadrangulateMesh supports selected meshes only")]
    UnsupportedQuadrangulateMeshGeometry,

    #[error("SwapMeshEdge supports selected meshes only")]
    UnsupportedSwapMeshEdgeGeometry,

    #[error(
        "SwapMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    SwapMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges can be swapped")]
    NoSwappableMeshEdges,

    #[error("CollapseMeshEdge supports selected meshes only")]
    UnsupportedCollapseMeshEdgeGeometry,

    #[error(
        "CollapseMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    CollapseMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("none of the selected mesh edges can be collapsed")]
    NoCollapsibleMeshEdges,

    #[error("SplitMeshEdge supports selected meshes only")]
    UnsupportedSplitMeshEdgeGeometry,

    #[error(
        "SplitMeshEdge edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    SplitMeshEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("SplitMeshEdge parameter {0} must lie in [0, 1]")]
    InvalidMeshEdgeSplitParameter(Real),

    #[error("none of the selected mesh edges can be split")]
    NoSplittableMeshEdges,

    #[error("FillMeshHole supports selected meshes only")]
    UnsupportedFillMeshHoleGeometry,
    #[error("PatchSingleFace supports selected meshes only")]
    UnsupportedPatchSingleFaceGeometry,
    #[error("ExtrudeMesh supports selected meshes only")]
    UnsupportedExtrudeMeshGeometry,
    #[error("selected mesh components cannot form a single patch face")]
    NoPatchableMeshFace,

    #[error(
        "FillMeshHole edge index {edge} is outside the selected mesh's edge count {edge_count}"
    )]
    FillMeshHoleEdgeIndexOutOfRange { edge: usize, edge_count: usize },

    #[error("no selected mesh has a fillable closed naked boundary")]
    NoFillableMeshHoles,

    #[error("FillMeshHoles supports selected meshes only")]
    UnsupportedFillMeshHolesGeometry,

    #[error("ExtractNonManifoldMeshEdges supports selected meshes only")]
    UnsupportedExtractNonManifoldGeometry,

    #[error("none of the selected meshes has faces on a qualifying non-manifold edge")]
    NoNonManifoldMeshFaces,

    #[error("ExtractDuplicateMeshFaces supports selected meshes only")]
    UnsupportedExtractDuplicateMeshFacesGeometry,

    #[error("none of the selected meshes contains duplicate faces")]
    NoDuplicateMeshFaces,

    #[error(
        "CrvStart and CrvEnd support selected lines, analytic curves, polylines, and NURBS curves only"
    )]
    UnsupportedCurveEndpointGeometry,

    #[error("none of the selected objects has extractable defining points")]
    NoExtractablePoints,

    #[error("ExtractPt would extract more than {maximum} points")]
    TooManyExtractedPoints { maximum: usize },

    #[error(
        "ExtractPt component is unavailable, has an invalid index, or belongs to an unsupported object"
    )]
    ExtractPointComponentUnavailable,

    #[error("PointGrid supports at most {maximum} output points")]
    TooManyPointGridPoints { maximum: usize },

    #[error("the array would create more than {maximum} object copies")]
    TooManyArrayObjects { maximum: usize },

    #[error("CloseCrv requires curve geometry")]
    UnsupportedCloseCurveGeometry,

    #[error("Offset requires selected line, circle, circular arc, or planar polyline curves")]
    UnsupportedOffsetGeometry,

    #[error("ToNURBS requires a curve, surface, B-rep, or mesh")]
    UnsupportedToNurbsGeometry,

    #[error("ToNURBS would create more than {maximum} NURBS control points")]
    TooManyNurbsConversionControls { maximum: usize },

    #[error("none of the selected objects is an extrudable curve")]
    NoExtrudableCurves,

    #[error("none of the selected curves is a closed, nondegenerate planar boundary")]
    NoPlanarSurfaceBoundaries,

    #[error("Mesh supports selected NURBS surfaces and B-reps only")]
    UnsupportedMeshGeometry,

    #[error("MeshToNURB requires at least one selected polygon mesh")]
    UnsupportedMeshToNurbGeometry,

    #[error("MeshToNURB would create more than {maximum} NURBS control points")]
    TooManyMeshNurbsControls { maximum: usize },

    #[error("ExtrudeCrv currently supports Output=Surface only")]
    UnsupportedCurveExtrusionOutput,

    #[error("ExtrudeCrvToPoint currently supports Output=Surface only")]
    UnsupportedCurveToPointExtrusionOutput,

    #[error("the two Box base corners must lie on the same World XY plane")]
    BoxBaseCornersNotCoplanar,

    #[error("BoundingBox requires extents in at least two coordinate directions")]
    DegenerateBoundingBox,

    #[error("BoundingBox failed after reporting {reported_boxes} valid box(es): {reports}")]
    PartialBoundingBox {
        reported_boxes: usize,
        reports: String,
    },

    #[error("Distribute requires at least three independent objects or groups; found {actual}")]
    InsufficientDistributionObjects { actual: usize },

    #[error("Align ToFitPlane requires at least three selected objects; found {actual}")]
    InsufficientPlaneAlignmentObjects { actual: usize },

    #[error("ExtrudeCrvAlongCrv currently supports Output=Surface only")]
    UnsupportedCurveAlongCurveOutput,

    #[error(
        "ExtrudeCrvAlongCrv requires a named path or a selected path as the last selected object"
    )]
    CurveAlongCurvePathRequired,

    #[error("no object named '{0}' was found for the extrusion path")]
    CurveAlongCurvePathNotFound(String),

    #[error("more than one object named '{0}' could be the extrusion path")]
    AmbiguousCurveAlongCurvePath(String),

    #[error("the curve-along-curve extrusion path is not a supported curve")]
    CurveAlongCurvePathNotCurve,

    #[error("ExtrudeCrvAlongCrv requires at least one selected profile besides the path")]
    CurveAlongCurveProfilesRequired,

    #[error("none of the selected profile objects is an extrudable curve")]
    NoCurveAlongCurveProfiles,

    #[error("none of the selected objects is a revolvable curve")]
    NoRevolvableCurves,

    #[error("Revolve currently supports Output=Surface only")]
    UnsupportedRevolveOutput,

    #[error("deformable Revolve output is not yet supported")]
    DeformableRevolveUnsupported,

    #[error("'{0}' is not a valid non-zero revolution angle from -360 through 360 degrees")]
    InvalidRevolutionAngle(String),

    #[error(
        "the document contains no visible mesh, NURBS surface, or tessellatable B-rep to export"
    )]
    NoMeshToExport,

    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error(transparent)]
    Stl(#[from] StlError),

    #[error(transparent)]
    Step(#[from] StepError),

    #[error(transparent)]
    ThreeDm(#[from] ThreeDmError),

    #[error(transparent)]
    Document(#[from] DocumentError),
}
