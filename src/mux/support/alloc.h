/** @file
 * External definitions for memory allocation subsystem.
 */
#pragma once

#include <stdlib.h>

#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"

constexpr int LBUF_SIZE = 16384;
constexpr int MBUF_SIZE = 2048;
constexpr int SBUF_SIZE = 256;

/** Executes free buf. @param[in,out] buffer Caller-owned output storage. */

static inline void free_buf(void *buffer) {
  if (buffer)
    free(buffer);
}

/** Executes alloc lbuf. @param[in] description Description. */

[[nodiscard]] static inline void *alloc_lbuf(const char *description
                                             [[maybe_unused]]) {
  return checked_storage_allocate(LBUF_SIZE);
}
/** Executes alloc mbuf. @param[in] description Description. */

[[nodiscard]] static inline void *alloc_mbuf(const char *description
                                             [[maybe_unused]]) {
  return checked_storage_allocate(MBUF_SIZE);
}
/** Executes alloc sbuf. @param[in] description Description. */

[[nodiscard]] static inline void *alloc_sbuf(const char *description
                                             [[maybe_unused]]) {
  return checked_storage_allocate(SBUF_SIZE);
}

/** Executes safe str. @param[in] s String or object to process. @param[in,out]
 * b B. @param[in,out] p P. */

static inline int safe_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, LBUF_SIZE - 1);
}
/** Executes safe chr. @param[in] c C. @param[in,out] b B. @param[in,out] p P.
 */

static inline int safe_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, LBUF_SIZE - 1);
}
/** Executes safe sb str. @param[in] s String or object to process.
 * @param[in,out] b B. @param[in,out] p P. */

static inline int safe_sb_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, SBUF_SIZE - 1);
}
/** Executes safe sb chr. @param[in] c C. @param[in,out] b B. @param[in,out] p
 * P. */

static inline int safe_sb_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, SBUF_SIZE - 1);
}
/** Executes safe mb str. @param[in] s String or object to process.
 * @param[in,out] b B. @param[in,out] p P. */

static inline int safe_mb_str(const char *s, char *b, char **p) {
  return safe_copy_str(s, b, p, MBUF_SIZE - 1);
}
/** Executes safe mb chr. @param[in] c C. @param[in,out] b B. @param[in,out] p
 * P. */

static inline int safe_mb_chr(char c, char *b, char **p) {
  return safe_copy_chr(c, b, p, MBUF_SIZE - 1);
}
