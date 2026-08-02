/* styled_text.c - Safe color markup and terminal-specific ANSI rendering. */

#include "mux/server/platform.h"

#include <limits.h>
#include <stdlib.h>

#include "mux/support/alloc.h"
#include "mux/support/styled_text.h"
#include "mux/support/utf8.h"

typedef enum StyledColorKind {
  STYLED_COLOR_DEFAULT,
  STYLED_COLOR_ANSI,
  STYLED_COLOR_RGB,
} StyledColorKind;

typedef struct StyledColor {
  StyledColorKind kind;
  int value;
  int red;
  int green;
  int blue;
} StyledColor;

typedef struct StyledState {
  StyledColor foreground;
  StyledColor background;
  bool bold;
  bool blink;
  bool underline;
  bool inverse;
} StyledState;

typedef struct NamedColor {
  const char *name;
  int ansi;
  int red;
  int green;
  int blue;
} NamedColor;

typedef struct CustomNamedColor {
  char *name;
  int red;
  int green;
  int blue;
} CustomNamedColor;

struct StyledTextPalette {
  CustomNamedColor *colors;
  size_t count;
  size_t capacity;
};

static const NamedColor built_in_colors[] = {
    {"black", 0, 0, 0, 0},
    {"red", 1, 205, 0, 0},
    {"green", 2, 0, 205, 0},
    {"yellow", 3, 205, 205, 0},
    {"blue", 4, 0, 0, 238},
    {"magenta", 5, 205, 0, 205},
    {"cyan", 6, 0, 205, 205},
    {"white", 7, 229, 229, 229},
    {"bright-black", 8, 127, 127, 127},
    {"bright-red", 9, 255, 0, 0},
    {"bright-green", 10, 0, 255, 0},
    {"bright-yellow", 11, 255, 255, 0},
    {"bright-blue", 12, 92, 92, 255},
    {"bright-magenta", 13, 255, 0, 255},
    {"bright-cyan", 14, 0, 255, 255},
    {"bright-white", 15, 255, 255, 255},
    {"gray", 8, 127, 127, 127},
    {"grey", 8, 127, 127, 127},
    {nullptr, 0, 0, 0, 0},
};

enum { STYLE_STACK_LIMIT = 32, SGR_PARAMETER_LIMIT = 32 };

static const char *skip_escape(const char *cursor);
static void set_error(char *error, size_t error_size, const char *message);

StyledTextPalette *styled_text_palette_create(void) {
  return calloc(1, sizeof(StyledTextPalette));
}

void styled_text_palette_destroy(StyledTextPalette *palette) {
  if (!palette)
    return;
  for (size_t index = 0; index < palette->count; index++)
    free(palette->colors[index].name);
  free(palette->colors);
  free(palette);
}

static bool styled_text_color_name_valid(const char *name) {
  size_t length;

  if (!name || !*name)
    return false;
  length = strlen(name);
  if (length > 60)
    return false;
  for (const char *cursor = name; *cursor; cursor++) {
    unsigned char ch = (unsigned char)*cursor;
    if (!isalnum(ch) && ch != '-' && ch != '_')
      return false;
  }
  return true;
}

