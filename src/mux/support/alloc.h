/* alloc.h - External definitions for memory allocation subsystem */

#pragma once

#include <stdlib.h>

#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"

constexpr int LBUF_SIZE = 16384;
constexpr int MBUF_SIZE = 2048;
constexpr int SBUF_SIZE = 256;

static inline void free_buf(void *buffer) {
  if (buffer)
    free(buffer);
}

[[nodiscard]] static inline void *alloc_lbuf(const char *description) {
  (void)description;
  return checked_storage_allocate(LBUF_SIZE);
}
[[nodiscard]] static inline void *alloc_mbuf(const char *description) {
  (void)description;
  return checked_storage_allocate(MBUF_SIZE);
}
[[nodiscard]] static inline void *alloc_sbuf(const char *description) {
  (void)description;
  return checked_storage_allocate(SBUF_SIZE);
}

static inline int safe_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, LBUF_SIZE - 1);
}
static inline int safe_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, LBUF_SIZE - 1);
}
static inline int safe_sb_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, SBUF_SIZE - 1);
}
static inline int safe_sb_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, SBUF_SIZE - 1);
}
static inline int safe_mb_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, MBUF_SIZE - 1);
}
static inline int safe_mb_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, MBUF_SIZE - 1);
}
