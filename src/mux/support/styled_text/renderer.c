/* renderer.c - Terminal capability and ANSI rendering. */

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/styled_text/palette.h"
#include "mux/support/styled_text/render.h"

static const char *renderer_suffix(const char *text, size_t length,
                                   size_t offset) {
  return checked_storage_at_const(text, length + 1, sizeof(char), offset);
}

static char renderer_character(const char *text, size_t length, size_t index) {
  return *renderer_suffix(text, length, index);
}

static int *renderer_parameter(int *parameters, size_t index) {
  return checked_storage_at(parameters, SGR_PARAMETER_LIMIT,
                            sizeof(*parameters), index);
}

static int renderer_parameter_value(const int *parameters,
                                    size_t parameter_count, size_t index) {
  return *(const int *)checked_storage_at_const(parameters, parameter_count,
                                                sizeof(*parameters), index);
}

static int renderer_clamp_color(int color) {
  if (color < 0)
    return 0;
  return color > 255 ? 255 : color;
}

bool styled_sgr_parse(const char *cursor, const char **end, int *parameters,
                      size_t *parameter_count) {
  const size_t LENGTH = strlen(cursor);
  size_t offset = 2;
  int value = 0;
  bool have_digit = false;

  if (LENGTH < 2 || renderer_character(cursor, LENGTH, 0) != '\033' ||
      renderer_character(cursor, LENGTH, 1) != '[')
    return false;
  *parameter_count = 0;
  while (offset < LENGTH) {
    const char CHARACTER = renderer_character(cursor, LENGTH, offset);
    if ((isdigit)((unsigned char)CHARACTER)) {
      if (value > (INT_MAX - 9) / 10)
        return false;
      value = value * 10 + (CHARACTER - '0');
      have_digit = true;
      offset++;
      continue;
    }
    if (CHARACTER == ';' || CHARACTER == 'm') {
      if (*parameter_count >= SGR_PARAMETER_LIMIT)
        return false;
      *renderer_parameter(parameters, (*parameter_count)++) =
          have_digit ? value : 0;
      value = 0;
      have_digit = false;
      if (CHARACTER == 'm') {
        *end = renderer_suffix(cursor, LENGTH, offset + 1);
        return true;
      }
      offset++;
      continue;
    }
    return false;
  }
  return false;
}

const char *styled_skip_escape(const char *cursor) {
  const size_t LENGTH = strlen(cursor);
  size_t offset = LENGTH > 0 ? 1 : 0;

  if (offset < LENGTH && renderer_character(cursor, LENGTH, offset) == '[') {
    offset++;
    while (offset < LENGTH &&
           ((unsigned char)renderer_character(cursor, LENGTH, offset) < 0x40 ||
            (unsigned char)renderer_character(cursor, LENGTH, offset) > 0x7e))
      offset++;
    if (offset < LENGTH)
      offset++;
    return renderer_suffix(cursor, LENGTH, offset);
  }
  if (offset < LENGTH && renderer_character(cursor, LENGTH, offset) == ']') {
    offset++;
    while (offset < LENGTH &&
           renderer_character(cursor, LENGTH, offset) != '\a' &&
           !(renderer_character(cursor, LENGTH, offset) == '\033' &&
             offset + 1 < LENGTH &&
             renderer_character(cursor, LENGTH, offset + 1) == '\\'))
      offset++;
    if (offset < LENGTH && renderer_character(cursor, LENGTH, offset) == '\a')
      offset++;
    else if (offset < LENGTH)
      offset += 2;
    return renderer_suffix(cursor, LENGTH, offset);
  }
  if (offset < LENGTH)
    offset++;
  return renderer_suffix(cursor, LENGTH, offset);
}

static const int TERMINAL_ANSI_COLORS[16][3] = {
    {0, 0, 0},       {128, 0, 0},   {0, 128, 0},   {128, 128, 0},
    {0, 0, 128},     {128, 0, 128}, {0, 128, 128}, {192, 192, 192},
    {128, 128, 128}, {255, 0, 0},   {0, 255, 0},   {255, 255, 0},
    {0, 0, 255},     {255, 0, 255}, {0, 255, 255}, {255, 255, 255},
};

static int color_channel(const int colors[][3], size_t color_count,
                         size_t color, size_t channel) {
  const int *entry =
      checked_storage_at_const(colors, color_count, sizeof(*colors), color);
  return *(const int *)checked_storage_at_const(entry, 3, sizeof(*entry),
                                                channel);
}

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

  if (!name || !depth || !is_screen_reader ||
      strncasecmp(name, "MTTS ", 5) != 0)
    return false;
  errno = 0;
  capabilities = strtol(checked_string_suffix(name, 5), &end, 10);
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
  static const int CUBE[] = {0, 95, 135, 175, 215, 255};

  if (index < 0)
    index = 0;
  if (index > 255)
    index = 255;
  if (index < 16) {
    *red = color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 0);
    *green = color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 1);
    *blue = color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 2);
  } else if (index < 232) {
    int cube_index = index - 16;
    *red = *(const int *)checked_storage_at_const(CUBE, 6, sizeof(*CUBE),
                                                  (size_t)(cube_index / 36));
    *green = *(const int *)checked_storage_at_const(
        CUBE, 6, sizeof(*CUBE), (size_t)((cube_index / 6) % 6));
    *blue = *(const int *)checked_storage_at_const(CUBE, 6, sizeof(*CUBE),
                                                   (size_t)(cube_index % 6));
  } else {
    *red = *green = *blue = 8 + (index - 232) * 10;
  }
}

