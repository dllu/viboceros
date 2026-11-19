cmake_minimum_required(VERSION 3.20)
project(viboceros_smlib LANGUAGES CXX)
find_package(TBB REQUIRED COMPONENTS tbb tbbmalloc)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
# Match upstream rounding on ARM64 for the kernel, inline headers and probes.
if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64)$")
    add_compile_options(-ffp-contract=off)
endif()
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
file(GLOB KERNEL_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SMLib/src/*.cpp")
file(GLOB API_SOURCES CONFIGURE_DEPENDS "${USD_BREP_ROOT}/source/SM_API/src/*.cpp")
list(FILTER KERNEL_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")
list(FILTER API_SOURCES EXCLUDE REGEX "/StdAfx\\.cpp$")
include(usd_brep_patches.cmake)
include("${usd_brep_patches}/usd-brep-basis-evaluation.cmake")
include("${usd_brep_patches}/usd-brep-point-interpolation.cmake")
include("${usd_brep_patches}/usd-brep-curve-intersection-distance.cmake")
include("${usd_brep_patches}/usd-brep-curve-self-intersection-pairs.cmake")
include("${usd_brep_patches}/usd-brep-curve-contact-tolerance.cmake")
include("${usd_brep_patches}/usd-brep-copy-basis.cmake")
include("${usd_brep_patches}/usd-brep-exact-uv-transpose.cmake")
include("${usd_brep_patches}/usd-brep-isocurve-pairs.cmake")
include("${usd_brep_patches}/usd-brep-surface-patch.cmake")
include("${usd_brep_patches}/usd-brep-curve-continuity.cmake")
set(kernel_source "${USD_BREP_ROOT}/source/SMLib/src")
set(kernel_include "${USD_BREP_ROOT}/source/SMLib/inc")

usd_brep_patch_source("${kernel_source}/SmProtoTopology.cpp"
    proto-edge-endpoint-links "${usd_brep_patched}/SmProtoTopologyLinked.cpp")

usd_brep_patch_source("${kernel_source}/SmBrep.cpp"
    blend-rail-parameterization "${usd_brep_patched}/SmBrep.cpp")
usd_brep_patch_source("${usd_brep_patched}/SmBrep.cpp"
    isolated-loop-owner "${usd_brep_patched}/SmBrepLoopOwner.cpp")
usd_brep_patch_source("${usd_brep_patched}/SmBrepLoopOwner.cpp"
    protected-topology "${usd_brep_patched}/SmBrepProtected.cpp")

usd_brep_patch_source("${kernel_source}/SmSurface.cpp"
    blend-derivative-scaling "${usd_brep_patched}/SmSurface.cpp")
usd_brep_patch_source("${usd_brep_patched}/SmSurface.cpp"
    planarity-zone "${usd_brep_patched}/SmSurfacePlanar.cpp")

usd_brep_patch_source("${kernel_source}/SmPrimitiveCreation.cpp"
    tapered-sliver-domains "${usd_brep_patched}/SmPrimitiveCreation.cpp")
usd_brep_patch_source("${kernel_source}/NL_CrvAdv.cpp"
    extension-break-allocation "${usd_brep_patched}/NL_CrvAdv.cpp")

include("${usd_brep_patches}/usd-brep-curve-segment-limits.cmake")
include("${usd_brep_patches}/usd-brep-exact-split.cmake")
include("${usd_brep_patches}/usd-brep-tensor-bezier.cmake")
include("${usd_brep_patches}/usd-brep-surface-fillet.cmake")

usd_brep_patch_source("${kernel_source}/SmCurveFilletExecutive.cpp"
    curve-fillet-sides "${usd_brep_patched}/SmCurveFilletExecutive.cpp")
usd_brep_patch_source("${usd_brep_patched}/SmCurveFilletExecutive.cpp"
    curve-fillet-seed "${usd_brep_patched}/SmCurveFilletSeeded.cpp")
usd_brep_apply_patch("${kernel_include}/SmCurveFilletExecutive.h"
    curve-fillet-seed-header "${usd_brep_patched}/SmCurveFilletExecutive.h")

usd_brep_patch_source("${kernel_source}/SmSurfaceDropCurve.cpp"
    surface-drop-start-normal "${usd_brep_patched}/SmSurfaceDropCurve.cpp")
include("${usd_brep_patches}/usd-brep-drop-curve-accuracy.cmake")

usd_brep_patch_source("${kernel_source}/SmFilletStandardSolver.cpp"
    g2-blend-rail-parameterization "${usd_brep_patched}/SmFilletStandardSolver.cpp")
usd_brep_patch_source("${kernel_source}/SmFilletCorner.cpp"
    closed-fillet-corner-state "${usd_brep_patched}/SmFilletCorner.cpp")
usd_brep_patch_source("${usd_brep_patched}/SmFilletCorner.cpp"
    required-ball-corner "${usd_brep_patched}/SmFilletCornerBall.cpp")
include("${usd_brep_patches}/usd-brep-circular-fillet-rail-parameters.cmake")

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
add_executable(viboceros_smlib_circular_fillet_rails_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/circular_fillet_rails_probe.cpp")
target_link_libraries(viboceros_smlib_circular_fillet_rails_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_circular_fillet_rails_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_circular_fillet_rails_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/circular_fillet_rails_probe.cpp"
    "${circular_fillet_source}")
target_link_libraries(viboceros_smlib_circular_fillet_rails_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_circular_fillet_rails_baseline PRIVATE -Wno-write-strings
    -include "${USD_BREP_ROOT}/source/SMLib/inc/StdAfx.h")
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
add_executable(viboceros_smlib_curve_continuity_probe "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_continuity_probe.cpp")
target_link_libraries(viboceros_smlib_curve_continuity_probe PRIVATE SMLib)
target_compile_options(viboceros_smlib_curve_continuity_probe PRIVATE -Wall -Wextra -Werror)
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
    "${kernel_source}/NL_CrvTool.cpp")
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
add_executable(viboceros_smlib_surface_pull_baseline EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_pull_probe.cpp" "${kernel_source}/SmSurfaceDropCurve.cpp")
target_link_libraries(viboceros_smlib_surface_pull_baseline PRIVATE SM_API)
target_compile_options(viboceros_smlib_surface_pull_baseline PRIVATE -Wno-write-strings)

add_executable(viboceros_smlib_planarity_probe EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/planarity_zone_probe.cpp")
target_link_libraries(viboceros_smlib_planarity_probe PRIVATE SM_API)
target_compile_options(viboceros_smlib_planarity_probe PRIVATE -Wall -Wextra -Werror)
add_executable(viboceros_smlib_planarity_baseline EXCLUDE_FROM_ALL "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/planarity_zone_probe.cpp" "${usd_brep_patched}/SmSurface.cpp")
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

add_executable(viboceros_smlib_surface_fillet_parents_probe EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/surface_fillet_parents_probe.cpp")
target_link_libraries(viboceros_smlib_surface_fillet_parents_probe PRIVATE viboceros_smlib)
target_include_directories(viboceros_smlib_surface_fillet_parents_probe PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(viboceros_smlib_surface_fillet_parents_probe PRIVATE -Wall -Wextra -Werror)

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
    foreach(instrumented_target viboceros_smlib_curve_continuity_probe viboceros_smlib_surface_fillet_parents_probe viboceros_smlib_circular_fillet_rails_probe viboceros_smlib_drop_curve_probe viboceros_smlib_curve_contact_probe viboceros_smlib_surface_fillet_probe viboceros_smlib_tensor_bezier_probe SMLib SM_API viboceros_smlib viboceros_smlib_isocurve_pair_probe viboceros_smlib_surface_patch_probe viboceros_smlib_curve_segment_probe viboceros_smlib_basis_evaluation_probe viboceros_smlib_point_interpolation_probe viboceros_smlib_exact_split_probe)
        target_compile_options(${instrumented_target} PRIVATE -fsanitize=address -g -fno-omit-frame-pointer)
        target_link_libraries(${instrumented_target} PUBLIC asan)
    endforeach()
endif()
