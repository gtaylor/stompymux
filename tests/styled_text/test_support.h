/* test_support.h - Shared styled-text test assertions. */

#pragma once

#include "mux/support/styled_text/markup.h"
#include "mux/support/styled_text/render.h"

extern StyledTextPalette *styled_text_test_palette;

int expect_compile(const char *markup, const char *expected);
int expect_invalid(const char *markup);
int expect_valid(const char *markup);
int expect_render(const char *styled, TerminalColorDepth depth,
                  const char *expected);
int expect_render_options(const char *styled,
                          const StyledTextRenderOptions *options,
                          const char *expected);

int styled_text_markup_tests(void);
int styled_text_terminal_tests(void);
int styled_text_osc8_tests(void);
int styled_text_preset_tests(void);
