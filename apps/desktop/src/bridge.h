#pragma once

#include <cstdint>

extern "C" {
struct DesktopFilesystem;

DesktopFilesystem *kubeweft_desktop_open(const char *data_directory);
void kubeweft_desktop_close(DesktopFilesystem *filesystem);
char *kubeweft_desktop_list(DesktopFilesystem *filesystem, const char *path);
char *kubeweft_desktop_read(DesktopFilesystem *filesystem, const char *path);
char *kubeweft_desktop_mkdir(DesktopFilesystem *filesystem, const char *path);
char *kubeweft_desktop_create(DesktopFilesystem *filesystem, const char *path);
char *kubeweft_desktop_write(DesktopFilesystem *filesystem, const char *path,
                             const char *content,
                             std::uint64_t expected_generation);
char *kubeweft_desktop_remove(DesktopFilesystem *filesystem, const char *path);
void kubeweft_desktop_string_free(char *value);
}
