/* terminal.c - Terminal capability parsing tests. */

#include "test_support.h"

int styled_text_terminal_tests(void) {
  TerminalColorDepth depth;
  bool screen_reader;

  if (terminal_color_depth_from_type("xterm") != TERMINAL_COLOR_ANSI_256 ||
      terminal_color_depth_from_type("ANSI-TRUECOLOR") !=
          TERMINAL_COLOR_TRUECOLOR ||
      terminal_color_depth_from_type("DUMB") != TERMINAL_COLOR_NONE ||
      !terminal_mtts_parse("MTTS 329", &depth, &screen_reader) ||
      depth != TERMINAL_COLOR_TRUECOLOR || !screen_reader ||
      terminal_mtts_parse("MTTS nonsense", &depth, &screen_reader))
    return 1;
  return 0;
}
