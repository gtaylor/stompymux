/* alloc.h - External definitions for memory allocation subsystem */

#pragma once

#include "mux/support/stringutil.h"

constexpr int LBUF_SIZE = 16384;
constexpr int MBUF_SIZE = 2048;
constexpr int SBUF_SIZE = 256;

static inline void *alloc_lbuf(const char *s) { return malloc(LBUF_SIZE); }
static inline void free_lbuf(void *b) {
  if (b)
    free(b);
}
static inline void *alloc_mbuf(const char *s) { return malloc(MBUF_SIZE); }
static inline void free_mbuf(void *b) {
  if (b)
    free(b);
}
static inline void *alloc_sbuf(const char *s) { return malloc(SBUF_SIZE); }
static inline void free_sbuf(void *b) {
  if (b)
    free(b);
}

// Defined in boolexp.c, where struct BooleanExpression is fully declared.
struct BooleanExpression *alloc_bool(const char *s);
void free_bool(struct BooleanExpression *b);

// Defined in command_queue.c, where BQUE is fully declared.
struct bque *alloc_qentry(const char *s);
void free_qentry(struct bque *b);

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
