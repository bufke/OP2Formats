# OP2Formats for CMake projects. The library is Rust: this file runs Cargo to build the C interface (op2formats-c)
# and defines imported targets for it, so a C or C++ project can either
#
#   add_subdirectory(path/to/OP2Formats)              # builds it with the project
#
# or install it once (cmake --install) and use
#
#   find_package(OP2Formats REQUIRED)
#
# then
#
#   target_link_libraries(app PRIVATE OP2Formats::OP2Formats)
#   op2formats_copy_runtime(app)                       # copies the DLL next to app.exe
#
# Needs Cargo (rustup's, from https://rustup.rs); rust-toolchain.toml picks the Rust version. See README.md.

cmake_minimum_required(VERSION 3.19)
project(OP2Formats VERSION 0.1.0 LANGUAGES C)

option(OP2FORMATS_LINK_STATIC "Link OP2Formats::OP2Formats statically" OFF)
option(OP2FORMATS_BUILD_EXAMPLES "Build the C++ example" ${PROJECT_IS_TOP_LEVEL})
set(OP2FORMATS_FEATURES "" CACHE STRING
  "Cargo features beyond the default inno and opu, comma-separated (e.g. fetch-opu)")
set(OP2FORMATS_RUST_TARGET "" CACHE STRING
  "Rust target to build for; empty for Cargo's default (MinGW builds default to x86_64-pc-windows-gnu)")

find_program(OP2FORMATS_CARGO cargo HINTS "$ENV{CARGO_HOME}/bin" "$ENV{USERPROFILE}/.cargo/bin" "$ENV{HOME}/.cargo/bin"
  REQUIRED DOC "Cargo, to build the Rust library")

# A MinGW compiler needs the GNU flavour of Rust's Windows target: an MSVC-built library won't link with it.
set(rust_target "${OP2FORMATS_RUST_TARGET}")
if(NOT rust_target AND MINGW)
  set(rust_target "x86_64-pc-windows-gnu")
endif()

set(cargo_dir "${CMAKE_CURRENT_BINARY_DIR}/cargo")
set(cargo_args build --package op2formats-c --release --target-dir "${cargo_dir}")
if(rust_target)
  list(APPEND cargo_args --target "${rust_target}")
  set(out_dir "${cargo_dir}/${rust_target}/release")
else()
  set(out_dir "${cargo_dir}/release")
endif()
if(OP2FORMATS_FEATURES)
  list(APPEND cargo_args --features "${OP2FORMATS_FEATURES}")
endif()

include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/OP2FormatsTargets.cmake")
_op2formats_define_targets("${CMAKE_CURRENT_SOURCE_DIR}/op2formats-c/include" "${out_dir}" "${out_dir}")

# Cargo decides what to rebuild, so it runs on every build (a fraction of a second when nothing changed). The Rust
# library is always an optimised build, whether the program using it is built for Debug or Release.
get_target_property(shared_file OP2Formats::shared IMPORTED_LOCATION)
get_target_property(static_file OP2Formats::static IMPORTED_LOCATION)
set(byproducts "${shared_file}" "${static_file}")
if(WIN32)
  get_target_property(implib_file OP2Formats::shared IMPORTED_IMPLIB)
  list(APPEND byproducts "${implib_file}")
endif()
add_custom_target(op2formats_cargo ALL
  COMMAND "${OP2FORMATS_CARGO}" ${cargo_args}
  WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
  BYPRODUCTS ${byproducts}
  COMMENT "Building OP2Formats (Rust) with Cargo"
  USES_TERMINAL
  VERBATIM)
add_dependencies(OP2Formats::shared op2formats_cargo)
add_dependencies(OP2Formats::static op2formats_cargo)

# Installing: the header, the libraries, and a package for find_package(OP2Formats).
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
install(FILES op2formats-c/include/op2formats.h DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}")
if(WIN32)
  set(OP2FORMATS_SHARED_DIR "${CMAKE_INSTALL_BINDIR}")
  install(FILES "${shared_file}" DESTINATION "${CMAKE_INSTALL_BINDIR}")
  install(FILES "${implib_file}" "${static_file}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
else()
  set(OP2FORMATS_SHARED_DIR "${CMAKE_INSTALL_LIBDIR}")
  install(FILES "${shared_file}" "${static_file}" DESTINATION "${CMAKE_INSTALL_LIBDIR}")
endif()
set(OP2FORMATS_FEATURES_ALL "inno, opu")
if(OP2FORMATS_FEATURES)
  string(APPEND OP2FORMATS_FEATURES_ALL ", ${OP2FORMATS_FEATURES}")
endif()
set(package_dir "${CMAKE_INSTALL_LIBDIR}/cmake/OP2Formats")
configure_package_config_file(cmake/OP2FormatsConfig.cmake.in "${CMAKE_CURRENT_BINARY_DIR}/OP2FormatsConfig.cmake"
  INSTALL_DESTINATION "${package_dir}"
  PATH_VARS CMAKE_INSTALL_INCLUDEDIR CMAKE_INSTALL_LIBDIR OP2FORMATS_SHARED_DIR)
write_basic_package_version_file("${CMAKE_CURRENT_BINARY_DIR}/OP2FormatsConfigVersion.cmake"
  COMPATIBILITY SameMinorVersion)
install(FILES
  "${CMAKE_CURRENT_BINARY_DIR}/OP2FormatsConfig.cmake"
  "${CMAKE_CURRENT_BINARY_DIR}/OP2FormatsConfigVersion.cmake"
  cmake/OP2FormatsTargets.cmake
  DESTINATION "${package_dir}")

if(OP2FORMATS_BUILD_EXAMPLES)
  add_subdirectory(op2formats-c/examples/cpp)
endif()
