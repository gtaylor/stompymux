/* formatting.c - Safe transient-buffer printf formatting helpers. */

#include "mux/support/formatting.h"

#include <stdarg.h>
#include <stdio.h>

#include "mux/support/alloc.h"

void safe_tprintf_str(char *str, char **bp, const char *format, ...) {
  char buff[LBUF_SIZE];
  va_list ap;

  va_start(ap, format);

  /*
   * Sigh, don't we wish _all_ vsprintf's returned int...
   */

  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(buff, LBUF_SIZE, format, ap);
  va_end(ap);
  buff[LBUF_SIZE - 1] = '\0';
  safe_str(buff, str, bp);
  **bp = '\0';
}
