# The C++ example. Built as part of OP2Formats (OP2FORMATS_BUILD_EXAMPLES), or on its own against an installed copy:
#   cmake -S op2formats-c/examples/cpp -B build-example -DCMAKE_PREFIX_PATH=<where OP2Formats was installed>

cmake_minimum_required(VERSION 3.19)
project(op2formats_example LANGUAGES CXX)

if(NOT TARGET OP2Formats::OP2Formats)
  find_package(OP2Formats 0.1 REQUIRED)
endif()

add_executable(op2formats_example main.cpp)
target_compile_features(op2formats_example PRIVATE cxx_std_17)
target_link_libraries(op2formats_example PRIVATE OP2Formats::OP2Formats)
op2formats_copy_runtime(op2formats_example)
