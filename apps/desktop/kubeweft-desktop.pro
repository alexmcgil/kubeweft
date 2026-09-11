QT += core gui qml quick
CONFIG += c++17
CONFIG -= app_bundle

TEMPLATE = app
TARGET = kubeweft-desktop

ROOT = $$clean_path($$PWD/../..)
DESTDIR = $$ROOT/target/desktop
OBJECTS_DIR = $$OUT_PWD/obj
MOC_DIR = $$OUT_PWD/moc
RCC_DIR = $$OUT_PWD/rcc

SOURCES += \
    src/main.cpp \
    src/filesystembridge.cpp

HEADERS += \
    src/bridge.h \
    src/filesystembridge.h

RESOURCES += qml.qrc

INCLUDEPATH += src
PRE_TARGETDEPS += $$ROOT/target/release/libkubeweft_desktop_bridge.a
LIBS += $$ROOT/target/release/libkubeweft_desktop_bridge.a -lpthread -ldl -lm
