/* Formats the initial colored line of BattleTech registry help. */

#include "registry_help_internal.h"

#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

void registry_help_color_initialize(const char *from, char *to) {
  char buf[LBUF_SIZE];
  char *tp = to;

  const size_t FIRST_WORD_LENGTH = strcspn(from, " ");
  if (*checked_string_suffix(from, FIRST_WORD_LENGTH) != '\0') {
    const size_t COPY_SIZE =
        FIRST_WORD_LENGTH < sizeof(buf) ? FIRST_WORD_LENGTH + 1 : sizeof(buf);
    (void)string_copy_bounded(buf, COPY_SIZE, from);
    safe_str("[fg=blue bold]", to, &tp);
    safe_str(buf, to, &tp);
    safe_str("[reset] ", to, &tp);
    safe_str(checked_string_suffix(from, FIRST_WORD_LENGTH + 1), to, &tp);
  } else {
    safe_str("[fg=cyan]", to, &tp);
    safe_str(from, to, &tp);
    safe_str("[reset]", to, &tp);
  }
  *tp = '\0';
}
