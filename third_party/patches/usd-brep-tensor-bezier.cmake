# Keep the exact split/extraction repairs while repairing tensor decomposition.
set(bezier_source "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolSplits.cpp")
set(bezier_patch "${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/usd-brep-tensor-bezier.patch")
set(bezier_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/NL_SrfToolBezier.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${bezier_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${bezier_output}" "${bezier_source}" "${bezier_patch}"
                RESULT_VARIABLE bezier_patch_status)
if(NOT bezier_patch_status EQUAL 0)
    message(FATAL_ERROR "usd-brep tensor Bezier repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${bezier_source}")
list(APPEND KERNEL_SOURCES "${bezier_output}")
