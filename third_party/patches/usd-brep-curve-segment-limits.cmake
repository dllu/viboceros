# Preserve the knot-removal repair and select exact extraction endpoints.
set(segment_source "${REMOVE_PATCHED}")
set(segment_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-curve-segment-limits.patch")
set(segment_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_CrvToolSegments.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${segment_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${segment_output}" "${segment_source}" "${segment_patch}"
                RESULT_VARIABLE segment_patch_status)
if(NOT segment_patch_status EQUAL 0)
    message(FATAL_ERROR "usd-brep curve segment limit repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${segment_source}")
list(APPEND KERNEL_SOURCES "${segment_output}")
