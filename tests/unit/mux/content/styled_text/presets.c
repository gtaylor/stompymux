/* presets.c - OSC 8 Tier 6 compact and preset tests. */

#include <stdio.h>
#include <string.h>

#include "mux/support/checked_storage.h"

#include "test_support.h"

int styled_text_preset_tests(void) {
  const StyledTextRenderOptions tier_two_basic = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_style_basic = true,
  };
  const StyledTextRenderOptions tier_three_full = {
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
      .osc_hyperlinks_style_basic = true,
      .osc_hyperlinks_style_states = true,
      .osc_hyperlinks_tooltip = true,
      .osc_hyperlinks_menu = true,
  };
  const StyledTextRenderOptions tier_six_full = {
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
      .osc_hyperlinks_style_basic = true,
      .osc_hyperlinks_style_states = true,
      .osc_hyperlinks_tooltip = true,
      .osc_hyperlinks_menu = true,
      .osc_hyperlinks_visibility = true,
      .osc_hyperlinks_spoiler = true,
      .osc_hyperlinks_disabled = true,
      .osc_hyperlinks_selection = true,
      .osc_hyperlinks_compact = true,
      .osc_hyperlinks_presets = true,
  };
  char error[256] = "";
  char tier_six_output[8192];
  char preset_definition[8192];

  styled_text_test_palette = styled_text_palette_create();
  if (!styled_text_test_palette)
    return 1;

  if (!styled_text_palette_set_preset(
          styled_text_test_palette,
          &(StyledPresetDefinition){.name = "danger",
                                    .directives =
                                        "color=red bold hover.color=yellow "
                                        "tooltip=\"Dangerous action\"",
                                    .error = error,
                                    .error_size = sizeof(error)}) ||
      !styled_text_palette_set_preset(
          styled_text_test_palette,
          &(StyledPresetDefinition){
              .name = "poll",
              .directives = "selection.group=\"demo\" selection.exclusive",
              .error = error,
              .error_size = sizeof(error)}) ||
      !styled_text_palette_set_preset(
          styled_text_test_palette,
          &(StyledPresetDefinition){
              .name = "menu",
              .directives = "menu.1.label=\"One\" menu.1.send=\"one\" "
                            "title=\"Choices\"",
              .error = error,
              .error_size = sizeof(error)}) ||
      styled_text_palette_preset_count(styled_text_test_palette) != 3 ||
      !styled_text_palette_render_preset(styled_text_test_palette, 0,
                                         &tier_six_full, preset_definition,
                                         sizeof(preset_definition)) ||
      !strstr(preset_definition, "preset:danger?config=") ||
      !strstr(preset_definition, "%22s%22%3A") ||
      !strstr(preset_definition, "%22t%22%3A%22Dangerous%20action%22") ||
      strcmp(checked_string_suffix(preset_definition,
                                   strlen(preset_definition) - 7),
             "\033]8;;\033\\")) {
    fprintf(stderr, "OSC 8 preset setup or definition failed: %s\n", error);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }

  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"x\" preset=\"danger\" active.bg=blue]Go[/]", &tier_six_full,
      tier_six_output, sizeof(tier_six_output));
  if (!strstr(tier_six_output, "send:x?preset=danger&config=") ||
      !strstr(tier_six_output, "%22s%22%3A%7B%22a%22%3A") ||
      strstr(tier_six_output, "%22t%22")) {
    fprintf(stderr, "OSC 8 preset reference or compact override failed: %s\n",
            tier_six_output);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }

  styled_text_render_with_options(
      styled_text_test_palette, "[send=\"x\" preset=\"danger\"]Go[/]",
      &tier_two_basic, tier_six_output, sizeof(tier_six_output));
  if (!strstr(tier_six_output,
              "%22style%22%3A%7B%22color%22%3A%22%23ff0000%22%2C%22bold%22"
              "%3Atrue%7D") ||
      strstr(tier_six_output, "preset=danger")) {
    fprintf(stderr, "OSC 8 unsupported-preset expansion failed: %s\n",
            tier_six_output);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }
  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"vote\" preset=\"poll\" selection.value=\"one\"]One[/]",
      &tier_six_full, tier_six_output, sizeof(tier_six_output));
  if (!strstr(tier_six_output, "preset=poll&config=") ||
      !strstr(tier_six_output, "%22sel%22%3A%7B%22value%22%3A%22one%22%7D")) {
    fprintf(stderr, "OSC 8 partial preset merge failed: %s\n", tier_six_output);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }
  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"x\" preset=\"menu\" menu.1.label=\"Two\" "
      "menu.1.send=\"two\"]Menu[/]",
      &tier_three_full, tier_six_output, sizeof(tier_six_output));
  if (!strstr(tier_six_output, "%22Two%22%3A%22send%3Atwo%22") ||
      strstr(tier_six_output, "%22One%22")) {
    fprintf(stderr, "OSC 8 preset menu replacement failed: %s\n",
            tier_six_output);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }
  styled_text_render_with_options(
      styled_text_test_palette,
      "[link=\"https://example.com/?preset=real&config=value#part\"]Web[/]",
      &tier_six_full, tier_six_output, sizeof(tier_six_output));
  if (!strstr(tier_six_output,
              "?%70%72%65%73%65%74=real&%63%6F%6E%66%69%67=value#part")) {
    fprintf(stderr, "OSC 8 Tier 6 reserved parameter rewrite failed: %s\n",
            tier_six_output);
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }
  if (!expect_invalid("[send=\"x\" preset=\"missing\"]x[/]") ||
      !expect_invalid("[send=\"x\" preset=danger]x[/]") ||
      styled_text_palette_set_preset(
          styled_text_test_palette,
          &(StyledPresetDefinition){.name = "bad name",
                                    .directives = "bold",
                                    .error = error,
                                    .error_size = sizeof(error)}) ||
      styled_text_palette_set_preset(
          styled_text_test_palette,
          &(StyledPresetDefinition){.name = "empty",
                                    .directives = "",
                                    .error = error,
                                    .error_size = sizeof(error)})) {
    fprintf(stderr, "OSC 8 invalid preset validation failed\n");
    styled_text_palette_destroy(styled_text_test_palette);
    return 1;
  }

  styled_text_palette_destroy(styled_text_test_palette);
  return 0;
}