typedef struct RgbColor {
  int red;
  int green;
  int blue;
} RgbColor;

typedef struct ColorDistanceRequest {
  RgbColor first;
  RgbColor second;
} ColorDistanceRequest;

static int distance_squared(const ColorDistanceRequest *request) {
  int red_delta = request->first.red - request->second.red;
  int green_delta = request->first.green - request->second.green;
  int blue_delta = request->first.blue - request->second.blue;
  return red_delta * red_delta + green_delta * green_delta +
         blue_delta * blue_delta;
}

static int nearest_ansi(int red, int green, int blue) {
  int best = 0;
  int best_distance = INT_MAX;

  for (int index = 0; index < 16; index++) {
    int distance = distance_squared(&(ColorDistanceRequest){
        .first = {.red = red, .green = green, .blue = blue},
        .second = {
            .red = color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 0),
            .green = color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 1),
            .blue =
                color_channel(TERMINAL_ANSI_COLORS, 16, (size_t)index, 2)}});
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
    distance = distance_squared(&(ColorDistanceRequest){
        .first = {.red = red, .green = green, .blue = blue},
        .second = {
            .red = palette_red, .green = palette_green, .blue = palette_blue}});
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

typedef struct SgrRenderRequest {
  const int *parameters;
  size_t parameter_count;
  TerminalColorDepth depth;
  char *output;
  size_t output_size;
  size_t *used;
} SgrRenderRequest;

static void render_sgr(const SgrRenderRequest *request) {
  const int *parameters = request->parameters;
  size_t parameter_count = request->parameter_count;
  TerminalColorDepth depth = request->depth;
  char *output = request->output;
  size_t output_size = request->output_size;
  size_t *used = request->used;
  for (size_t index = 0; index < parameter_count; index++) {
    int parameter =
        renderer_parameter_value(parameters, parameter_count, index);
    bool foreground;
    int red;
    int green;
    int blue;

    if ((parameter == 38 || parameter == 48) && index + 1 < parameter_count) {
      foreground = parameter == 38;
      if (renderer_parameter_value(parameters, parameter_count, index + 1) ==
              2 &&
          index + 4 < parameter_count) {
        red = renderer_parameter_value(parameters, parameter_count, index + 2);
        green =
            renderer_parameter_value(parameters, parameter_count, index + 3);
        blue = renderer_parameter_value(parameters, parameter_count, index + 4);
        index += 4;
      } else if (renderer_parameter_value(parameters, parameter_count,
                                          index + 1) == 5 &&
                 index + 2 < parameter_count) {
        ansi_256_rgb(
            renderer_parameter_value(parameters, parameter_count, index + 2),
            &red, &green, &blue);
        index += 2;
      } else {
        continue;
      }
      red = renderer_clamp_color(red);
      green = renderer_clamp_color(green);
      blue = renderer_clamp_color(blue);
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
        int code;
        if (foreground)
          code = color < 8 ? 30 + color : 90 + color - 8;
        else
          code = color < 8 ? 40 + color : 100 + color - 8;
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
  terminator = strstr(checked_string_suffix(cursor, 5), "\033\\");
  if (terminator == nullptr)
    return false;
  *is_close = strlen(cursor) - strlen(terminator) == 5;
  *end = checked_string_suffix(terminator, 2);
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

  const size_t STYLED_LENGTH = strlen(styled);
  for (size_t cursor_offset = 0; cursor_offset < STYLED_LENGTH;) {
    const char *cursor = renderer_suffix(styled, STYLED_LENGTH, cursor_offset);
    if (renderer_character(styled, STYLED_LENGTH, cursor_offset) != '\033') {
      size_t consumed;
      size_t available = link_open && output_size > OSC8_CLOSE_SIZE
                             ? output_size - OSC8_CLOSE_SIZE
                             : output_size;
      if (!styled_append_utf8_codepoint(output, available, &used, cursor,
                                        &consumed))
        break;
      cursor_offset += consumed;
      continue;
    }
    int parameters[SGR_PARAMETER_LIMIT];
    size_t parameter_count;
    const char *end;
    if (styled_sgr_parse(cursor, &end, parameters, &parameter_count)) {
      size_t available = link_open && output_size > OSC8_CLOSE_SIZE
                             ? output_size - OSC8_CLOSE_SIZE
                             : output_size;
      render_sgr(&(SgrRenderRequest){.parameters = parameters,
                                     .parameter_count = parameter_count,
                                     .depth = depth,
                                     .output = output,
                                     .output_size = available,
                                     .used = &used});
      saw_sgr = true;
      cursor_offset += strlen(cursor) - strlen(end);
    } else {
      bool is_close;

      if (parse_osc8(cursor, &end, &is_close)) {
        size_t length = strlen(cursor) - strlen(end);
        size_t available = output_size;
        if (!is_close && output_size > OSC8_CLOSE_SIZE)
          available = output_size - OSC8_CLOSE_SIZE;

        if (styled_append_bytes(output, available, &used, cursor, length)) {
          link_open = !is_close;
          cursor_offset += length;
          continue;
        }
        break;
      }
      cursor_offset += strlen(cursor) - strlen(styled_skip_escape(cursor));
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
