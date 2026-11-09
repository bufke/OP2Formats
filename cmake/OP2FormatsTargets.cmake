# Defines the imported targets for OP2Formats' C interface. Shared by the build tree (the top-level CMakeLists.txt)
# and an installed copy (OP2FormatsConfig.cmake):
#
#   OP2Formats::shared      op2formats.dll / libop2formats.so / libop2formats.dylib
#   OP2Formats::static      the static library, with the system libraries Rust's standard library needs
#   OP2Formats::OP2Formats  one of the two: static when OP2FORMATS_LINK_STATIC is on, else shared
#
# and op2formats_copy_runtime(<target>), which copies the shared library next to <target> after each build (Windows
# finds DLLs there; elsewhere it does no harm).

include_guard(GLOBAL)

# The library file names Cargo writes for this toolchain.
if(MSVC)
  set(_op2f_shared "op2formats.dll")
  set(_op2f_implib "op2formats.dll.lib")
  set(_op2f_static "op2formats.lib")
elseif(WIN32)
  # MinGW, with Rust's x86_64-pc-windows-gnu target.
  set(_op2f_shared "op2formats.dll")
  set(_op2f_implib "libop2formats.dll.a")
  set(_op2f_static "libop2formats.a")
elseif(APPLE)
  set(_op2f_shared "libop2formats.dylib")
  set(_op2f_static "libop2formats.a")
else()
  set(_op2f_shared "libop2formats.so")
  set(_op2f_static "libop2formats.a")
endif()

# System libraries the static library needs (`cargo rustc ... -- --print native-static-libs`). Only the MSVC list is
# checked so far.
if(WIN32)
  set(_op2f_system_libs kernel32 ntdll userenv ws2_32 dbghelp)
elseif(APPLE)
  set(_op2f_system_libs System c m)
else()
  set(_op2f_system_libs gcc_s util rt pthread m dl c)
endif()

# _op2formats_define_targets(<include dir> <dir of the libraries> <dir of the shared library>)
function(_op2formats_define_targets include_dir lib_dir bin_dir)
  if(TARGET OP2Formats::shared)
    return()
  endif()

  add_library(OP2Formats::shared SHARED IMPORTED GLOBAL)
  set_target_properties(OP2Formats::shared PROPERTIES
    IMPORTED_LOCATION "${bin_dir}/${_op2f_shared}"
    INTERFACE_INCLUDE_DIRECTORIES "${include_dir}")
  if(WIN32)
    set_target_properties(OP2Formats::shared PROPERTIES IMPORTED_IMPLIB "${lib_dir}/${_op2f_implib}")
  endif()

  add_library(OP2Formats::static STATIC IMPORTED GLOBAL)
  set_target_properties(OP2Formats::static PROPERTIES
    IMPORTED_LOCATION "${lib_dir}/${_op2f_static}"
    INTERFACE_INCLUDE_DIRECTORIES "${include_dir}"
    INTERFACE_LINK_LIBRARIES "${_op2f_system_libs}")

  add_library(OP2Formats::OP2Formats INTERFACE IMPORTED GLOBAL)
  if(OP2FORMATS_LINK_STATIC)
    target_link_libraries(OP2Formats::OP2Formats INTERFACE OP2Formats::static)
  else()
    target_link_libraries(OP2Formats::OP2Formats INTERFACE OP2Formats::shared)
  endif()
endfunction()

# Copies the shared library next to `target`'s output after each build. Nothing to do when linking statically.
function(op2formats_copy_runtime target)
  if(OP2FORMATS_LINK_STATIC)
    return()
  endif()
  add_custom_command(TARGET ${target} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different $<TARGET_FILE:OP2Formats::shared> $<TARGET_FILE_DIR:${target}>
    VERBATIM)
endfunction()
