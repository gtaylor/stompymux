/* osc8.c - OSC 8 Tier 1-5 and markup validation tests. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "test_support.h"

int styled_text_osc8_tests(void) {
  const char red[] = "\033[0m\033[38;2;255;0;0mRed\033[0m";
  const char nested[] =
      "\033[0m\033[38;2;255;0;0mred \033[0m\033[1m\033[38;2;255;0;0m"
      "bold\033[0m\033[38;2;255;0;0m"
      " red\033[0m";
  const char grouped[] =
      "\033[0m\033[1m\033[38;2;0;0;255m\033[48;2;255;255;255mBlue\033[0m";
  const char blinking[] = "\033[0m\033[1m\033[5m\033[38;2;255;0;0mAlert\033[0m";
  const char truecolor[] = "\033[38;2;255;0;0mR";
  const char send_markup[] = "[send=\"cast fireball\"]Cast[/]";
  const char send_osc[] =
      "\033]8;;send:cast%20fireball\033\\Cast\033]8;;\033\\";
  const char styled_send_markup[] =
      "[send=\"attack\" color=red bg=black bold hover.color=yellow "
      "active.bg=rgb(255,0,0)]Attack[/]";
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
  const StyledTextRenderOptions tier_one_ansi = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
  };
  const StyledTextRenderOptions tier_two_basic = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_style_basic = true,
  };
  const StyledTextRenderOptions tier_two_states = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_style_states = true,
  };
  const StyledTextRenderOptions tier_two_full = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_style_basic = true,
      .osc_hyperlinks_style_states = true,
  };
  const StyledTextRenderOptions tier_three_tooltip = {
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_tooltip = true,
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
  const StyledTextRenderOptions tier_three_filtered = {
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_menu = true,
  };
  const StyledTextRenderOptions tier_four_full = {
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
  };
  const StyledTextRenderOptions tier_four_behavior = {
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_visibility = true,
      .osc_hyperlinks_spoiler = true,
      .osc_hyperlinks_disabled = true,
  };
  const StyledTextRenderOptions tier_five_full = {
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
  };
  const StyledTextRenderOptions tier_five_selection = {
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
      .osc_hyperlinks_selection = true,
  };
  const StyledTextRenderOptions no_links = {0};
  const StyledTextRenderOptions no_link_ansi = {
      .color_depth = TERMINAL_COLOR_ANSI_16,
  };
  char small[4];
  char small_link[32];
  char small_tier_three[96];
  char small_tier_four[128];
  char small_tier_five[160];
  char oversized_link[4200];
  char oversized_selection[4200];
  char oversized_tooltip[4200];
  int result = 0;

  styled_text_test_palette = styled_text_palette_create();
  if (!styled_text_test_palette)
    return 1;

  memcpy(oversized_link, "[link=\"https://", 15);
  memset(oversized_link + 15, 'a', 4090);
  memcpy(oversized_link + 4105, "\"]x[/]", 7);
  oversized_link[4112] = '\0';
  const char selection_prefix[] =
      "[send=\"x\" selection.group=\"g\" selection.value=\"";
  size_t selection_prefix_size = strlen(selection_prefix);
  memcpy(oversized_selection, selection_prefix, selection_prefix_size);
  memset(oversized_selection + selection_prefix_size, 'a', 4000);
  memcpy(oversized_selection + selection_prefix_size + 4000, "\"]x[/]", 7);
  oversized_selection[selection_prefix_size + 4007] = '\0';
  const char tooltip_prefix[] = "[send=\"x\" tooltip=\"";
  size_t tooltip_prefix_size = strlen(tooltip_prefix);
  memcpy(oversized_tooltip, tooltip_prefix, tooltip_prefix_size);
  memset(oversized_tooltip + tooltip_prefix_size, 'a', 4050);
  memcpy(oversized_tooltip + tooltip_prefix_size + 4050, "\"]x[/]", 7);
  oversized_tooltip[tooltip_prefix_size + 4057] = '\0';

  if (!expect_compile("[fg=red]Red[/]", red) ||
      !expect_compile("[fg=red]red [bold]bold[/] red[/]", nested) ||
      !expect_compile("[fg=red]caf\xc3\xa9[/]",
                      "\033[0m\033[38;2;255;0;0mcaf\xc3\xa9\033[0m") ||
      !expect_compile("[fg=blue bg=white bold]Blue[/]", grouped) ||
      !expect_compile(
          "[fg=RGB(1,2,3) bg=rgb(255,254,253)]x[/]",
          "\033[0m\033[38;2;1;2;3m\033[48;2;255;254;253mx\033[0m") ||
      !expect_compile("[fg=RebeccaPurple]x[/]",
                      "\033[0m\033[38;2;102;51;153mx\033[0m") ||
      !expect_compile("[color=red italic overline strikethrough]R[/]",
                      "\033[0m\033[3m\033[53m\033[9m\033[38;2;255;0;0mR"
                      "\033[0m") ||
      !expect_compile("[bold][bold=false]x[/]y[/]",
                      "\033[0m\033[1m\033[0mx\033[0m\033[1my\033[0m") ||
      !expect_compile("[fg=red bold blink]Alert[/]", blinking) ||
      !expect_valid(
          "[send=\"x\" color=red bg=black bold=false italic "
          "underline=wavy overline=dotted strikethrough=dashed "
          "text-decoration-color=green active.color=red hover.bg=blue "
          "focus-visible.bold focus.italic visited.underline=false "
          "selected.overline=true disabled.strikethrough=true "
          "link.fg=yellow any-link.bg=black]x[/]") ||
      !expect_valid("[send=\"x\" tooltip=\"caf\xc3\xa9 \\\"tip\\\" \\\\\" "
                    "title=\"Actions\" title.color=red title.bold "
                    "menu.2.prompt=\"examine target\" menu.1.send=\"attack\" "
                    "menu.2.label=\"Inspect\" menu.1.label=\"Attack\"]x[/]") ||
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
      !expect_invalid("[fg=#fff]x[/]") ||
      !expect_invalid("[fg=rgb(256,0,0)]x[/]") ||
      !expect_invalid("[fg=rgb(1,2)]x[/]") ||
      !expect_invalid("[fg=rgb(1,2,3,4)]x[/]") ||
      !expect_invalid("[fg=rgba(1,2,3,1)]x[/]") ||
      !expect_invalid("[fg=rgb(100%,0%,0%)]x[/]") ||
      !expect_invalid("[fg=rgb(1, 2, 3)]x[/]") ||
      !expect_invalid("[fg=red unknown]x[/]") ||
      !expect_invalid("[send=\"x\" hover.unknown=red]x[/]") ||
      !expect_invalid("[send=\"x\" unknown.color=red]x[/]") ||
      !expect_invalid("[send=\"x\" hover.bold=maybe]x[/]") ||
      !expect_invalid("[send=\"x\" underline=double]x[/]") ||
      !expect_invalid("[send=\"x\" color]x[/]") ||
      !expect_invalid("[send=\"x\" tooltip=plain]x[/]") ||
      !expect_invalid("[send=\"x\" tooltip=\"\"]x[/]") ||
      !expect_invalid("[send=\"x\" tooltip=\"unterminated]x[/]") ||
      !expect_invalid("[send=\"x\" tooltip=\"line\nbreak\"]x[/]") ||
      !expect_invalid("[send=\"x\" tooltip=\"bad\xc0\xaf\"]x[/]") ||
      !expect_invalid("[send=\"x\" title=\"Orphan\"]x[/]") ||
      !expect_invalid("[send=\"x\" title.bold menu.1.label=\"A\" "
                      "menu.1.send=\"a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.2.label=\"A\" "
                      "menu.2.send=\"a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.label=\"A\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.send=\"a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.label=\"A\" "
                      "menu.1.label=\"B\" menu.1.send=\"a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.label=\"A\" "
                      "menu.1.send=\"a\" menu.1.prompt=\"a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.separator "
                      "menu.1.label=\"A\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.1.label=\"A\" "
                      "menu.1.link=\"file:///tmp/a\"]x[/]") ||
      !expect_invalid("[send=\"x\" menu.0.separator]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.delay=1]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=hide]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=\"conceal\"]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.delay=-1]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.delay=1.5]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.delay=4294967296]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.delay=999999999999999999999999]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.expire.input=false]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.expire.outputDelay=500]x[/]") ||
      !expect_invalid("[send=\"x\" visibility.action=conceal "
                      "visibility.expire.unknown]x[/]") ||
      !expect_invalid("[send=\"x\" spoiler=maybe]x[/]") ||
      !expect_invalid("[send=\"x\" disabled=\"true\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.value=\"v\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.toggle]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=g "
                      "selection.value=\"v\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"\" "
                      "selection.value=\"v\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\" "
                      "selection.value=\"line\nbreak\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\" "
                      "selection.value=\"bad\xc0\xaf\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\" "
                      "selection.value=\"v\" selection.toggle=maybe]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\" "
                      "selection.value=\"v\" selection.toggle=\"true\"]x[/]") ||
      !expect_invalid("[send=\"x\" selection.group=\"g\" "
                      "selection.value=\"v\" selection.unknown]x[/]") ||
      !expect_invalid(oversized_selection) ||
      !expect_invalid(oversized_tooltip) || !expect_invalid("[fg=red /]x[/]") ||
      !expect_invalid("[/]") || !expect_invalid("[send=look]x[/]") ||
      !expect_invalid("[send=\"look\"][prompt=\"say\"]x[/][/]") ||
      !expect_invalid("[link=\"file:///tmp/a\"]x[/]") ||
      !expect_invalid("[link=\"https://example.com/%xx\"]x[/]") ||
      !expect_invalid(oversized_link) ||
      !expect_invalid("[send=\"line\nbreak\"]x[/]") ||
      !expect_invalid("bad\xc0\xaf") || !expect_invalid("\033[31mraw"))
    result = 1;

  if (!result &&
      (!expect_render(truecolor, TERMINAL_COLOR_NONE, "R") ||
       !expect_render_options(send_markup, &no_links, "Cast") ||
       !expect_render_options("[send=\"x\" color=red bold]X[/]", &no_link_ansi,
                              "\033[0m\033[1m\033[91mX\033[0m\033[0m") ||
       !expect_render_options(send_markup, &send_only, send_osc) ||
       !expect_render_options("[link=\"https://example.com\"]Web[/]",
                              &send_only, "Web") ||
       !expect_render_options("[link=\"https://example.com\"]Web[/]", &links,
                              "\033]8;;https://example.com\033\\Web"
                              "\033]8;;\033\\") ||
       !expect_render_options("[prompt=\"look\"]Edit[/]", &send_only, "Edit") ||
       !expect_render_options(
           styled_send_markup, &tier_two_full,
           "\033]8;;send:attack?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%23ff0000%22%2C%22bg%22%3A%22%23000000%22%2C%22bold%22%3Atrue%2C"
           "%22active%22%3A%7B%22bg%22%3A%22%23ff0000%22%7D%2C%22hover%22%3A"
           "%7B%22color%22%3A%22%23ffff00%22%7D%7D%7D\033\\Attack"
           "\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"attack\" color=red]Attack[/]", &tier_two_basic,
           "\033]8;;send:attack?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%23ff0000%22%7D%7D\033\\Attack\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"attack\" color=red]Attack[/]", &tier_one_ansi,
           "\033]8;;send:attack\033\\\033[0m\033[91mAttack\033]8;;\033\\"
           "\033[0m\033[0m") ||
       !expect_render_options(
           "[send=\"attack\" color=red hover.color=yellow]Attack[/]",
           &tier_two_states,
           "\033]8;;send:attack?config=%7B%22style%22%3A%7B%22hover%22%3A%7B"
           "%22color%22%3A%22%23ffff00%22%7D%7D%7D\033\\\033[0m\033[91m"
           "Attack\033]8;;\033\\\033[0m\033[0m") ||
       !expect_render_options(
           "[send=\"x\" color=rgb(1,2,3) "
           "text-decoration-color=rgb(7,8,9) hover.bg=rgb(4,5,6)]x[/]",
           &tier_two_full,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%23010203%22%2C%22text-decoration-color%22%3A%22%23070809%22%2C"
           "%22hover%22%3A%7B%22bg%22%3A%22%23040506%22%7D%7D%7D\033\\x"
           "\033]8;;\033\\") ||
       !expect_render_options(
           "[link=\"https://example.com/?config=old&a=1#part\" color=red]x[/]",
           &(const StyledTextRenderOptions){.osc_hyperlinks = true,
                                            .osc_hyperlinks_style_basic = true},
           "\033]8;;https://example.com/?%63%6F%6E%66%69%67=old&a=1&config="
           "%7B%22style%22%3A%7B%22color%22%3A%22%23ff0000%22%7D%7D#part"
           "\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" fg=red color=blue]x[/]", &tier_two_basic,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%230000ff%22%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" tooltip=\"Choose an action\"]x[/]", &tier_three_tooltip,
           "\033]8;;send:x?config=%7B%22tooltip%22%3A%22Choose%20an%20"
           "action%22%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" tooltip=\"Say \\\"hi\\\" \\\\ caf\xc3\xa9\"]x[/]",
           &tier_three_tooltip,
           "\033]8;;send:x?config=%7B%22tooltip%22%3A%22Say%20%5C%22hi%5C%22"
           "%20%5C%5C%20caf%C3%A9%22%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" title.bold title.color=red "
           "menu.4.link=\"https://example.com/guide\" "
           "menu.1.send=\"attack\" menu.2.prompt=\"examine target\" "
           "menu.3.separator menu.2.label=\"Inspect\" title=\"Combat\" "
           "menu.4.label=\"Guide\" menu.1.label=\"Attack\"]x[/]",
           &tier_three_full,
           "\033]8;;send:x?config=%7B%22menu%22%3A%5B%7B%22Attack%22%3A%22"
           "send%3Aattack%22%7D%2C%7B%22Inspect%22%3A%22prompt%3Aexamine%20"
           "target%22%7D%2C%22-%22%2C%7B%22Guide%22%3A%22https%3A%2F%2F"
           "example.com%2Fguide%22%7D%5D%2C%22title%22%3A%7B%22text%22%3A"
           "%22Combat%22%2C%22style%22%3A%7B%22color%22%3A%22%23ff0000%22"
           "%2C%22bold%22%3Atrue%7D%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" color=red tooltip=\"Tip\" title=\"Combat\" "
           "menu.1.label=\"Attack\" menu.1.send=\"attack\"]x[/]",
           &tier_three_full,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%23ff0000%22%7D%2C%22tooltip%22%3A%22Tip%22%2C%22menu%22%3A"
           "%5B%7B%22Attack%22%3A%22send%3Aattack%22%7D%5D%2C%22title%22%3A"
           "%22Combat%22%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" menu.1.separator menu.2.label=\"Web\" "
           "menu.2.link=\"https://example.com\" menu.3.separator "
           "menu.4.label=\"Attack\" menu.4.send=\"attack\" "
           "menu.5.separator menu.6.separator menu.7.label=\"Edit\" "
           "menu.7.prompt=\"look\" menu.8.separator menu.9.label=\"Flee\" "
           "menu.9.send=\"flee\" menu.10.separator title=\"Combat\"]x[/]",
           &tier_three_filtered,
           "\033]8;;send:x?config=%7B%22menu%22%3A%5B%7B%22Attack%22%3A%22"
           "send%3Aattack%22%7D%2C%22-%22%2C%7B%22Flee%22%3A%22send%3A"
           "flee%22%7D%5D%2C%22title%22%3A%22Combat%22%7D\033\\x\033]8;;"
           "\033\\") ||
       !expect_render_options(
           "[send=\"x\" title=\"Combat\" title.color=red "
           "menu.1.label=\"Attack\" menu.1.send=\"attack\"]x[/]",
           &tier_three_filtered,
           "\033]8;;send:x?config=%7B%22menu%22%3A%5B%7B%22Attack%22%3A%22"
           "send%3Aattack%22%7D%5D%2C%22title%22%3A%22Combat%22%7D\033\\x"
           "\033]8;;\033\\") ||
       !expect_render_options("[send=\"x\" menu.1.label=\"Edit\" "
                              "menu.1.prompt=\"look\"]x[/]",
                              &tier_three_filtered,
                              "\033]8;;send:x\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" tooltip=\"Tip\" menu.1.label=\"Attack\" "
           "menu.1.send=\"attack\"]x[/]",
           &no_links, "x") ||
       !expect_render_options(
           "[link=\"https://example.com/?config=old#part\" "
           "tooltip=\"Tip\"]x[/]",
           &(const StyledTextRenderOptions){.osc_hyperlinks = true,
                                            .osc_hyperlinks_tooltip = true},
           "\033]8;;https://example.com/?%63%6F%6E%66%69%67=old&config="
           "%7B%22tooltip%22%3A%22Tip%22%7D#part\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" visibility.action=conceal visibility.delay=0 "
           "visibility.expire.input visibility.expire.prompt=false "
           "visibility.expire.output visibility.expire.outputDelay=500 "
           "visibility.wholeline]x[/]",
           &tier_four_behavior,
           "\033]8;;send:x?config=%7B%22visibility%22%3A%7B%22action%22%3A"
           "%22conceal%22%2C%22delay%22%3A0%2C%22expire%22%3A%7B%22input"
           "%22%3Atrue%2C%22prompt%22%3Afalse%2C%22output%22%3Atrue%2C%22"
           "outputDelay%22%3A500%7D%2C%22wholeline%22%3Atrue%7D%7D\033\\x"
           "\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" visibility.action=reveal,conceal spoiler "
           "disabled=false]x[/]",
           &tier_four_behavior,
           "\033]8;;send:x?config=%7B%22visibility%22%3A%7B%22action%22%3A"
           "%5B%22reveal%22%2C%22conceal%22%5D%7D%2C%22spoiler%22%3Atrue"
           "%2C%22disabled%22%3Afalse%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" visibility.action=reveal visibility.wholeline=false "
           "spoiler=false]x[/]",
           &tier_four_behavior,
           "\033]8;;send:x?config=%7B%22visibility%22%3A%7B%22action%22%3A"
           "%22reveal%22%2C%22wholeline%22%3Afalse%7D%2C%22spoiler%22%3A"
           "false%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options("[send=\"x\" spoiler]x[/]", &send_only,
                              "\033]8;;send:x\033\\x\033]8;;\033\\") ||
       !expect_render_options("[send=\"x\" disabled]x[/]", &send_only, "x") ||
       !expect_render_options(
           "[send=\"x\" spoiler disabled]x[/]", &tier_four_behavior,
           "\033]8;;send:x?config=%7B%22spoiler%22%3Atrue%2C%22disabled%22"
           "%3Atrue%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[link=\"https://example.com/?config=old&a=1#part\"]x[/]",
           &(const StyledTextRenderOptions){.osc_hyperlinks = true,
                                            .osc_hyperlinks_spoiler = true},
           "\033]8;;https://example.com/?%63%6F%6E%66%69%67=old&a=1#part"
           "\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" color=red tooltip=\"Tip\" title=\"Actions\" "
           "menu.1.label=\"Look\" menu.1.send=\"look\" "
           "visibility.action=reveal visibility.delay=5 spoiler "
           "disabled=false]x[/]",
           &tier_four_full,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22color%22%3A%22"
           "%23ff0000%22%7D%2C%22tooltip%22%3A%22Tip%22%2C%22menu%22%3A"
           "%5B%7B%22Look%22%3A%22send%3Alook%22%7D%5D%2C%22title%22%3A"
           "%22Actions%22%2C%22visibility%22%3A%7B%22action%22%3A%22reveal"
           "%22%2C%22delay%22%3A5%7D%2C%22spoiler%22%3Atrue%2C%22disabled"
           "%22%3Afalse%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"react?type=like\" selection.group=\"reactions\" "
           "selection.value=\"like\"]Like[/]",
           &tier_five_selection,
           "\033]8;;send:react%3Ftype%3Dlike?config=%7B%22selection%22%3A%7B"
           "%22group%22%3A%22reactions%22%2C%22value%22%3A%22like%22%7D%7D"
           "\033\\Like\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selection.group=\"g\" selection.value=\"v\" "
           "selection.toggle=true selection.selected=false "
           "selection.exclusive=true selection.disabled=false]x[/]",
           &tier_five_selection,
           "\033]8;;send:x?config=%7B%22selection%22%3A%7B%22group%22%3A%22g"
           "%22%2C%22value%22%3A%22v%22%2C%22toggle%22%3Atrue%2C%22selected"
           "%22%3Afalse%2C%22exclusive%22%3Atrue%2C%22disabled%22%3Afalse"
           "%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selection.group=\"old\" "
           "selection.group=\"caf\xc3\xa9\" "
           "selection.value=\"old\" selection.value=\"Say \\\"yes\\\" \\\\\" "
           "selection.toggle selection.toggle=false selection.selected "
           "selection.exclusive=false selection.disabled]x[/]",
           &tier_five_selection,
           "\033]8;;send:x?config=%7B%22selection%22%3A%7B%22group%22%3A%22"
           "caf%C3%A9%22%2C%22value%22%3A%22Say%20%5C%22yes%5C%22%20%5C%5C"
           "%22%2C%22toggle%22%3Afalse%2C%22selected%22%3Atrue%2C%22exclusive"
           "%22%3Afalse%2C%22disabled%22%3Atrue%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selected.color=blue disabled.strikethrough "
           "selection.group=\"difficulty\" selection.value=\"hard\" "
           "selection.exclusive]x[/]",
           &tier_five_full,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22selected%22%3A%7B"
           "%22color%22%3A%22%230000ff%22%7D%2C%22disabled%22%3A%7B%22"
           "strikethrough%22%3Atrue%7D%7D%2C%22selection%22%3A%7B%22group%22"
           "%3A%22difficulty%22%2C%22value%22%3A%22hard%22%2C%22exclusive"
           "%22%3Atrue%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selected.bold selection.group=\"g\" "
           "selection.value=\"v\"]x[/]",
           &tier_two_states,
           "\033]8;;send:x?config=%7B%22style%22%3A%7B%22selected%22%3A%7B"
           "%22bold%22%3Atrue%7D%7D%7D\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selection.group=\"g\" selection.value=\"v\"]x[/]",
           &send_only, "\033]8;;send:x\033\\x\033]8;;\033\\") ||
       !expect_render_options(
           "[prompt=\"choose?mode=fast\" selection.group=\"mode\" "
           "selection.value=\"fast\"]Fast[/]",
           &tier_five_selection,
           "\033]8;;prompt:choose%3Fmode%3Dfast?config=%7B%22selection%22%3A"
           "%7B%22group%22%3A%22mode%22%2C%22value%22%3A%22fast%22%7D%7D"
           "\033\\Fast\033]8;;\033\\") ||
       !expect_render_options(
           "[link=\"https://example.com/?config=old#part\" "
           "selection.group=\"tabs\" selection.value=\"one\"]One[/]",
           &tier_five_selection,
           "\033]8;;https://example.com/?%63%6F%6E%66%69%67=old&config=%7B"
           "%22selection%22%3A%7B%22group%22%3A%22tabs%22%2C%22value%22%3A"
           "%22one%22%7D%7D#part\033\\One\033]8;;\033\\") ||
       !expect_render_options(
           "[link=\"https://example.com/?config=old#part\"]x[/]",
           &(const StyledTextRenderOptions){.osc_hyperlinks = true,
                                            .osc_hyperlinks_selection = true},
           "\033]8;;https://example.com/?%63%6F%6E%66%69%67=old#part\033\\x"
           "\033]8;;\033\\") ||
       !expect_render_options(
           "[send=\"x\" selection.group=\"g\" selection.value=\"v\"]x[/]",
           &(const StyledTextRenderOptions){.osc_hyperlinks_selection = true},
           "x") ||
       !expect_render_options(
           "[send=\"x\" tooltip=\"Choose\" menu.1.label=\"One\" "
           "menu.1.send=\"one\" visibility.action=conceal "
           "selection.group=\"choices\" selection.value=\"one\" spoiler=false "
           "disabled=false]x[/]",
           &tier_five_full,
           "\033]8;;send:x?config=%7B%22tooltip%22%3A%22Choose%22%2C%22menu"
           "%22%3A%5B%7B%22One%22%3A%22send%3Aone%22%7D%5D%2C%22visibility"
           "%22%3A%7B%22action%22%3A%22conceal%22%7D%2C%22selection%22%3A"
           "%7B%22group%22%3A%22choices%22%2C%22value%22%3A%22one%22%7D%2C"
           "%22spoiler%22%3Afalse%2C%22disabled%22%3Afalse%7D\033\\x\033]8;;"
           "\033\\") ||
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

  styled_text_render(styled_text_test_palette, "ab\xc3\xa9",
                     TERMINAL_COLOR_NONE, small, sizeof(small));
  if (!result && strcmp(small, "ab") != 0)
    result = 1;
  styled_text_render_with_options(styled_text_test_palette,
                                  "[send=\"x\"]abcdefghijklmnopqrstuvwxyz[/]",
                                  &send_only, small_link, sizeof(small_link));
  if (!result &&
      (!strstr(small_link, "\033]8;;send:x\033\\") || strlen(small_link) < 7 ||
       strcmp(small_link + strlen(small_link) - 7, "\033]8;;\033\\")))
    result = 1;
  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"x\" tooltip=\"Tip\"]abcdefghijklmnopqrstuvwxyz[/]",
      &tier_three_tooltip, small_tier_three, sizeof(small_tier_three));
  if (!result &&
      (!strstr(small_tier_three, "?config=") || strlen(small_tier_three) < 7 ||
       strcmp(small_tier_three + strlen(small_tier_three) - 7,
              "\033]8;;\033\\")))
    result = 1;
  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"x\" visibility.action=conceal]abcdefghijklmnopqrstuvwxyz[/]",
      &tier_four_behavior, small_tier_four, sizeof(small_tier_four));
  if (!result &&
      (!strstr(small_tier_four, "?config=") || strlen(small_tier_four) < 7 ||
       strcmp(small_tier_four + strlen(small_tier_four) - 7, "\033]8;;\033\\")))
    result = 1;
  styled_text_render_with_options(
      styled_text_test_palette,
      "[send=\"x\" selection.group=\"g\" selection.value=\"v\"]"
      "abcdefghijklmnopqrstuvwxyz[/]",
      &tier_five_selection, small_tier_five, sizeof(small_tier_five));
  if (!result &&
      (!strstr(small_tier_five, "?config=") || strlen(small_tier_five) < 7 ||
       strcmp(small_tier_five + strlen(small_tier_five) - 7, "\033]8;;\033\\")))
    result = 1;

  styled_text_palette_destroy(styled_text_test_palette);
  return result;
}
