// C interface of the Rust core (src-tauri/src/native.rs), used by the Swift layer.
// Also the Swift bridging header for the app target.
#ifndef COFFI_BRIDGE_H
#define COFFI_BRIDGE_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    // JSON {title, artist, album, duration, cover} or NULL when nothing is loaded.
    void (*now_playing)(const char *json);
    void (*playback)(bool has_track, bool playing, double position, double rate);
    // "airplay" | "pick_folder" | "refresh_shortcuts"
    void (*request)(const char *what);
} CoffiCallbacks;

void coffi_native_register(CoffiCallbacks cb);

// 0 play, 1 pause, 2 toggle, 3 next, 4 previous, 5 stop, 6 seek to, 7 seek by, 8 like.
void coffi_control(int32_t cmd, double value);
void coffi_lifecycle(const char *event);
void coffi_folder_picked(const char *path);
// Default name shown to other Coff-fi devices on the network.
void coffi_set_device_name(const char *name);

bool coffi_ready(void);
// JSON array of {kind, id, name, subtitle}; free with coffi_string_free.
char *coffi_find(const char *kind, const char *query);
bool coffi_play(const char *kind, const char *id, bool shuffle);
void coffi_string_free(char *s);

// Implemented in Swift (CoffiNative.swift); called from main.mm before the app starts.
void coffi_native_install(void);

#ifdef __cplusplus
}
#endif

#endif
