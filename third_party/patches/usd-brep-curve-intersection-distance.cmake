# Compile scoped intersection convergence repairs without mutating upstream.
find_program(PATCH_EXECUTABLE patch REQUIRED)
file(MAKE_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched")
set(INTERSECTION_REPAIRS
  "src/SmCurve.cpp|usd-brep-curve-intersection-distance.patch"
  "src/SmLocalSolveNd.cpp|usd-brep-curve-intersection-distance-solver.patch"
  "inc/SmLocalSolveNd.h|usd-brep-curve-intersection-distance-header.patch")
foreach(REPAIR IN LISTS INTERSECTION_REPAIRS)
  string(REPLACE "|" ";" PARTS "${REPAIR}")
  list(GET PARTS 0 RELATIVE_SOURCE)
  list(GET PARTS 1 PATCH_NAME)
  get_filename_component(BASENAME "${RELATIVE_SOURCE}" NAME)
  set(SOURCE "${USD_BREP_ROOT}/source/SMLib/${RELATIVE_SOURCE}")
  set(PATCH "${CMAKE_CURRENT_LIST_DIR}/${PATCH_NAME}")
  set(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/usd-brep-patched/${BASENAME}")
  set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${SOURCE}" "${PATCH}")
  execute_process(COMMAND "${PATCH_EXECUTABLE}" --batch --fuzz=0 -o "${OUTPUT}" "${SOURCE}" "${PATCH}"
    RESULT_VARIABLE STATUS OUTPUT_VARIABLE PATCH_OUTPUT ERROR_VARIABLE PATCH_ERROR)
  if(NOT STATUS EQUAL 0)
    message(FATAL_ERROR "Curve intersection patch failed: ${PATCH_OUTPUT} ${PATCH_ERROR}")
  endif()
  if(RELATIVE_SOURCE MATCHES "^src/")
    list(REMOVE_ITEM KERNEL_SOURCES "${SOURCE}")
    list(APPEND KERNEL_SOURCES "${OUTPUT}")
  endif()
endforeach()
