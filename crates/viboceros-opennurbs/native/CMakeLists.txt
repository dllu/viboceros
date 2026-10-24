cmake_minimum_required(VERSION 3.16)
project(viboceros_opennurbs_bridge LANGUAGES C CXX)

if(NOT OPENNURBS_ROOT)
  message(FATAL_ERROR "OPENNURBS_ROOT must name an OpenNURBS checkout")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)

add_subdirectory("${OPENNURBS_ROOT}" opennurbs EXCLUDE_FROM_ALL)
include("${CMAKE_CURRENT_SOURCE_DIR}/../../../third_party/patches/opennurbs-merge-face-holes.cmake")

add_library(viboceros_opennurbs STATIC viboceros_opennurbs.cpp mesh_modeling.cpp subd.cpp)
target_include_directories(viboceros_opennurbs PRIVATE "${OPENNURBS_ROOT}")
target_compile_definitions(viboceros_opennurbs PRIVATE ON_CMAKE_BUILD)
target_link_libraries(viboceros_opennurbs PRIVATE opennurbsStatic)

if(CMAKE_VERSION VERSION_GREATER_EQUAL "3.24")
  add_executable(viboceros_extrusion_roundoff_check EXCLUDE_FROM_ALL extrusion_roundoff_test.cpp)
  target_include_directories(viboceros_extrusion_roundoff_check PRIVATE "${OPENNURBS_ROOT}")
  target_compile_definitions(viboceros_extrusion_roundoff_check PRIVATE ON_CMAKE_BUILD)
  target_link_libraries(viboceros_extrusion_roundoff_check PRIVATE "$<LINK_LIBRARY:WHOLE_ARCHIVE,opennurbsStatic>")
endif()

if(MSVC)
  target_compile_options(viboceros_opennurbs PRIVATE /W4 /permissive-)
else()
  target_compile_options(viboceros_opennurbs PRIVATE -Wall -Wextra -Wpedantic)
  # Preserve explicit double-operation boundaries in OpenNURBS constructions.
  # Contracting a multiply/add on ARM can change a coordinate-axis arc into a
  # slightly tilted one, which changes the side of a singular Flow frame.
  target_compile_options(opennurbsStatic PRIVATE -ffp-contract=off)
  target_compile_options(viboceros_opennurbs PRIVATE -ffp-contract=off)
  if(TARGET viboceros_extrusion_roundoff_check)
    target_compile_options(viboceros_extrusion_roundoff_check PRIVATE -ffp-contract=off)
  endif()
endif()

option(VIBOCEROS_OPENNURBS_ASAN "Instrument the native kernel and C ABI" OFF)
if(VIBOCEROS_OPENNURBS_ASAN)
  foreach(instrumented opennurbsStatic viboceros_opennurbs)
    target_compile_options(${instrumented} PRIVATE -fsanitize=address -g -fno-omit-frame-pointer)
  endforeach()
endif()
