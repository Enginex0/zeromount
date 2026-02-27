cmake_minimum_required(VERSION 3.28)
project(zeromount_zygisk)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(LSPLANT_ROOT ${CMAKE_SOURCE_DIR}/../../external/lsplant)
set(LSPLANT_JNI  ${LSPLANT_ROOT}/lsplant/src/main/jni)
set(DOBBY_ROOT   ${CMAKE_SOURCE_DIR}/../../external/dobby)

set(LSPLANT_BUILD_SHARED OFF CACHE INTERNAL "" FORCE)
add_subdirectory(${LSPLANT_JNI} lsplant_build)
add_subdirectory(${DOBBY_ROOT} dobby_build)

set(CXA_ATEXIT_ROOT ${CMAKE_SOURCE_DIR}/external/cxa_atexit)

add_library(zeromount_zygisk SHARED
    src/main.cpp
    src/settings_hook.cpp
    ${CXA_ATEXIT_ROOT}/atexit.cpp
)

target_include_directories(zeromount_zygisk PRIVATE
    include
    ${LSPLANT_JNI}/include
    ${DOBBY_ROOT}/include
)

target_link_libraries(zeromount_zygisk
    lsplant_static
    dobby_static
    android
    log
)

target_compile_options(zeromount_zygisk PRIVATE
    -fvisibility=hidden
    -ffunction-sections
    -fdata-sections
    -O2
    -Wall
)

target_link_options(zeromount_zygisk PRIVATE
    -Wl,--gc-sections
    -Wl,--exclude-libs,ALL
)
