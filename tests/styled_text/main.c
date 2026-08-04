/* main.c - Styled-text component test runner. */

#include <stdio.h>

#include "test_support.h"

int main(void) {
  static const struct {
    const char *name;
    int (*run)(void);
  } suites[] = {
      {"markup and palette", styled_text_markup_tests},
      {"terminal rendering", styled_text_terminal_tests},
      {"OSC 8 Tier 1-5", styled_text_osc8_tests},
      {"OSC 8 presets", styled_text_preset_tests},
  };

  for (size_t index = 0; index < sizeof(suites) / sizeof(suites[0]); index++) {
    if (suites[index].run() != 0) {
      fprintf(stderr, "styled-text suite failed: %s\n", suites[index].name);
      return 1;
    }
  }
  return 0;
}
