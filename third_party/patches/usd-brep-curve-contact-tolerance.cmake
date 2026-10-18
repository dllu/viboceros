# Preserve strict intersection solves, then test stalled bounded contact witnesses.
set(contact_source "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveSelfPairs.cpp")
set(contact_patch "${CMAKE_CURRENT_LIST_DIR}/usd-brep-curve-contact-tolerance.patch")
set(contact_output "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/SmCurveContacts.cpp")
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${contact_patch}")
execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${contact_output}"
    "${contact_source}" "${contact_patch}"
    RESULT_VARIABLE contact_status OUTPUT_VARIABLE contact_stdout ERROR_VARIABLE contact_stderr)
if(NOT contact_status EQUAL 0)
    message(FATAL_ERROR "Curve contact repair failed: ${contact_stdout} ${contact_stderr}")
endif()
list(REMOVE_ITEM KERNEL_SOURCES "${contact_source}")
list(APPEND KERNEL_SOURCES "${contact_output}")
