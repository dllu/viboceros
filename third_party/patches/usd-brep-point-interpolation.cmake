find_program(PATCH_EXECUTABLE patch REQUIRED)
set(fit_source "${USD_BREP_ROOT}/source/SMLib/src/NL_CrvFit.cpp")
set(fit_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-point-interpolation.patch")
set(fit_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvFit.cpp")
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${fit_source}" "${fit_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${fit_output}" "${fit_source}" "${fit_patch}"
                RESULT_VARIABLE fit_patch_status)
if(NOT fit_patch_status EQUAL 0)
    message(FATAL_ERROR "usd-brep point interpolation repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${fit_source}")
list(APPEND KERNEL_SOURCES "${fit_output}")

# Replace only the fitting producer object for the opt-in control executable.
add_executable(viboceros_smlib_point_interpolation_baseline EXCLUDE_FROM_ALL
    "${CMAKE_CURRENT_SOURCE_DIR}/../../../tools/usd_brep/point_interpolation_probe.cpp"
    "${fit_source}")
target_link_libraries(viboceros_smlib_point_interpolation_baseline PRIVATE SMLib)
target_compile_definitions(viboceros_smlib_point_interpolation_baseline PRIVATE VIBOCEROS_POINT_INTERPOLATION_BASELINE=1)
