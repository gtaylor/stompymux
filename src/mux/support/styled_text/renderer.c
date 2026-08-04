/* renderer.c - Terminal capability and ANSI rendering. */

#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/alloc.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/utf8.h"

bool styled_sgr_parse(const char *cursor, const char **end, int *parameters,
                      size_t *parameter_count) {
  const char *scan;
  int value = 0;
  bool have_digit = false;

  if (cursor[0] != '\033' || cursor[1] != '[')
    return false;
  scan = cursor + 2;
  *parameter_count = 0;
  while (*scan) {
    if (isdigit((unsigned char)*scan)) {
      if (value > (INT_MAX - 9) / 10)
        return false;
      value = value * 10 + (*scan - '0');
      have_digit = true;
      scan++;
      continue;
    }
    if (*scan == ';' || *scan == 'm') {
      if (*parameter_count >= SGR_PARAMETER_LIMIT)
        return false;
      parameters[(*parameter_count)++] = have_digit ? value : 0;
      value = 0;
      have_digit = false;
      if (*scan == 'm') {
        *end = scan + 1;
        return true;
      }
      scan++;
      continue;
    }
    return false;
  }
  return false;
}

const char *styled_skip_escape(const char *cursor) {
  const char *scan = cursor + 1;

  if (*scan == '[') {
    scan++;
    while (*scan &&
           ((unsigned char)*scan < 0x40 || (unsigned char)*scan > 0x7e))
      scan++;
    if (*scan)
      scan++;
    return scan;
  }
  if (*scan == ']') {
    scan++;
    while (*scan && *scan != '\a' && !(*scan == '\033' && scan[1] == '\\'))
      scan++;
    if (*scan == '\a')
      return scan + 1;
    if (*scan)
      return scan + 2;
    return scan;
  }
  return *scan ? scan + 1 : scan;
}

static const int terminal_ansi_colors[16][3] = {
    {0, 0, 0},       {128, 0, 0},   {0, 128, 0},   {128, 128, 0},
    {0, 0, 128},     {128, 0, 128}, {0, 128, 128}, {192, 192, 192},
    {128, 128, 128}, {255, 0, 0},   {0, 255, 0},   {255, 255, 0},
    {0, 0, 255},     {255, 0, 255}, {0, 255, 255}, {255, 255, 255},
};

TerminalColorDepth terminal_color_depth_from_type(const char *name) {
  if (!name)
    return TERMINAL_COLOR_ANSI_16;
  if (strcasestr(name, "TRUECOLOR"))
    return TERMINAL_COLOR_TRUECOLOR;
  if (strcasestr(name, "256COLOR") || !strcasecmp(name, "XTERM"))
    return TERMINAL_COLOR_ANSI_256;
  if (!strcasecmp(name, "DUMB"))
    return TERMINAL_COLOR_NONE;
  return TERMINAL_COLOR_ANSI_16;
}

bool terminal_mtts_parse(const char *name, TerminalColorDepth *depth,
                         bool *is_screen_reader) {
  constexpr long MTTS_ANSI = 1;
  constexpr long MTTS_256_COLORS = 8;
  constexpr long MTTS_SCREEN_READER = 64;
  constexpr long MTTS_TRUECOLOR = 256;
  char *end;
  long capabilities;

  if (!name || !depth || !is_screen_reader || strncasecmp(name, "MTTS ", 5))
    return false;
  errno = 0;
  capabilities = strtol(name + 5, &end, 10);
  if (errno != 0 || *end != '\0' || capabilities < 0)
    return false;
  *is_screen_reader = (capabilities & MTTS_SCREEN_READER) != 0;
  if (capabilities & MTTS_TRUECOLOR)
    *depth = TERMINAL_COLOR_TRUECOLOR;
  else if (capabilities & MTTS_256_COLORS)
    *depth = TERMINAL_COLOR_ANSI_256;
  else if (capabilities & MTTS_ANSI)
    *depth = TERMINAL_COLOR_ANSI_16;
  else
    *depth = TERMINAL_COLOR_NONE;
  return true;
}