bool styled_text_palette_set_rgb(StyledTextPalette *palette, const char *name,
                                 int red, int green, int blue, char *error,
                                 size_t error_size) {
  CustomNamedColor *entry;

  if (error && error_size > 0)
    error[0] = '\0';
  if (!palette) {
    set_error(error, error_size, "color palette is not available");
    return false;
  }
  if (!styled_text_color_name_valid(name)) {
    set_error(error, error_size,
              "color name must use 1-60 letters, digits, '-' or '_'");
    return false;
  }
  if (red < 0 || red > 255 || green < 0 || green > 255 || blue < 0 ||
      blue > 255) {
    set_error(error, error_size, "RGB channels must be between 0 and 255");
    return false;
  }
  for (size_t index = 0; index < palette->count; index++) {
    if (!strcasecmp(name, palette->colors[index].name)) {
      palette->colors[index].red = red;
      palette->colors[index].green = green;
      palette->colors[index].blue = blue;
      return true;
    }
  }
  if (palette->count == palette->capacity) {
    size_t capacity = palette->capacity ? palette->capacity * 2 : 16;
    CustomNamedColor *colors =
        realloc(palette->colors, capacity * sizeof(*colors));
    if (!colors) {
      set_error(error, error_size, "unable to allocate named color");
      return false;
    }
    palette->colors = colors;
    palette->capacity = capacity;
  }
  entry = &palette->colors[palette->count];
  entry->name = strdup(name);
  if (!entry->name) {
    set_error(error, error_size, "unable to allocate named color");
    return false;
  }
  entry->red = red;
  entry->green = green;
  entry->blue = blue;
  palette->count++;
  return true;
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

static bool append_bytes(char *output, size_t output_size, size_t *used,
                         const char *value, size_t length) {
  if (*used + length >= output_size)
    return false;
  memcpy(output + *used, value, length);
  *used += length;
  output[*used] = '\0';
  return true;
}

static bool append_string(char *output, size_t output_size, size_t *used,
                          const char *value) {
  return append_bytes(output, output_size, used, value, strlen(value));
}

static bool append_utf8_codepoint(char *output, size_t output_size,
                                  size_t *used, const char *value,
                                  size_t *consumed) {
  static const char replacement[] = "\xef\xbf\xbd";
  Utf8DecodeResult decoded;
  size_t available = strnlen(value, 4);

  if (utf8_decode(value, available, &decoded)) {
    *consumed = decoded.length;
    return append_bytes(output, output_size, used, value, decoded.length);
  }
  *consumed = 1;
  return append_bytes(output, output_size, used, replacement,
                      sizeof(replacement) - 1);
}

static void set_error(char *error, size_t error_size, const char *message) {
  if (error && error_size > 0)
    snprintf(error, error_size, "%s", message);
}

static bool parse_hex_byte(const char *value, int *result) {
  int high;
  int low;

  if (!isxdigit((unsigned char)value[0]) || !isxdigit((unsigned char)value[1]))
    return false;
  high = isdigit((unsigned char)value[0])
             ? value[0] - '0'
             : tolower((unsigned char)value[0]) - 'a' + 10;
  low = isdigit((unsigned char)value[1])
            ? value[1] - '0'
            : tolower((unsigned char)value[1]) - 'a' + 10;
  *result = high * 16 + low;
  return true;
}

static bool parse_color(const StyledTextPalette *palette, const char *value,
                        StyledColor *color) {
  if (value[0] == '#' && strlen(value) == 7) {
    if (!parse_hex_byte(value + 1, &color->red) ||
        !parse_hex_byte(value + 3, &color->green) ||
        !parse_hex_byte(value + 5, &color->blue))
      return false;
    color->kind = STYLED_COLOR_RGB;
    color->value = 0;
    return true;
  }

  if (palette) {
    for (size_t index = 0; index < palette->count; index++) {
      const CustomNamedColor *named = &palette->colors[index];
      if (strcasecmp(value, named->name) != 0)
        continue;
      *color = (StyledColor){
          .kind = STYLED_COLOR_RGB,
          .value = 0,
          .red = named->red,
          .green = named->green,
          .blue = named->blue,
      };
      return true;
    }
  }

  for (const NamedColor *named = built_in_colors; named->name; named++) {
    if (strcasecmp(value, named->name) != 0)
      continue;
    *color = (StyledColor){
        .kind = STYLED_COLOR_ANSI,
        .value = named->ansi,
        .red = named->red,
        .green = named->green,
        .blue = named->blue,
    };
    return true;
  }
  return false;
}

static bool emit_state(const StyledState *state, char *output,
                       size_t output_size, size_t *used) {
  char sequence[64];

  if (!append_string(output, output_size, used, "\033[0m"))
    return false;
  if (state->bold && !append_string(output, output_size, used, "\033[1m"))
    return false;
  if (state->blink && !append_string(output, output_size, used, "\033[5m"))
    return false;
  if (state->underline && !append_string(output, output_size, used, "\033[4m"))
    return false;
  if (state->inverse && !append_string(output, output_size, used, "\033[7m"))
    return false;
  if (state->foreground.kind == STYLED_COLOR_ANSI) {
    int code = state->foreground.value < 8 ? 30 + state->foreground.value
                                           : 90 + state->foreground.value - 8;
    snprintf(sequence, sizeof(sequence), "\033[%dm", code);
    if (!append_string(output, output_size, used, sequence))
      return false;
  } else if (state->foreground.kind == STYLED_COLOR_RGB) {
    snprintf(sequence, sizeof(sequence), "\033[38;2;%d;%d;%dm",
             state->foreground.red, state->foreground.green,
             state->foreground.blue);
    if (!append_string(output, output_size, used, sequence))
      return false;
  }
  if (state->background.kind == STYLED_COLOR_ANSI) {
    int code = state->background.value < 8 ? 40 + state->background.value
                                           : 100 + state->background.value - 8;
    snprintf(sequence, sizeof(sequence), "\033[%dm", code);
    if (!append_string(output, output_size, used, sequence))
      return false;
  } else if (state->background.kind == STYLED_COLOR_RGB) {
    snprintf(sequence, sizeof(sequence), "\033[48;2;%d;%d;%dm",
             state->background.red, state->background.green,
             state->background.blue);
    if (!append_string(output, output_size, used, sequence))
      return false;
  }
  return true;
}

static bool apply_style_directive(const StyledTextPalette *palette,
                                  const char *directive, StyledState *state,
                                  char *error, size_t error_size) {
  StyledColor color;

  if (!strcasecmp(directive, "bold")) {
    state->bold = true;
  } else if (!strcasecmp(directive, "blink")) {
    state->blink = true;
  } else if (!strcasecmp(directive, "underline")) {
    state->underline = true;
  } else if (!strcasecmp(directive, "inverse")) {
    state->inverse = true;
  } else if (!strncasecmp(directive, "fg=", 3)) {
    if (!parse_color(palette, directive + 3, &color)) {
      set_error(error, error_size, "unknown foreground color");
      return false;
    }
    state->foreground = color;
  } else if (!strncasecmp(directive, "bg=", 3)) {
    if (!parse_color(palette, directive + 3, &color)) {
      set_error(error, error_size, "unknown background color");
      return false;
    }
    state->background = color;
  } else if (!strcmp(directive, "/") || !strcasecmp(directive, "reset")) {
    set_error(error, error_size,
              "style close and reset tags cannot be combined");
    return false;
  } else {
    set_error(error, error_size, "unknown style tag");
    return false;
  }
  return true;
}

static bool apply_tag(const StyledTextPalette *palette, const char *tag,
                      StyledState *state, StyledState *stack,
                      size_t *stack_size, char *output, size_t output_size,
                      size_t *used, char *error, size_t error_size) {
  const char *start = tag;
  const char *end;
  StyledState updated = *state;
  bool have_directive = false;

  while (isspace((unsigned char)*start))
    start++;
  end = start + strlen(start);
  while (end > start && isspace((unsigned char)end[-1]))
    end--;

  if (end - start == 1 && *start == '/') {
    if (*stack_size == 0) {
      set_error(error, error_size, "style close tag has no matching open tag");
      return false;
    }
    *state = stack[--*stack_size];
    return emit_state(state, output, output_size, used);
  }
  if (end - start == 5 && !strncasecmp(start, "reset", 5)) {
    *state = (StyledState){0};
    *stack_size = 0;
    return emit_state(state, output, output_size, used);
  }
  if (*stack_size >= STYLE_STACK_LIMIT) {
    set_error(error, error_size, "style nesting is too deep");
    return false;
  }

  while (start < end) {
    const char *directive_end;
    char directive[64];
    size_t directive_length;

    while (start < end && isspace((unsigned char)*start))
      start++;
    if (start == end)
      break;
    directive_end = start;
    while (directive_end < end && !isspace((unsigned char)*directive_end))
      directive_end++;
    directive_length = (size_t)(directive_end - start);
    memcpy(directive, start, directive_length);
    directive[directive_length] = '\0';
    if (!apply_style_directive(palette, directive, &updated, error, error_size))
      return false;
    have_directive = true;
    start = directive_end;
  }
  if (!have_directive) {
    set_error(error, error_size, "unknown style tag");
    return false;
  }

  stack[(*stack_size)++] = *state;
  *state = updated;
  return emit_state(state, output, output_size, used);
}

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size) {
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;

  if (!markup || !output || output_size == 0) {
    set_error(error, error_size, "invalid style input");
    return false;
  }
  output[0] = '\0';
  if (error && error_size > 0)
    error[0] = '\0';

  for (const char *cursor = markup; *cursor;) {
    if ((unsigned char)*cursor == 0x1b) {
      set_error(error, error_size, "literal escape sequences are not allowed");
      return false;
    }
    if (*cursor != '[') {
      Utf8DecodeResult decoded;
      if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded)) {
        set_error(error, error_size, "text is not valid UTF-8");
        return false;
      }
      if (!append_bytes(output, output_size, &used, cursor, decoded.length))
        goto too_long;
      cursor += decoded.length;
      continue;
    }
    if (cursor[1] == '[') {
      if (!append_bytes(output, output_size, &used, "[", 1))
        goto too_long;
      cursor += 2;
      continue;
    }

    const char *close = strchr(cursor + 1, ']');
    char tag[64];
    size_t tag_length;
    if (!close) {
      set_error(error, error_size, "unterminated style tag");
      return false;
    }
    tag_length = (size_t)(close - cursor - 1);
    if (tag_length == 0 || tag_length >= sizeof(tag)) {
      set_error(error, error_size, "invalid style tag");
      return false;
    }
    memcpy(tag, cursor + 1, tag_length);
    tag[tag_length] = '\0';
    if (!apply_tag(palette, tag, &state, stack, &stack_size, output,
                   output_size, &used, error, error_size)) {
      if (error && error[0] == '\0')
        goto too_long;
      return false;
    }
    cursor = close + 1;
  }
  if (stack_size != 0) {
    set_error(error, error_size, "style tag is not closed");
    return false;
  }
  return true;

