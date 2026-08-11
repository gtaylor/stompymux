/* diagnostics.h - Timestamped stderr tracing and always-on assertions. */

#pragma once

#include <errno.h>

typedef struct DiagnosticLocation {
  const char *file;
  int line;
  const char *function;
} DiagnosticLocation;

void diagnostics_log(DiagnosticLocation location, const char *format, ...)
    __attribute__((format(printf, 2, 3)));
[[noreturn]] void diagnostics_assert_failed(DiagnosticLocation location,
                                            const char *expression);
void diagnostics_perror(DiagnosticLocation location, const char *expression,
                        int saved_errno);

/* dassert: abort with a timestamped message if `x` is false. Always active,
 * regardless of the DEBUG build option. */
#define DASSERT(x)                                                             \
  do {                                                                         \
    if (!(x))                                                                  \
      diagnostics_assert_failed(                                               \
          (DiagnosticLocation){__FILE__, __LINE__, __FUNCTION__}, #x);         \
  } while (0)

/* dperror: log a timestamped message if `x` is true, using errno to describe
 * the failure. */
#define DPERROR(x)                                                             \
  do {                                                                         \
    if (x)                                                                     \
      diagnostics_perror(                                                      \
          (DiagnosticLocation){__FILE__, __LINE__, __FUNCTION__}, #x, errno);  \
  } while (0)

/* printk: always-on timestamped trace message. */
#define PRINTK(...)                                                            \
  diagnostics_log((DiagnosticLocation){__FILE__, __LINE__, __FUNCTION__},      \
                  __VA_ARGS__)

/* dprintk: timestamped trace message, compiled out unless DEBUG is set. */
#ifdef DEBUG
#define dprintk(...)                                                           \
  diagnostics_log((DiagnosticLocation){__FILE__, __LINE__, __FUNCTION__},      \
                  __VA_ARGS__)
#else
#define DPRINTK(...) ((void)0)
#endif