static void ansi_256_rgb(int index, int *red, int *green, int *blue) {
  static const int base[16][3] = {
      {0, 0, 0},       {128, 0, 0},   {0, 128, 0},   {128, 128, 0},
      {0, 0, 128},     {128, 0, 128}, {0, 128, 128}, {192, 192, 192},
      {128, 128, 128}, {255, 0, 0},   {0, 255, 0},   {255, 255, 0},
      {0, 0, 255},     {255, 0, 255}, {0, 255, 255}, {255, 255, 255},
  };
  static const int cube[] = {0, 95, 135, 175, 215, 255};

  if (index < 0)
    index = 0;
  if (index > 255)
    index = 255;
  if (index < 16) {
    *red = base[index][0];
    *green = base[index][1];
    *blue = base[index][2];
  } else if (index < 232) {
    int cube_index = index - 16;
    *red = cube[cube_index / 36];
    *green = cube[(cube_index / 6) % 6];
    *blue = cube[cube_index % 6];
  } else {
    *red = *green = *blue = 8 + (index - 232) * 10;
  }
}

static int distance_squared(int red, int green, int blue, int other_red,
                            int other_green, int other_blue) {
  int red_delta = red - other_red;
  int green_delta = green - other_green;
  int blue_delta = blue - other_blue;
  return red_delta * red_delta + green_delta * green_delta +
         blue_delta * blue_delta;
}

static int nearest_ansi(int red, int green, int blue) {
  int best = 0;
  int best_distance = INT_MAX;

  for (int index = 0; index < 16; index++) {
    int distance = distance_squared(
        red, green, blue, terminal_ansi_colors[index][0],
        terminal_ansi_colors[index][1], terminal_ansi_colors[index][2]);
    if (distance < best_distance) {
      best = index;
      best_distance = distance;
    }
  }
  return best;
}

static int nearest_ansi_256(int red, int green, int blue) {
  int best = 0;
  int best_distance = INT_MAX;

  for (int index = 0; index < 256; index++) {
    int palette_red;
    int palette_green;
    int palette_blue;
    int distance;
    ansi_256_rgb(index, &palette_red, &palette_green, &palette_blue);
    distance = distance_squared(red, green, blue, palette_red, palette_green,
                                palette_blue);
    if (distance < best_distance) {
      best = index;
      best_distance = distance;
    }
  }
  return best;
}

static bool append_sgr(char *output, size_t output_size, size_t *used,
                       int value) {
  char sequence[24];
  int length = snprintf(sequence, sizeof(sequence), "\033[%dm", value);
  return length > 0 && styled_append_bytes(output, output_size, used, sequence,
                                           (size_t)length);
}

static void render_sgr(const int *parameters, size_t parameter_count,
                       TerminalColorDepth depth, char *output,
                       size_t output_size, size_t *used) {
  for (size_t index = 0; index < parameter_count; index++) {
    int parameter = parameters[index];
    bool foreground;
    int red;
    int green;
    int blue;

    if ((parameter == 38 || parameter == 48) && index + 1 < parameter_count) {
      foreground = parameter == 38;
      if (parameters[index + 1] == 2 && index + 4 < parameter_count) {
        red = parameters[index + 2];
        green = parameters[index + 3];
        blue = parameters[index + 4];
        index += 4;
      } else if (parameters[index + 1] == 5 && index + 2 < parameter_count) {
        ansi_256_rgb(parameters[index + 2], &red, &green, &blue);
        index += 2;
      } else {
        continue;
      }
      red = red < 0 ? 0 : red > 255 ? 255 : red;
      green = green < 0 ? 0 : green > 255 ? 255 : green;
      blue = blue < 0 ? 0 : blue > 255 ? 255 : blue;
      if (depth == TERMINAL_COLOR_TRUECOLOR) {
        char sequence[48];
        int length = snprintf(sequence, sizeof(sequence), "\033[%d;2;%d;%d;%dm",
                              foreground ? 38 : 48, red, green, blue);
        if (length > 0)
          styled_append_bytes(output, output_size, used, sequence,
                              (size_t)length);
      } else if (depth == TERMINAL_COLOR_ANSI_256) {
        char sequence[32];
        int color = nearest_ansi_256(red, green, blue);
        int length = snprintf(sequence, sizeof(sequence), "\033[%d;5;%dm",
                              foreground ? 38 : 48, color);
        if (length > 0)
          styled_append_bytes(output, output_size, used, sequence,
                              (size_t)length);
      } else if (depth == TERMINAL_COLOR_ANSI_16) {
        int color = nearest_ansi(red, green, blue);
        int code = foreground ? (color < 8 ? 30 + color : 90 + color - 8)
                              : (color < 8 ? 40 + color : 100 + color - 8);
        append_sgr(output, output_size, used, code);
      }
      continue;
    }
    if (depth != TERMINAL_COLOR_NONE)
      append_sgr(output, output_size, used, parameter);
  }
}

