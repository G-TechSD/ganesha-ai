cmake_minimum_required(VERSION 3.10)
project(SampleProject LANGUAGES CXX)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Add executable target
add_executable(sample_app src/main.cpp)

# Include directories (if any)
target_include_directories(sample_app PRIVATE include)

# Link libraries (if any)
# target_link_libraries(sample_app PRIVATE some_library)
