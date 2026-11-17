# Keep generated replacements out of the upstream checkout. Some probes use
# intermediate copies, so each patch in a chain has its own output.
find_program(PATCH_EXECUTABLE patch REQUIRED)
set(usd_brep_patches "${CMAKE_CURRENT_LIST_DIR}/../../../third_party/patches")
set(usd_brep_patched "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
file(MAKE_DIRECTORY "${usd_brep_patched}")

function(usd_brep_apply_patch source patch output)
    set(patch "${usd_brep_patches}/usd-brep-${patch}.patch")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${source}" "${patch}")
    execute_process(
        COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${output}.tmp" "${source}" "${patch}"
        RESULT_VARIABLE status OUTPUT_VARIABLE detail ERROR_VARIABLE error
    )
    if(NOT status STREQUAL "0")
        message(FATAL_ERROR "Failed to apply ${patch} to ${source}:\n${detail}${error}")
    endif()
    execute_process(
        COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${output}.tmp" "${output}"
        COMMAND_ERROR_IS_FATAL ANY
    )
    file(REMOVE "${output}.tmp")
endfunction()

function(usd_brep_patch_source source patch output)
    usd_brep_apply_patch("${source}" "${patch}" "${output}")
    list(REMOVE_ITEM KERNEL_SOURCES "${source}")
    list(APPEND KERNEL_SOURCES "${output}")
    set(KERNEL_SOURCES "${KERNEL_SOURCES}" PARENT_SCOPE)
endfunction()
