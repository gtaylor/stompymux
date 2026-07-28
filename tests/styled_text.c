/* styled_text.c -- Styled-text parser and renderer unit tests. */

#include <stdio.h>
#include <string.h>

#include "mux/support/styled_text.h"

static int expect_compile(const char *markup, const char *expected) {
  char output[2048];
  char error[256];

  if (!styled_text_compile(markup, output, sizeof(output), error,
                           sizeof(error))) {
    fprintf(stderr, "compile failed for %s: %s\n", markup, error);
    return 0;
  }
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected compile result for %s\n", markup);
    return 0;
  }
  return 1;
}

static int expect_invalid(const char *markup) {
  char output[2048];
  char error[256];

  if (styled_text_compile(markup, output, sizeof(output), error,
                          sizeof(error))) {
    fprintf(stderr, "unexpectedly accepted %s\n", markup);
    return 0;
  }
  return error[0] != '\0';
}

static int expect_render(const char *styled, TerminalColorDepth depth,
                         const char *expected) {
  char output[2048];

  styled_text_render(styled, depth, output, sizeof(output));
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected render result for depth %d\n", (int)depth);
    return 0;
  }
  return 1;
}

int main(void) {
  char escaped[256];
  char stripped[256];
  char truncated[256];
  const char red[] = "\033[0m\033[31mRed\033[0m";
  const char nested[] =
      "\033[0m\033[31mred \033[0m\033[1m\033[31mbold\033[0m\033[31m"
      " red\033[0m";
  const char truecolor[] = "\033[38;2;255;0;0mR";
  TerminalColorDepth depth;
  bool screen_reader;

  if (!expect_compile("[fg=red]Red[/]", red) ||
      !expect_compile("[fg=red]red [bold]bold[/] red[/]", nested) ||
      !expect_compile("[[literal]", "[literal]") ||
      !expect_invalid("[fg=unknown]x[/]") ||
      !expect_invalid("[fg=#abcd]x[/]") || !expect_invalid("[bold]x") ||
      !expect_invalid("[/]") || !expect_invalid("\033[31mraw"))
    return 1;

  if (terminal_color_depth_from_type("xterm") !=
          TERMINAL_COLOR_ANSI_256 ||
      terminal_color_depth_from_type("ANSI-TRUECOLOR") !=
          TERMINAL_COLOR_TRUECOLOR ||
      terminal_color_depth_from_type("DUMB") != TERMINAL_COLOR_NONE ||
      !terminal_mtts_parse("MTTS 329", &depth, &screen_reader) ||
      depth != TERMINAL_COLOR_TRUECOLOR || !screen_reader ||
      terminal_mtts_parse("MTTS nonsense", &depth, &screen_reader))
    return 1;

  if (!expect_render(truecolor, TERMINAL_COLOR_NONE, "R") ||
      !expect_render(truecolor, TERMINAL_COLOR_ANSI_16,
                     "\033[91mR\033[0m") ||
      !expect_render(truecolor, TERMINAL_COLOR_ANSI_256,
                     "\033[38;5;9mR\033[0m") ||
      !expect_render(truecolor, TERMINAL_COLOR_TRUECOLOR,
                     "\033[38;2;255;0;0mR\033[0m"))
    return 1;

  if (!expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_16,
                     "\033[0m\033[91mR\033[0m\033[0m") ||
      !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_256,
                     "\033[0m\033[38;5;9mR\033[0m\033[0m") ||
      !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_TRUECOLOR,
                     "\033[0m\033[38;2;255;0;0mR\033[0m\033[0m") ||
      !styled_text_escape("[fg=red]literal", escaped, sizeof(escaped)) ||
      strcmp(escaped, "[[fg=red]literal") != 0)
    return 1;

  styled_text_strip("[fg=red]Red[/]", stripped, sizeof(stripped));
  styled_text_truncate("[fg=red]Red Alert[/]", 3, truncated,
                       sizeof(truncated));
  if (strcmp(stripped, "Red") != 0 ||
      strcmp(truncated, "[fg=red]Red[/]") != 0 ||
      styled_text_width("[fg=red]Red[/]") != 3)
    return 1;
  return 0;
}
