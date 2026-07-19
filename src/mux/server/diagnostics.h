/* diagnostics.h - Timestamped stderr tracing and always-on assertions. */

#pragma once

#include <errno.h>

void diagnostics_log(const char *file, int line, const char *func,
                     const char *format, ...)
    __attribute__((format(printf, 4, 5)));
[[noreturn]] void diagnostics_assert_failed(const char *file, int line,
                                            const char *func, const char *expr);
void diagnostics_perror(const char *file, int line, const char *func,
                        const char *expr, int saved_errno);

/* dassert: abort with a timestamped message if `x` is false. Always active,
 * regardless of the DEBUG build option. */
#define dassert(x)                                                             \
  do {                                                                         \
    if (!(x))                                                                  \
      diagnostics_assert_failed(__FILE__, __LINE__, __FUNCTION__, #x);         \
  } while (0)

/* dperror: log a timestamped message if `x` is true, using errno to describe
 * the failure. */
#define dperror(x)                                                             \
  do {                                                                         \
    if (x)                                                                     \
      diagnostics_perror(__FILE__, __LINE__, __FUNCTION__, #x, errno);         \
  } while (0)

/* printk: always-on timestamped trace message. */
#define printk(...)                                                            \
  diagnostics_log(__FILE__, __LINE__, __FUNCTION__, __VA_ARGS__)

/* dprintk: timestamped trace message, compiled out unless DEBUG is set. */
#ifdef DEBUG
#define dprintk(...)                                                           \
  diagnostics_log(__FILE__, __LINE__, __FUNCTION__, __VA_ARGS__)
#else
#define dprintk(...) ((void)0)
#endif