static bool parse_osc8(const char *cursor, const char **end, bool *is_close) {
  const char *terminator;

  if (strncmp(cursor, "\033]8;;", 5) != 0)
    return false;
  terminator = strstr(cursor + 5, "\033\\");
  if (terminator == nullptr)
    return false;
  *is_close = terminator == cursor + 5;
  *end = terminator + 2;
  return true;
}

static void styled_text_render_ansi(const char *styled,
                                    TerminalColorDepth depth, char *output,
                                    size_t output_size) {
  size_t used = 0;
  bool saw_sgr = false;
  bool link_open = false;

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;

  for (const char *cursor = styled; *cursor;) {
    if (*cursor != '\033') {
      size_t consumed;
      size_t available = link_open && output_size > OSC8_CLOSE_SIZE
                             ? output_size - OSC8_CLOSE_SIZE
                             : output_size;
      if (!styled_append_utf8_codepoint(output, available, &used, cursor,
                                        &consumed))
        break;
      cursor += consumed;
      continue;
    }
    int parameters[SGR_PARAMETER_LIMIT];
    size_t parameter_count;
    const char *end;
    if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
      size_t available = link_open && output_size > OSC8_CLOSE_SIZE
                             ? output_size - OSC8_CLOSE_SIZE
                             : output_size;
      render_sgr(parameters, parameter_count, depth, output, available, &used);
      saw_sgr = true;
      cursor = end;
    } else {
      bool is_close;

      if (parse_osc8(cursor, &end, &is_close)) {
        size_t length = (size_t)(end - cursor);
        size_t available = is_close ? output_size
                           : output_size > OSC8_CLOSE_SIZE
                               ? output_size - OSC8_CLOSE_SIZE
                               : output_size;

        if (styled_append_bytes(output, available, &used, cursor, length)) {
          link_open = !is_close;
          cursor = end;
          continue;
        }
        break;
      }
      cursor = styled_skip_escape(cursor);
    }
  }
  if (link_open)
    styled_emit_link_close(output, output_size, &used);
  if (saw_sgr && depth != TERMINAL_COLOR_NONE)
    styled_append_string(output, output_size, &used, "\033[0m");
}

void styled_text_render(const StyledTextPalette *palette, const char *styled,
                        TerminalColorDepth depth, char *output,
                        size_t output_size) {
  StyledTextRenderOptions options = {.color_depth = depth};

  styled_text_render_with_options(palette, styled, &options, output,
                                  output_size);
}

void styled_text_render_with_options(const StyledTextPalette *palette,
                                     const char *styled,
                                     const StyledTextRenderOptions *options,
                                     char *output, size_t output_size) {
  char compiled[LBUF_SIZE];

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;
  styled_text_compile_permissive(palette, styled, compiled, sizeof(compiled),
                                 options);
  styled_text_render_ansi(compiled,
                          options ? options->color_depth : TERMINAL_COLOR_NONE,
                          output, output_size);
}

size_t styled_text_width(const StyledTextPalette *palette, const char *styled) {
  char plain[LBUF_SIZE];

  if (!styled)
    return 0;
  styled_text_render(palette, styled, TERMINAL_COLOR_NONE, plain,
                     sizeof(plain));
  return strlen(plain);
}

void styled_text_strip(const StyledTextPalette *palette, const char *styled,
                       char *output, size_t output_size) {
  styled_text_render(palette, styled, TERMINAL_COLOR_NONE, output, output_size);
}
