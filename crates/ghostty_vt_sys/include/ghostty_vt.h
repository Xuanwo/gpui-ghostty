#ifndef GHOSTTY_VT_H
#define GHOSTTY_VT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void* ghostty_vt_terminal_t;

typedef struct ghostty_vt_bytes_s {
  const uint8_t* ptr;
  size_t len;
} ghostty_vt_bytes_t;

ghostty_vt_terminal_t ghostty_vt_terminal_new(uint16_t cols, uint16_t rows);
void ghostty_vt_terminal_free(ghostty_vt_terminal_t terminal);

int ghostty_vt_terminal_feed(ghostty_vt_terminal_t terminal,
                             const uint8_t* bytes,
                             size_t len);

int ghostty_vt_terminal_resize(ghostty_vt_terminal_t terminal, uint16_t cols, uint16_t rows);

int ghostty_vt_terminal_scroll_viewport(ghostty_vt_terminal_t terminal, int32_t delta_lines);
int ghostty_vt_terminal_scroll_viewport_top(ghostty_vt_terminal_t terminal);
int ghostty_vt_terminal_scroll_viewport_bottom(ghostty_vt_terminal_t terminal);

ghostty_vt_bytes_t ghostty_vt_terminal_dump_viewport(ghostty_vt_terminal_t terminal);
ghostty_vt_bytes_t ghostty_vt_terminal_take_dirty_viewport_rows(ghostty_vt_terminal_t terminal,
                                                                uint16_t rows);
void ghostty_vt_bytes_free(ghostty_vt_bytes_t bytes);

#ifdef __cplusplus
}
#endif

#endif /* GHOSTTY_VT_H */
