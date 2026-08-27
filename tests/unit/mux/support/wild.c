#include "mux/support/wild.h"

#include <string.h>

#include "mux/support/checked_storage.h"

typedef struct WildCase {
  const char *pattern;
  const char *text;
  bool expected;
} WildCase;

int main(void) {
  const WildCase cases[] = {
      {.pattern = "", .text = "", .expected = true},
      {.pattern = "*", .text = "anything", .expected = true},
      {.pattern = "a?c", .text = "AbC", .expected = true},
      {.pattern = "a*c", .text = "abbbc", .expected = true},
      {.pattern = "a**?c", .text = "abbc", .expected = true},
      {.pattern = "a\\*c", .text = "a*c", .expected = true},
      {.pattern = "a\\?c", .text = "a?c", .expected = true},
      {.pattern = "a*d", .text = "abbbc", .expected = false},
      {.pattern = "*?", .text = "", .expected = false},
  };
  const size_t case_count = sizeof(cases) / sizeof(cases[0]);

  for (size_t index = 0; index < case_count; index++) {
    const WildCase *test =
        checked_storage_at_const(cases, case_count, sizeof(*cases), index);
    if (quick_wild(test->pattern, test->text) != test->expected)
      return 1;
  }

  char pattern[4097];
  memset(pattern, '*', sizeof(pattern) - 2);
  *(char *)checked_storage_at(pattern, sizeof(pattern), sizeof(char),
                              sizeof(pattern) - 2) = 'z';
  *(char *)checked_storage_at(pattern, sizeof(pattern), sizeof(char),
                              sizeof(pattern) - 1) = '\0';
  return quick_wild(pattern, "z") ? 0 : 1;
}
