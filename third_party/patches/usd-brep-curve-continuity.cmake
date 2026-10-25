# Use geometric curvature, independent of parameter speed and acceleration.
find_program(PATCH_EXECUTABLE patch REQUIRED)
set(continuity_source "${USD_BREP_ROOT}/source/SMLib/src/SmGeomUtility.cpp")
set(continuity_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-continuity.patch")
set(continuity_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmGeomUtilityContinuity.cpp")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${continuity_source}" "${continuity_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${continuity_output}" "${continuity_source}" "${continuity_patch}"
                RESULT_VARIABLE continuity_patch_status)
if(NOT continuity_patch_status EQUAL 0)
    message(FATAL_ERROR "usd-brep curve continuity repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${continuity_source}")
list(APPEND KERNEL_SOURCES "${continuity_output}")

add_executable(viboceros_smlib_curve_continuity_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/curve_continuity_probe.cpp"
    "${continuity_source}")
target_link_libraries(viboceros_smlib_curve_continuity_baseline PRIVATE SMLib)
