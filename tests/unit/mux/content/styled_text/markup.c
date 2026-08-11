/* markup.c - Markup transforms, ANSI fallback, and palette tests. */

#include <string.h>

#include "test_support.h"

int styled_text_markup_tests(void) {
  const char send_markup[] = "[send=\"cast fireball\"]Cast[/]";
  const StyledTextRenderOptions tier_two_basic = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_style_basic = true,
  };
  char escaped[256];
  char stripped[256];
  char truncated[256];
  char error[256];
  int result = 0;

  styled_text_test_palette = styled_text_palette_create();
  if (!styled_text_test_palette)
    return 1;

  if (!expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_16,
                     "\033[0m\033[91mR\033[0m\033[0m") ||
      !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_ANSI_256,
                     "\033[0m\033[38;5;9mR\033[0m\033[0m") ||
      !expect_render("[fg=#ff0000]R[/]", TERMINAL_COLOR_TRUECOLOR,
                     "\033[0m\033[38;2;255;0;0mR\033[0m\033[0m") ||
      !expect_render("[blink]Alert[/]", TERMINAL_COLOR_ANSI_16,
                     "\033[0m\033[5mAlert\033[0m\033[0m") ||
      !styled_text_escape("[fg=red]literal", escaped, sizeof(escaped)) ||
      strcmp(escaped, "[[fg=red]literal") != 0)
    result = 1;

  styled_text_strip(styled_text_test_palette, "[fg=red]Red[/]", stripped,
                    sizeof(stripped));
  styled_text_truncate(styled_text_test_palette,
                       "[fg=red bg=white]Red Alert[/]", 3, truncated,
                       sizeof(truncated));
  if (!result &&
      (strcmp(stripped, "Red") != 0 ||
       strcmp(truncated, "[fg=red bg=white]Red[/]") != 0 ||
       styled_text_width(styled_text_test_palette, "[fg=red]Red[/]") != 3))
    result = 1;

  styled_text_strip(styled_text_test_palette, send_markup, stripped,
                    sizeof(stripped));
  styled_text_truncate(styled_text_test_palette, send_markup, 2, truncated,
                       sizeof(truncated));
  if (!result &&
      (strcmp(stripped, "Cast") != 0 ||
       strcmp(truncated, "[send=\"cast fireball\"]Ca[/]") != 0 ||
       styled_text_width(styled_text_test_palette, send_markup) != 4))
    result = 1;

  styled_text_truncate(styled_text_test_palette, "caf\xc3\xa9!", 4, truncated,
                       sizeof(truncated));
  if (!result && strcmp(truncated, "caf") != 0)
    result = 1;
  styled_text_truncate(styled_text_test_palette, "caf\xc3\xa9!", 5, truncated,
                       sizeof(truncated));
  if (!result && strcmp(truncated, "caf\xc3\xa9") != 0)
    result = 1;

  if (!result &&
      (!styled_text_palette_set_rgb(styled_text_test_palette, "brand-blue", 32,
                                    96, 192, error, sizeof(error)) ||
       !expect_compile("[fg=BRAND-BLUE]B[/]",
                       "\033[0m\033[38;2;32;96;192mB\033[0m") ||
       !expect_render("[bg=brand-blue]B[/]", TERMINAL_COLOR_TRUECOLOR,
                      "\033[0m\033[48;2;32;96;192mB\033[0m\033[0m") ||
       !expect_render_options(
           "[send=\"x\" color=brand-blue]B[/]", &tier_two_basic,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%232060c0%22%7D%7D\033\\B\033]8;;\033\\") ||
       styled_text_width(styled_text_test_palette, "[fg=brand-blue]Blue[/]") !=
           4))
    result = 1;

  if (!result &&
      (styled_text_palette_set_rgb(styled_text_test_palette, "ReD", 1, 2, 3,
                                   error, sizeof(error)) ||
       !strstr(error, "built-in CSS/X11") ||
       !expect_compile("[fg=red]R[/]", "\033[0m\033[38;2;255;0;0mR\033[0m") ||
       styled_text_palette_set_rgb(styled_text_test_palette, "bad name", 1, 2,
                                   3, error, sizeof(error)) ||
       styled_text_palette_set_rgb(styled_text_test_palette, "bad", 256, 2, 3,
                                   error, sizeof(error))))
    result = 1;

  styled_text_palette_destroy(styled_text_test_palette);
  return result;
}
