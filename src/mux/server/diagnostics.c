/* diagnostics.c - Timestamped stderr tracing and always-on assertions. */

#include <bits/types/struct_timeval.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "mux/server/diagnostics.h"

static void diagnostics_print_prefix(DiagnosticLocation location) {
  struct timeval tv;
  struct tm tm;
  time_t now;

  now = time(nullptr);
  if (now == (time_t)-1)
    now = 0;
  localtime_r(&now, &tm);
  gettimeofday(&tv, nullptr);
  (void)fprintf(stderr, "%02d%02d%02d.%08d:%5d %s (%s:%d)] ", tm.tm_hour,
                tm.tm_min, tm.tm_sec, (int)tv.tv_usec, getpid(),
                location.function, location.file, location.line);
}

void diagnostics_log(DiagnosticLocation location, const char *format, ...) {
  va_list args;

  diagnostics_print_prefix(location);
  va_start(args, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vfprintf(stderr, format, args);
  va_end(args);
  (void)fprintf(stderr, "\n");
}

[[noreturn]] void diagnostics_assert_failed(DiagnosticLocation location,
                                            const char *expression) {
  diagnostics_print_prefix(location);
  (void)fprintf(stderr, "failed assertion '%s'\n", expression);
  abort();
}

void diagnostics_perror(DiagnosticLocation location, const char *expression,
                        int saved_errno) {
  diagnostics_print_prefix(location);
  (void)fprintf(stderr, "'%s' failed with '%s'\n", expression,
                strerror(saved_errno));
}
