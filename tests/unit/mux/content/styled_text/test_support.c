/* test_support.c - Shared styled-text test assertions. */

#include <stdio.h>
#include <string.h>

#include "test_support.h"

StyledTextPalette *styled_text_test_palette;

int expect_compile(const char *markup, const char *expected) {
  char output[2048];
  char error[256];

  if (!styled_text_compile(styled_text_test_palette, markup, output,
                           sizeof(output), error, sizeof(error))) {
    fprintf(stderr, "compile failed for %s: %s\n", markup, error);
    return 0;
  }
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected compile result for %s\n", markup);
    return 0;
  }
  return 1;
}

int expect_invalid(const char *markup) {
  char output[2048];
  char error[256];

  if (styled_text_compile(styled_text_test_palette, markup, output,
                          sizeof(output), error, sizeof(error))) {
    fprintf(stderr, "unexpectedly accepted %s\n", markup);
    return 0;
  }
  return error[0] != '\0';
}

int expect_valid(const char *markup) {
  char output[8192];
  char error[256];

  if (!styled_text_compile(styled_text_test_palette, markup, output,
                           sizeof(output), error, sizeof(error))) {
    fprintf(stderr, "compile failed for %s: %s\n", markup, error);
    return 0;
  }
  return 1;
}

int expect_render(const char *styled, TerminalColorDepth depth,
                  const char *expected) {
  char output[2048];

  styled_text_render(styled_text_test_palette, styled, depth, output,
                     sizeof(output));
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected render result for depth %d\n", (int)depth);
    return 0;
  }
  return 1;
}

int expect_render_options(const char *styled,
                          const StyledTextRenderOptions *options,
                          const char *expected) {
  char output[8192];

  styled_text_render_with_options(styled_text_test_palette, styled, options,
                                  output, sizeof(output));
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected OSC render result\nexpected: %s\nactual: %s\n",
            expected, output);
    return 0;
  }
  return 1;
}
