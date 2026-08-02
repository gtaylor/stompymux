/* styled_text.c -- Styled-text parser and renderer unit tests. */

#include <stdio.h>
#include <string.h>

#include "mux/support/styled_text.h"

static StyledTextPalette *palette;

static int expect_compile(const char *markup, const char *expected) {
  char output[2048];
  char error[256];

  if (!styled_text_compile(palette, markup, output, sizeof(output), error,
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

  if (styled_text_compile(palette, markup, output, sizeof(output), error,
                          sizeof(error))) {
    fprintf(stderr, "unexpectedly accepted %s\n", markup);
    return 0;
  }
  return error[0] != '\0';
}

static int expect_render(const char *styled, TerminalColorDepth depth,
                         const char *expected) {
  char output[2048];

  styled_text_render(palette, styled, depth, output, sizeof(output));
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected render result for depth %d\n", (int)depth);
    return 0;
  }
  return 1;
}

static int expect_render_options(const char *styled,
                                 const StyledTextRenderOptions *options,
                                 const char *expected) {
  char output[8192];

  styled_text_render_with_options(palette, styled, options, output,
                                  sizeof(output));
  if (strcmp(output, expected) != 0) {
    fprintf(stderr, "unexpected OSC render result\nexpected: %s\nactual: %s\n",
            expected, output);
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
  const char grouped[] = "\033[0m\033[1m\033[34m\033[47mBlue\033[0m";
  const char blinking[] = "\033[0m\033[1m\033[5m\033[31mAlert\033[0m";
  const char truecolor[] = "\033[38;2;255;0;0mR";
  const char send_markup[] = "[send=\"cast fireball\"]Cast[/]";
  const char send_osc[] =
      "\033]8;;send:cast%20fireball\033\\Cast\033]8;;\033\\";
  const StyledTextRenderOptions links = {
      .color_depth = TERMINAL_COLOR_NONE,
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
  };
  const StyledTextRenderOptions send_only = {
      .color_depth = TERMINAL_COLOR_NONE,
      .osc_hyperlinks_send = true,
  };
  const StyledTextRenderOptions no_links = {0};
  TerminalColorDepth depth;
  bool screen_reader;
  char error[256];
  char small[4];
  char small_link[32];
  char oversized_link[4200];
  int result = 0;

  palette = styled_text_palette_create();
  if (!palette)
    return 1;

  memcpy(oversized_link, "[link=\"https://", 15);
  memset(oversized_link + 15, 'a', 4090);
  memcpy(oversized_link + 4105, "\"]x[/]", 7);
  oversized_link[4112] = '\0';

  if (!expect_compile("[fg=red]Red[/]", red) ||
      !expect_compile("[fg=red]red [bold]bold[/] red[/]", nested) ||
      !expect_compile("[fg=red]caf\xc3\xa9[/]",
                      "\033[0m\033[31mcaf\xc3\xa9\033[0m") ||
      !expect_compile("[fg=blue bg=white bold]Blue[/]", grouped) ||
      !expect_compile("[fg=red bold blink]Alert[/]", blinking) ||
      !expect_compile("[[literal]", "[literal]") ||
      !expect_compile(send_markup, send_osc) ||
      !expect_compile("[prompt=\"say \\\"hi\\\" \\\\ ok\"]Edit[/]",
                      "\033]8;;prompt:say%20%22hi%22%20%5C%20ok\033\\Edit"
                      "\033]8;;\033\\") ||
      !expect_compile("[link=\"https://example.com/a?x=1&y=%202\"]Web[/]",
                      "\033]8;;https://example.com/a?x=1&y=%202\033\\Web"
                      "\033]8;;\033\\") ||
      !expect_invalid("[fg=unknown]x[/]") ||
      !expect_invalid("[fg=#abcd]x[/]") || !expect_invalid("[bold]x") ||
      !expect_invalid("[fg=red unknown]x[/]") ||
      !expect_invalid("[fg=red /]x[/]") || !expect_invalid("[/]") ||
      !expect_invalid("[send=look]x[/]") ||
      !expect_invalid("[send=\"look\"][prompt=\"say\"]x[/][/]") ||
      !expect_invalid("[link=\"file:///tmp/a\"]x[/]") ||
      !expect_invalid("[link=\"https://example.com/%xx\"]x[/]") ||
      !expect_invalid(oversized_link) ||
      !expect_invalid("[send=\"line\nbreak\"]x[/]") ||
      !expect_invalid("bad\xc0\xaf") || !expect_invalid("\033[31mraw"))
    result = 1;

  if (!result &&
      (terminal_color_depth_from_type("xterm") != TERMINAL_COLOR_ANSI_256 ||
       terminal_color_depth_from_type("ANSI-TRUECOLOR") !=
           TERMINAL_COLOR_TRUECOLOR ||
       terminal_color_depth_from_type("DUMB") != TERMINAL_COLOR_NONE ||
       !terminal_mtts_parse("MTTS 329", &depth, &screen_reader) ||
       depth != TERMINAL_COLOR_TRUECOLOR || !screen_reader ||
       terminal_mtts_parse("MTTS nonsense", &depth, &screen_reader)))
    result = 1;

  if (!result &&
      (!expect_render(truecolor, TERMINAL_COLOR_NONE, "R") ||
       !expect_render_options(send_markup, &no_links, "Cast") ||
       !expect_render_options(send_markup, &send_only, send_osc) ||
       !expect_render_options("[link=\"https://example.com\"]Web[/]",
                              &send_only, "Web") ||
       !expect_render_options("[link=\"https://example.com\"]Web[/]", &links,
                              "\033]8;;https://example.com\033\\Web"
                              "\033]8;;\033\\") ||
       !expect_render_options("[prompt=\"look\"]Edit[/]", &send_only, "Edit") ||
       !expect_render("\033]8;;https://example.com\033\\Raw\033]8;;\033\\",
                      TERMINAL_COLOR_NONE, "Raw") ||
       !expect_render("caf\xc3\xa9 \xf0\x9f\x98\x80", TERMINAL_COLOR_NONE,
                      "caf\xc3\xa9 \xf0\x9f\x98\x80") ||
       !expect_render("bad\xc0\xaf", TERMINAL_COLOR_NONE,
                      "bad\xef\xbf\xbd\xef\xbf\xbd") ||
       !expect_render(truecolor, TERMINAL_COLOR_ANSI_16, "\033[91mR\033[0m") ||
       !expect_render(truecolor, TERMINAL_COLOR_ANSI_256,
                      "\033[38;5;9mR\033[0m") ||
       !expect_render(truecolor, TERMINAL_COLOR_TRUECOLOR,
                      "\033[38;2;255;0;0mR\033[0m")))
    result = 1;

  styled_text_render(palette, "ab\xc3\xa9", TERMINAL_COLOR_NONE, small,
                     sizeof(small));
  if (!result && strcmp(small, "ab") != 0)
    result = 1;
  styled_text_render_with_options(palette,
                                  "[send=\"x\"]abcdefghijklmnopqrstuvwxyz[/]",
                                  &send_only, small_link, sizeof(small_link));
  if (!result &&
      (!strstr(small_link, "\033]8;;send:x\033\\") || strlen(small_link) < 7 ||
       strcmp(small_link + strlen(small_link) - 7, "\033]8;;\033\\")))
    result = 1;

  if (!result &&
      (!expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_16,
                      "\033[0m\033[91mR\033[0m\033[0m") ||
       !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_256,
                      "\033[0m\033[38;5;9mR\033[0m\033[0m") ||
       !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_TRUECOLOR,
                      "\033[0m\033[38;2;255;0;0mR\033[0m\033[0m") ||
       !expect_render("[blink]Alert[/]", TERMINAL_COLOR_ANSI_16,
                      "\033[0m\033[5mAlert\033[0m\033[0m") ||
       !styled_text_escape("[fg=red]literal", escaped, sizeof(escaped)) ||
       strcmp(escaped, "[[fg=red]literal") != 0))
    result = 1;

  styled_text_strip(palette, "[fg=red]Red[/]", stripped, sizeof(stripped));
  styled_text_truncate(palette, "[fg=red bg=white]Red Alert[/]", 3, truncated,
                       sizeof(truncated));
  if (!result && (strcmp(stripped, "Red") != 0 ||
                  strcmp(truncated, "[fg=red bg=white]Red[/]") != 0 ||
                  styled_text_width(palette, "[fg=red]Red[/]") != 3))
    result = 1;
  styled_text_strip(palette, send_markup, stripped, sizeof(stripped));
  styled_text_truncate(palette, send_markup, 2, truncated, sizeof(truncated));
  if (!result && (strcmp(stripped, "Cast") != 0 ||
                  strcmp(truncated, "[send=\"cast fireball\"]Ca[/]") != 0 ||
                  styled_text_width(palette, send_markup) != 4))
    result = 1;
  styled_text_truncate(palette, "caf\xc3\xa9!", 4, truncated,
                       sizeof(truncated));
  if (!result && strcmp(truncated, "caf") != 0)
    result = 1;
  styled_text_truncate(palette, "caf\xc3\xa9!", 5, truncated,
                       sizeof(truncated));
  if (!result && strcmp(truncated, "caf\xc3\xa9") != 0)
    result = 1;

  if (!result &&
      (!styled_text_palette_set_rgb(palette, "brand-blue", 32, 96, 192, error,
                                    sizeof(error)) ||
       !expect_compile("[fg=BRAND-BLUE]B[/]",
                       "\033[0m\033[38;2;32;96;192mB\033[0m") ||
       !expect_render("[bg=brand-blue]B[/]", TERMINAL_COLOR_TRUECOLOR,
                      "\033[0m\033[48;2;32;96;192mB\033[0m\033[0m") ||
       styled_text_width(palette, "[fg=brand-blue]Blue[/]") != 4))
    result = 1;

  if (!result &&
      (!styled_text_palette_set_rgb(palette, "red", 1, 2, 3, error,
                                    sizeof(error)) ||
       !expect_compile("[fg=red]R[/]", "\033[0m\033[38;2;1;2;3mR\033[0m") ||
       styled_text_palette_set_rgb(palette, "bad name", 1, 2, 3, error,
                                   sizeof(error)) ||
       styled_text_palette_set_rgb(palette, "bad", 256, 2, 3, error,
                                   sizeof(error))))
    result = 1;

  styled_text_palette_destroy(palette);
  return result;
}
