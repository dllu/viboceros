# Keep the upstream checkout immutable; repair derived UV drop accuracy.
set(drop_accuracy_source "${USD_BREP_ROOT}/source/SMLib/src/SmSurfaceDropLib.cpp")
set(drop_accuracy_patch "${CMAKE_CURRENT_LIST_DIR}/usd-brep-drop-curve-accuracy.patch")
set(drop_accuracy_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmSurfaceDropLib.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${drop_accuracy_source}" "${drop_accuracy_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${drop_accuracy_output}"
    "${drop_accuracy_source}" "${drop_accuracy_patch}"
    RESULT_VARIABLE drop_accuracy_status OUTPUT_VARIABLE drop_accuracy_stdout ERROR_VARIABLE drop_accuracy_stderr)
if(NOT drop_accuracy_status EQUAL 0)
    message(FATAL_ERROR "Curve drop accuracy repair failed: ${drop_accuracy_stdout} ${drop_accuracy_stderr}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${drop_accuracy_source}")
list(APPEND KERNEL_SOURCES "${drop_accuracy_output}")
