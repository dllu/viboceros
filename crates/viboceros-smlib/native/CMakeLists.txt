cmake_minimum_required(VERSION 3.20)
project(viboceros_smlib LANGUAGES CXX)
find_package(TBB REQUIRED COMPONENTS tbb tbbmalloc)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
file(GLOB KERNEL_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SMLib/src/*.cpp")
file(GLOB API_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SM_API/src/*.cpp")
list(FILTER KERNEL_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")
list(FILTER API_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-basis-evaluation.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-point-interpolation.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-intersection-distance.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-self-intersection-pairs.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-contact-tolerance.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-copy-basis.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-exact-uv-transpose.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-isocurve-pairs.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-patch.cmake")
# Keep upstream immutable. Compile the bounded kernel repairs from generated
# copies, with the source/version and patches retained under third_party.
find_program(PATCH_EXECUTABLE patch REQUIRED)
set(PROTO_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmProtoTopology.cpp")
set(PROTO_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-interior-point.patch")
set(PROTO_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmProtoTopology.cpp")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${PROTO_SOURCE}" "${PROTO_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch -o "${PROTO_PATCHED}" "${PROTO_SOURCE}" "${PROTO_PATCH}"
                RESULT_VARIABLE PROTO_PATCH_STATUS OUTPUT_VARIABLE PROTO_PATCH_OUTPUT ERROR_VARIABLE PROTO_PATCH_ERROR)
if(NOT PROTO_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep topology patch failed; review the pinned upstream version: ${PROTO_PATCH_OUTPUT} ${PROTO_PATCH_ERROR}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${PROTO_SOURCE}")
list(APPEND KERNEL_SOURCES "${PROTO_PATCHED}")
# Preserve endpoint adjacency when combining temporary intersection edges.
set(PROTO_LINK_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-proto-edge-endpoint-links.patch")
set(PROTO_LINK_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmProtoTopologyLinked.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${PROTO_LINK_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${PROTO_LINK_PATCHED}" "${PROTO_PATCHED}" "${PROTO_LINK_PATCH}" RESULT_VARIABLE PROTO_LINK_STATUS)
if(NOT PROTO_LINK_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep proto-edge endpoint patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${PROTO_PATCHED}")
list(APPEND KERNEL_SOURCES "${PROTO_LINK_PATCHED}")

set(BLEND_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmBrep.cpp")
set(BLEND_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-blend-rail-parameterization.patch")
set(BLEND_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBrep.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${BLEND_SOURCE}" "${BLEND_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch -o "${BLEND_PATCHED}" "${BLEND_SOURCE}" "${BLEND_PATCH}"
                RESULT_VARIABLE BLEND_PATCH_STATUS OUTPUT_VARIABLE BLEND_PATCH_OUTPUT ERROR_VARIABLE BLEND_PATCH_ERROR)
if(NOT BLEND_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep blend rail patch failed; review the pinned version: ${BLEND_PATCH_OUTPUT} ${BLEND_PATCH_ERROR}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${BLEND_SOURCE}")
list(APPEND KERNEL_SOURCES "${BLEND_PATCHED}")
set(LOOP_OWNER_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-isolated-loop-owner.patch")
set(LOOP_OWNER_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBrepLoopOwner.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${LOOP_OWNER_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${LOOP_OWNER_PATCHED}" "${BLEND_PATCHED}" "${LOOP_OWNER_PATCH}" RESULT_VARIABLE LOOP_OWNER_STATUS)
if(NOT LOOP_OWNER_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep isolated loop owner patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${BLEND_PATCHED}")
list(APPEND KERNEL_SOURCES "${LOOP_OWNER_PATCHED}")

# Respect protected faces/vertices before querying mutable coincidence helpers.
set(PROTECTED_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-protected-topology.patch")
set(PROTECTED_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBrepProtected.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${PROTECTED_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${PROTECTED_PATCHED}" "${LOOP_OWNER_PATCHED}" "${PROTECTED_PATCH}" RESULT_VARIABLE PROTECTED_PATCH_STATUS)
if(NOT PROTECTED_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep protected topology repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${LOOP_OWNER_PATCHED}")
list(APPEND KERNEL_SOURCES "${PROTECTED_PATCHED}")

set(DERIV_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmSurface.cpp")
set(DERIV_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-blend-derivative-scaling.patch")
set(DERIV_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmSurface.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${DERIV_SOURCE}" "${DERIV_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch -o "${DERIV_PATCHED}" "${DERIV_SOURCE}" "${DERIV_PATCH}"
                RESULT_VARIABLE DERIV_PATCH_STATUS OUTPUT_VARIABLE DERIV_PATCH_OUTPUT ERROR_VARIABLE DERIV_PATCH_ERROR)
if(NOT DERIV_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep blend derivative patch failed; review the pinned version: ${DERIV_PATCH_OUTPUT} ${DERIV_PATCH_ERROR}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${DERIV_SOURCE}")
list(APPEND KERNEL_SOURCES "${DERIV_PATCHED}")

set(PLANAR_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-planarity-zone.patch")
set(PLANAR_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmSurfacePlanar.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${PLANAR_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${PLANAR_PATCHED}" "${DERIV_PATCHED}" "${PLANAR_PATCH}" RESULT_VARIABLE PLANAR_PATCH_STATUS)
if(NOT PLANAR_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep planarity zone repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${DERIV_PATCHED}")
list(APPEND KERNEL_SOURCES "${PLANAR_PATCHED}")
# Measure each ruling rail over its own domain when rejecting sliver faces.
set(TAPER_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmPrimitiveCreation.cpp")
set(TAPER_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-tapered-sliver-domains.patch")
set(TAPER_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmPrimitiveCreation.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${TAPER_SOURCE}" "${TAPER_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${TAPER_PATCHED}" "${TAPER_SOURCE}" "${TAPER_PATCH}"
                RESULT_VARIABLE TAPER_PATCH_STATUS)
if(NOT TAPER_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep tapered sliver-domain patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${TAPER_SOURCE}")
list(APPEND KERNEL_SOURCES "${TAPER_PATCHED}")
set(EXTEND_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/NL_CrvAdv.cpp")
set(EXTEND_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-extension-break-allocation.patch")
set(EXTEND_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvAdv.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${EXTEND_SOURCE}" "${EXTEND_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch -o "${EXTEND_PATCHED}" "${EXTEND_SOURCE}" "${EXTEND_PATCH}"
                RESULT_VARIABLE EXTEND_PATCH_STATUS OUTPUT_VARIABLE EXTEND_PATCH_OUTPUT ERROR_VARIABLE EXTEND_PATCH_ERROR)
if(NOT EXTEND_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep extension allocation patch failed: ${EXTEND_PATCH_OUTPUT} ${EXTEND_PATCH_ERROR}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${EXTEND_SOURCE}")
list(APPEND KERNEL_SOURCES "${EXTEND_PATCHED}")
set(REMOVE_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/NL_CrvTool.cpp")
set(REMOVE_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-knot-removal-bound-error.patch")
set(REMOVE_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvTool.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${REMOVE_SOURCE}" "${REMOVE_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch -o "${REMOVE_PATCHED}" "${REMOVE_SOURCE}" "${REMOVE_PATCH}"
                RESULT_VARIABLE REMOVE_PATCH_STATUS)
if(NOT REMOVE_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep knot-removal bound patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${REMOVE_SOURCE}")
list(APPEND KERNEL_SOURCES "${REMOVE_PATCHED}")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-segment-limits.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-exact-split.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-tensor-bezier.cmake")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-fillet.cmake")

set(CURVE_FILLET_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmCurveFilletExecutive.cpp")
set(CURVE_FILLET_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-fillet-sides.patch")
set(CURVE_FILLET_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveFilletExecutive.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CURVE_FILLET_SOURCE}" "${CURVE_FILLET_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${CURVE_FILLET_PATCHED}" "${CURVE_FILLET_SOURCE}" "${CURVE_FILLET_PATCH}"
                RESULT_VARIABLE CURVE_FILLET_PATCH_STATUS)
if(NOT CURVE_FILLET_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep curve fillet side patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${CURVE_FILLET_SOURCE}")
list(APPEND KERNEL_SOURCES "${CURVE_FILLET_PATCHED}")

set(CURVE_FILLET_SEED_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-fillet-seed.patch")
set(CURVE_FILLET_SEEDED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveFilletSeeded.cpp")
set(CURVE_FILLET_HEADER "${USD_BREP_ROOT}/source/SMLib/inc/SmCurveFilletExecutive.h")
set(CURVE_FILLET_HEADER_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-fillet-seed-header.patch")
set(CURVE_FILLET_HEADER_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveFilletExecutive.h")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${CURVE_FILLET_SEED_PATCH}" "${CURVE_FILLET_HEADER}" "${CURVE_FILLET_HEADER_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${CURVE_FILLET_SEEDED}" "${CURVE_FILLET_PATCHED}" "${CURVE_FILLET_SEED_PATCH}" RESULT_VARIABLE FILLET_SEED_STATUS)
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${CURVE_FILLET_HEADER_PATCHED}" "${CURVE_FILLET_HEADER}" "${CURVE_FILLET_HEADER_PATCH}" RESULT_VARIABLE FILLET_HEADER_STATUS)
if(NOT FILLET_SEED_STATUS EQUAL 0 OR NOT FILLET_HEADER_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep seeded fillet patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${CURVE_FILLET_PATCHED}")
list(APPEND KERNEL_SOURCES "${CURVE_FILLET_SEEDED}")

# Off-surface normal drops must seed with normal solutions, including when
# a source interval starts above a finite surface boundary.
set(DROP_SOURCE "${USD_BREP_ROOT}/source/SMLib/src/SmSurfaceDropCurve.cpp")
set(DROP_PATCH "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-surface-drop-start-normal.patch")
set(DROP_PATCHED "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmSurfaceDropCurve.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${DROP_SOURCE}" "${DROP_PATCH}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${DROP_PATCHED}" "${DROP_SOURCE}" "${DROP_PATCH}" RESULT_VARIABLE DROP_PATCH_STATUS)
if(NOT DROP_PATCH_STATUS EQUAL 0)
    message(FATAL_ERROR "usd-brep surface normal-drop seed patch failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${DROP_SOURCE}")
list(APPEND KERNEL_SOURCES "${DROP_PATCHED}")
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-drop-curve-accuracy.cmake")

# Keep G2 rail/corner repairs in generated copies of the pinned source.
foreach(repair IN ITEMS
    "SmFilletStandardSolver.cpp|usd-brep-g2-blend-rail-parameterization.patch"
    "SmFilletCorner.cpp|usd-brep-closed-fillet-corner-state.patch")
    string(REPLACE "|" ";" repair_parts "${repair}")
    list(GET repair_parts 0 repair_source_name)
    list(GET repair_parts 1 repair_patch_name)
    set(repair_source "${USD_BREP_ROOT}/source/SMLib/src/${repair_source_name}")
    set(repair_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/${repair_patch_name}")
    set(repair_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/${repair_source_name}")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${repair_source}" "${repair_patch}")
    execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${repair_output}" "${repair_source}" "${repair_patch}" RESULT_VARIABLE repair_status)
    if(NOT repair_status EQUAL 0)
        message(FATAL_ERROR "usd-brep repair ${repair_patch_name} failed; review the pinned source")
    endif()
    list(REMOVE_ITEM KERNEL_SOURCES "${repair_source}")
    list(APPEND KERNEL_SOURCES "${repair_output}")
endforeach()

add_library(SMLib STATIC ${KERNEL_SOURCES})
target_include_directories(SMLib BEFORE PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
target_compile_definitions(SMLib PUBLIC VIBOCEROS_CURVE_INTERSECTION_DISTANCE_PATCH=1)
target_include_directories(SMLib SYSTEM PUBLIC "${USD_BREP_ROOT}/source/SMLib/inc")
target_compile_definitions(SMLib PUBLIC NOMINMAX SM_GRAPHICS_CALLBACKS=1)
target_compile_options(SMLib PRIVATE -Wno-write-strings)
target_link_libraries(SMLib PRIVATE TBB::tbb TBB::tbbmalloc)
add_library(SM_API STATIC ${API_SOURCES})
target_include_directories(SM_API SYSTEM PUBLIC "${USD_BREP_ROOT}/source/SM_API/inc")
target_compile_definitions(SM_API PRIVATE SM_API_EXPORTS)
target_compile_options(SM_API PRIVATE -Wno-write-strings)
target_link_libraries(SM_API PUBLIC SMLib)
add_library(viboceros_smlib STATIC surface_fillet.cpp bridge.cpp basis_preservation.cpp brep_export.cpp brep_import.cpp boundary_validation.cpp coplanar_merge.cpp face_creases.cpp knot_refinement.cpp network_fit.cpp)
target_link_libraries(viboceros_smlib PUBLIC SM_API)
target_compile_options(viboceros_smlib PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_surface_blend_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_blend_probe.cpp")
target_link_libraries(viboceros_smlib_surface_blend_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_blend_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_surface_blend_curved_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_blend_curved_probe.cpp")
target_link_libraries(viboceros_smlib_surface_blend_curved_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_blend_curved_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_isocurve_pair_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/isocurve_pair_probe.cpp")
target_link_libraries(viboceros_smlib_isocurve_pair_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_isocurve_pair_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_surface_patch_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_patch_probe.cpp")
target_link_libraries(viboceros_smlib_surface_patch_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_patch_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_curve_segment_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_segment_limits_probe.cpp")
target_link_libraries(viboceros_smlib_curve_segment_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_curve_segment_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_basis_evaluation_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/basis_evaluation_probe.cpp")
target_link_libraries(viboceros_smlib_basis_evaluation_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_basis_evaluation_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_point_interpolation_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/point_interpolation_probe.cpp")
target_link_libraries(viboceros_smlib_point_interpolation_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_point_interpolation_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_tensor_bezier_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/tensor_bezier_probe.cpp")
target_link_libraries(viboceros_smlib_tensor_bezier_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_tensor_bezier_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_tensor_bezier_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/tensor_bezier_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolSplits.cpp")
target_link_libraries(viboceros_smlib_tensor_bezier_baseline PRIVATE SMLib)
target_compile_options(viboceros_smlib_tensor_bezier_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
add_executable(viboceros_smlib_exact_split_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/exact_split_probe.cpp")
target_link_libraries(viboceros_smlib_exact_split_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_exact_split_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_curve_segment_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_segment_limits_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvTool.cpp")
target_link_libraries(viboceros_smlib_curve_segment_baseline PRIVATE SMLib)
target_compile_options(viboceros_smlib_curve_segment_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
add_executable(viboceros_smlib_surface_patch_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_patch_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfTool.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBSplineSurfaceIso.cpp")
target_link_libraries(viboceros_smlib_surface_patch_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_patch_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
# Override only the two producer objects to reproduce the pinned parent behavior.
add_executable(viboceros_smlib_isocurve_pair_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/isocurve_pair_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBSplineSurfaceUV.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/SmIsoCurve.cpp")
target_link_libraries(viboceros_smlib_isocurve_pair_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_isocurve_pair_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
# Keep the repaired object/UV producers and override only NLib extraction.
add_executable(viboceros_smlib_isocurve_limits_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/isocurve_pair_probe.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/NL_SrfTool.cpp")
target_link_libraries(viboceros_smlib_isocurve_limits_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_isocurve_limits_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
install(TARGETS viboceros_smlib SM_API SMLib ARCHIVE DESTINATION lib)
add_executable(viboceros_smlib_copy_basis_probe EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/copy_basis_probe.cpp")
target_link_libraries(viboceros_smlib_copy_basis_probe PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_copy_basis_probe PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_copy_basis_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_copy_basis_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/copy_basis_probe.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/SmBSplineCurve.cpp"
    "${USD_BREP_ROOT}/source/SMLib/src/SmBSplineSurface.cpp")
target_link_libraries(viboceros_smlib_copy_basis_baseline PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_copy_basis_baseline PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_copy_basis_baseline PRIVATE -Wno-write-strings -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
# Reproduction targets are opt-in. The baseline object overrides the matching
# static archive member, isolating this repair while keeping other fixes equal.
add_executable(viboceros_smlib_surface_pull_probe EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_pull_probe.cpp")
target_link_libraries(viboceros_smlib_surface_pull_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_pull_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_surface_pull_baseline EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_pull_probe.cpp" "${DROP_SOURCE}")
target_link_libraries(viboceros_smlib_surface_pull_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_pull_baseline PRIVATE -Wno-write-strings)

add_executable(viboceros_smlib_planarity_probe EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/planarity_zone_probe.cpp")
target_link_libraries(viboceros_smlib_planarity_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_planarity_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_planarity_baseline EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/planarity_zone_probe.cpp" "${DERIV_PATCHED}")
target_link_libraries(viboceros_smlib_planarity_baseline PRIVATE SM_API)
# Generated kernel sources need the kernel precompiled-header includes; the
# API library also exports a different StdAfx.h directory.
target_compile_options(viboceros_smlib_planarity_baseline PRIVATE -Wno-write-strings -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")

add_executable(viboceros_smlib_curve_contact_probe EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_contact_probe.cpp")
target_link_libraries(viboceros_smlib_curve_contact_probe PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_curve_contact_probe PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_curve_contact_probe PRIVATE -Wall -Wextra -Werror)

add_executable(viboceros_smlib_curve_contact_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_contact_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveSelfPairs.cpp")
target_link_libraries(viboceros_smlib_curve_contact_baseline PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_curve_contact_baseline PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_curve_contact_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")

add_executable(viboceros_smlib_surface_fillet_probe EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_fillet_probe.cpp")
target_link_libraries(viboceros_smlib_surface_fillet_probe PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_surface_fillet_probe PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_surface_fillet_probe PRIVATE -Wall -Wextra -Werror)

add_executable(viboceros_smlib_surface_fillet_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_fillet_probe.cpp"
    "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletExecutiveBaseline.cpp")
target_link_libraries(viboceros_smlib_surface_fillet_baseline PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_surface_fillet_baseline PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_surface_fillet_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")

# Direct UV-drop qualification and the pinned producer for an identical baseline.
foreach(drop_target IN ITEMS probe baseline)
    add_executable(viboceros_smlib_drop_curve_${drop_target} EXCLUDE_FROM_ALL
        "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/drop_curve_accuracy_probe.cpp")
    target_link_libraries(viboceros_smlib_drop_curve_${drop_target} PRIVATE viboceros_smlib)
    target_include_directories(viboceros_smlib_drop_curve_${drop_target} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
    target_compile_options(viboceros_smlib_drop_curve_${drop_target} PRIVATE -Wno-write-strings
        -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
endforeach()
target_sources(viboceros_smlib_drop_curve_baseline PRIVATE "${drop_accuracy_source}")

# Optional instrumentation of both complete native libraries and the C ABI.
option(VIBOCEROS_SMLIB_ASAN "Instrument native geometry with AddressSanitizer" OFF)
if(VIBOCEROS_SMLIB_ASAN)
    foreach(instrumented_target viboceros_smlib_drop_curve_probe viboceros_smlib_curve_contact_probe viboceros_smlib_surface_fillet_probe viboceros_smlib_tensor_bezier_probe SMLib SM_API viboceros_smlib viboceros_smlib_isocurve_pair_probe viboceros_smlib_surface_patch_probe viboceros_smlib_curve_segment_probe viboceros_smlib_basis_evaluation_probe viboceros_smlib_point_interpolation_probe viboceros_smlib_exact_split_probe)
        target_compile_options(${instrumented_target} PRIVATE -fsanitize=address -g -fno-omit-frame-pointer)
        target_link_libraries(${instrumented_target} PUBLIC asan)
    endforeach()
endif()
