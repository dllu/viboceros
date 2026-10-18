# Copies must preserve valid source bases. Keep all constructors from raw data
# and their existing sanitization unchanged, and compile only generated copies.
find_program(PATCH_EXECUTABLE patch REQUIRED)
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
foreach(copy_kind IN ITEMS Curve Surface)
    string(TOLOWER "${copy_kind}" copy_name)
    set(copy_source "${USD_BREP_ROOT}/source/SMLib/src/SmBSpline${copy_kind}.cpp")
    set(copy_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-copy-${copy_name}-basis.patch")
    set(copy_patched "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmBSpline${copy_kind}.cpp")
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${copy_source}" "${copy_patch}")
    execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${copy_patched}" "${copy_source}" "${copy_patch}"
                    RESULT_VARIABLE copy_patch_status)
    if(NOT copy_patch_status EQUAL 0)
        message(FATAL_ERROR "usd-brep exact-copy patch failed; review the pinned ${copy_kind} source")
    endif()
    list(REMOVE_ITEM KERNEL_SOURCES "${copy_source}")
    list(APPEND KERNEL_SOURCES "${copy_patched}")
endforeach()
