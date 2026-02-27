cmake_minimum_required(VERSION 3.22.1)
project(prop_spoof CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_VISIBILITY_PRESET hidden)
add_compile_options($<$<CONFIG:RELEASE>:-Os> -fno-exceptions -fno-rtti)
add_link_options($<$<CONFIG:RELEASE>:-s>)

add_compile_definitions(LINE_MAX=2048)
add_subdirectory(Dobby)

add_library(prop_spoof SHARED
    src/main.cpp
    src/hooks.cpp
    src/settings_hook.cpp
)
target_include_directories(prop_spoof PRIVATE
    src/
    zygisk/
)
target_link_libraries(prop_spoof dobby_static log)