too_long:
  set_error(error, error_size, "styled text is too long");
  return false;
}

bool styled_text_escape(const char *text, char *output, size_t output_size) {
  size_t used = 0;

  if (!text || !output || output_size == 0)
    return false;
  output[0] = '\0';
  for (const char *cursor = text; *cursor;) {
    Utf8DecodeResult decoded;

    if (*cursor == '\033')
      return false;
    if (*cursor == '[' && !append_bytes(output, output_size, &used, "[", 1))
      return false;
    if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded) ||
        !append_bytes(output, output_size, &used, cursor, decoded.length))
      return false;
    cursor += decoded.length;
  }
  return true;
}

static bool parse_sgr(const char *cursor, const char **end, int *parameters,
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
        red, green, blue, built_in_colors[index].red,
        built_in_colors[index].green, built_in_colors[index].blue);
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
  return length > 0 &&
         append_bytes(output, output_size, used, sequence, (size_t)length);
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
          append_bytes(output, output_size, used, sequence, (size_t)length);
      } else if (depth == TERMINAL_COLOR_ANSI_256) {
        char sequence[32];
        int color = nearest_ansi_256(red, green, blue);
        int length = snprintf(sequence, sizeof(sequence), "\033[%d;5;%dm",
                              foreground ? 38 : 48, color);
        if (length > 0)
          append_bytes(output, output_size, used, sequence, (size_t)length);
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

static const char *skip_escape(const char *cursor) {
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

static void styled_text_render_ansi(const char *styled,
                                    TerminalColorDepth depth, char *output,
                                    size_t output_size) {
  size_t used = 0;
  bool saw_sgr = false;

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;

  for (const char *cursor = styled; *cursor;) {
    if (*cursor != '\033') {
      size_t consumed;
      if (!append_utf8_codepoint(output, output_size, &used, cursor, &consumed))
        break;
      cursor += consumed;
      continue;
    }
    int parameters[SGR_PARAMETER_LIMIT];
    size_t parameter_count;
    const char *end;
    if (parse_sgr(cursor, &end, parameters, &parameter_count)) {
      render_sgr(parameters, parameter_count, depth, output, output_size,
                 &used);
      saw_sgr = true;
      cursor = end;
    } else {
      cursor = skip_escape(cursor);
    }
  }
  if (saw_sgr && depth != TERMINAL_COLOR_NONE)
    append_string(output, output_size, &used, "\033[0m");
}

static void compile_markup_permissive(const StyledTextPalette *palette,
                                      const char *input, char *output,
                                      size_t output_size) {
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;

  output[0] = '\0';
  for (const char *cursor = input; *cursor;) {
    if (*cursor != '[') {
      size_t consumed;
      if (!append_utf8_codepoint(output, output_size, &used, cursor, &consumed))
        return;
      cursor += consumed;
      continue;
    }
    if (cursor[1] == '[') {
      if (!append_bytes(output, output_size, &used, "[", 1))
        return;
      cursor += 2;
      continue;
    }

    const char *close = strchr(cursor + 1, ']');
    size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
    if (close && tag_length > 0 && tag_length < 64) {
      StyledState candidate_state = state;
      StyledState candidate_stack[STYLE_STACK_LIMIT];
      size_t candidate_stack_size = stack_size;
      char rendered[512] = "";
      size_t rendered_size = 0;
      char error[128] = "";
      char tag[64];

      memcpy(tag, cursor + 1, tag_length);
      tag[tag_length] = '\0';
      memcpy(candidate_stack, stack, sizeof(stack));
      if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                    &candidate_stack_size, rendered, sizeof(rendered),
                    &rendered_size, error, sizeof(error))) {
        if (!append_string(output, output_size, &used, rendered))
          return;
        state = candidate_state;
        memcpy(stack, candidate_stack, sizeof(stack));
        stack_size = candidate_stack_size;
        cursor = close + 1;
        continue;
      }
    }
    if (!append_bytes(output, output_size, &used, cursor, 1))
      return;
    cursor++;
  }
}

