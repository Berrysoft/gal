cmake_minimum_required(VERSION 3.14)
project(gal-qt VERSION 0.1.0 LANGUAGES CXX)
set(CMAKE_VISIBILITY_INLINES_HIDDEN ON)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_CXX_VISIBILITY_PRESET hidden)

include(CheckIPOSupported)
check_ipo_supported(RESULT ENABLE_IPO OUTPUT output)
if(ENABLE_IPO)
    set(CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELEASE ON)
    message(STATUS "LTO is enabled for Release")
endif()

option(USE_QT5 "Build against Qt5" OFF)

if(${USE_QT5})
    set(QT_VERSION_MAJOR 5)
else()
    set(QT_VERSION_MAJOR 6)
endif()

find_package(Qt${QT_VERSION_MAJOR} COMPONENTS Widgets MultimediaWidgets REQUIRED)

if(WIN32)
    get_target_property(_qmake_executable Qt${QT_VERSION_MAJOR}::qmake IMPORTED_LOCATION)
    get_filename_component(_qt_bin_dir "${_qmake_executable}" DIRECTORY)
    find_program(WINDEPLOYQT_EXECUTABLE windeployqt HINTS "${_qt_bin_dir}")
endif()

add_subdirectory(corrosion)

corrosion_import_crate(MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/../gal-native/Cargo.toml)

execute_process(
    COMMAND cbindgen --config cbindgen.toml --crate gal-native --output ${CMAKE_CURRENT_BINARY_DIR}/gal.h
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/../gal-native
)

file(GLOB HEADERS "include/*.hpp")
file(GLOB SOURCES "src/*.cpp")

add_executable(gal-qt ${SOURCES} ${HEADERS})
target_include_directories(gal-qt
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
    PRIVATE
        ${CMAKE_CURRENT_BINARY_DIR}
)
target_link_libraries(gal-qt PUBLIC Qt${QT_VERSION_MAJOR}::Widgets Qt${QT_VERSION_MAJOR}::MultimediaWidgets gal-native-static)
set_target_properties(gal-qt PROPERTIES AUTOMOC ON AUTORCC ON Qt5_NO_LINK_QTMAIN ON qt_no_entrypoint ON)

if(WIN32)
    target_link_libraries(gal-qt PUBLIC ntdll icu)
    if(CMAKE_BUILD_TYPE MATCHES "[Rr]el")
        set_target_properties(gal-qt PROPERTIES WIN32_EXECUTABLE ON)
        if(MSVC)
            target_link_options(gal-qt PUBLIC "/ENTRY:mainCRTStartup")
        endif()
    endif()

    get_target_property(_qwidgets_lib Qt${QT_VERSION_MAJOR}::Widgets IMPORTED_LOCATION)
    get_filename_component(_qt_lib_dir "${_qwidgets_lib}" DIRECTORY)
    set(WINDEPLOYQT_ARGS "--no-compiler-runtime" "--no-translations" "--no-system-d3d-compiler" "--no-opengl-sw")
    if(NOT ${USE_QT5})
        list(APPEND WINDEPLOYQT_ARGS "--qmake" "${_qt_lib_dir}/qmake.exe")
    endif()
    add_custom_command(TARGET gal-qt POST_BUILD
        COMMAND "${CMAKE_COMMAND}" -E
            env PATH="${_qt_lib_dir}" "${WINDEPLOYQT_EXECUTABLE}"
                ${WINDEPLOYQT_ARGS}
                \"$<TARGET_FILE:gal-qt>\"
    )
elseif(APPLE)
    target_link_libraries(tunet-gui PUBLIC icucore)
elseif(UNIX)
    find_package(ICU REQUIRED)
    target_link_libraries(tunet-gui PUBLIC ${ICU_LIBRARIES})
endif()
