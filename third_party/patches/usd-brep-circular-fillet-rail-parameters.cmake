# Apply after the existing G2 repair to preserve both independent fixes.
set(circular_fillet_source "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletStandardSolver.cpp")
set(circular_fillet_patch "${CMAKE_CURRENT_LIST_DIR}/usd-brep-circular-fillet-rail-parameters.patch")
set(circular_fillet_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmFilletStandardSolverRails.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${circular_fillet_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${circular_fillet_output}"
    "${circular_fillet_source}" "${circular_fillet_patch}"
    RESULT_VARIABLE circular_fillet_status)
if(NOT circular_fillet_status EQUAL 0)
    message(FATAL_ERROR "Circular fillet rail repair failed; review the pinned source")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${circular_fillet_source}")
list(APPEND KERNEL_SOURCES "${circular_fillet_output}")