void styled_text_render(const StyledTextPalette *palette, const char *styled,
                        TerminalColorDepth depth, char *output,
                        size_t output_size) {
  char compiled[LBUF_SIZE];

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;
  compile_markup_permissive(palette, styled, compiled, sizeof(compiled));
  styled_text_render_ansi(compiled, depth, output, output_size);
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

void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size) {
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;
  size_t visible = 0;
  bool saw_sgr = false;

  if (!output || output_size == 0)
    return;
  output[0] = '\0';
  if (!styled)
    return;
  for (const char *cursor = styled; *cursor && visible < width;) {
    if (*cursor == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;
      if (parse_sgr(cursor, &end, parameters, &parameter_count)) {
        if (!append_bytes(output, output_size, &used, cursor,
                          (size_t)(end - cursor)))
          break;
        saw_sgr = true;
        cursor = end;
      } else {
        cursor = skip_escape(cursor);
      }
    } else if (*cursor == '[' && cursor[1] == '[') {
      if (!append_bytes(output, output_size, &used, cursor, 2))
        break;
      cursor += 2;
      visible++;
    } else if (*cursor == '[') {
      const char *close = strchr(cursor + 1, ']');
      size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
      bool applied = false;

      if (close && tag_length > 0 && tag_length < 64) {
        StyledState candidate_state = state;
        StyledState candidate_stack[STYLE_STACK_LIMIT];
        size_t candidate_stack_size = stack_size;
        char rendered[512] = "";
        size_t rendered_size = 0;
        char error[128] = "";
        char tag[64];

        memcpy(tag, cursor + 1, tag_length);
        tag[tag_length] = '\0';
        memcpy(candidate_stack, stack, sizeof(stack));
        if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                      &candidate_stack_size, rendered, sizeof(rendered),
                      &rendered_size, error, sizeof(error)) &&
            append_bytes(output, output_size, &used, cursor,
                         (size_t)(close - cursor + 1))) {
          state = candidate_state;
          memcpy(stack, candidate_stack, sizeof(stack));
          stack_size = candidate_stack_size;
          cursor = close + 1;
          applied = true;
        }
      }
      if (!applied) {
        if (!append_bytes(output, output_size, &used, cursor, 1))
          break;
        cursor++;
        visible++;
      }
    } else {
      Utf8DecodeResult decoded;
      if (!utf8_decode(cursor, strnlen(cursor, 4), &decoded) ||
          visible + decoded.length > width ||
          !append_bytes(output, output_size, &used, cursor, decoded.length))
        break;
      cursor += decoded.length;
      visible += decoded.length;
    }
  }
  while (stack_size > 0) {
    if (!append_string(output, output_size, &used, "[/]"))
      break;
    stack_size--;
  }
  if (saw_sgr)
    append_string(output, output_size, &used, "\033[0m");
}
