/* styled_text.c - Safe markup and capability-aware terminal rendering. */

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
  bool has_link;
  bool link_emitted;
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

enum {
  STYLE_STACK_LIMIT = 32,
  SGR_PARAMETER_LIMIT = 32,
  OSC8_URI_LIMIT = 4096,
  OSC8_CLOSE_SIZE = 7,
};

typedef enum StyledLinkKind {
  STYLED_LINK_EXTERNAL,
  STYLED_LINK_SEND,
  STYLED_LINK_PROMPT,
} StyledLinkKind;

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

static size_t styled_output_size(const StyledState *state, size_t output_size) {
  if (!state->link_emitted || output_size <= OSC8_CLOSE_SIZE)
    return output_size;
  return output_size - OSC8_CLOSE_SIZE;
}

static bool styled_format_equal(const StyledState *left,
                                const StyledState *right) {
  return left->foreground.kind == right->foreground.kind &&
         left->foreground.value == right->foreground.value &&
         left->foreground.red == right->foreground.red &&
         left->foreground.green == right->foreground.green &&
         left->foreground.blue == right->foreground.blue &&
         left->background.kind == right->background.kind &&
         left->background.value == right->background.value &&
         left->background.red == right->background.red &&
         left->background.green == right->background.green &&
         left->background.blue == right->background.blue &&
         left->bold == right->bold && left->blink == right->blink &&
         left->underline == right->underline && left->inverse == right->inverse;
}

static const char *find_tag_close(const char *start) {
  bool quoted = false;
  bool escaped = false;

  for (const char *cursor = start; *cursor; cursor++) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (quoted && *cursor == '\\') {
      escaped = true;
      continue;
    }
    if (*cursor == '"') {
      quoted = !quoted;
      continue;
    }
    if (!quoted && *cursor == ']')
      return cursor;
  }
  return nullptr;
}

static bool uri_unreserved(unsigned char byte) {
  return (byte >= 'A' && byte <= 'Z') || (byte >= 'a' && byte <= 'z') ||
         (byte >= '0' && byte <= '9') || byte == '-' || byte == '.' ||
         byte == '_' || byte == '~';
}

static bool uri_reserved(unsigned char byte) {
  return strchr(":/?#[]@!$&'()*+,;=", byte) != nullptr;
}

static bool link_target_unquote(const char *start, const char *end,
                                char *target, size_t target_size, char *error,
                                size_t error_size) {
  size_t used = 0;

  if (start == end || *start != '"') {
    set_error(error, error_size, "link target must be double quoted");
    return false;
  }
  start++;
  while (start < end && *start != '"') {
    unsigned char byte = (unsigned char)*start++;

    if (byte == '\\') {
      if (start == end || (*start != '\\' && *start != '"')) {
        set_error(error, error_size, "invalid escape in link target");
        return false;
      }
      byte = (unsigned char)*start++;
    }
    if (byte < 0x20 || byte == 0x7f) {
      set_error(error, error_size, "link target contains a control byte");
      return false;
    }
    if (used + 1 >= target_size) {
      set_error(error, error_size, "link target is too long");
      return false;
    }
    target[used++] = (char)byte;
  }
  if (start == end || *start != '"') {
    set_error(error, error_size, "unterminated quoted link target");
    return false;
  }
  start++;
  while (start < end && isspace((unsigned char)*start))
    start++;
  if (start != end) {
    set_error(error, error_size, "unexpected text after link target");
    return false;
  }
  target[used] = '\0';
  if (used == 0) {
    set_error(error, error_size, "link target must not be empty");
    return false;
  }
  if (!utf8_validate_printable(target, used)) {
    set_error(error, error_size, "link target must be printable, valid UTF-8");
    return false;
  }
  return true;
}

static bool external_uri_valid(const char *uri, char *error,
                               size_t error_size) {
  size_t length = strlen(uri);
  const char *body;

  if (!strncasecmp(uri, "http:", 5))
    body = uri + 5;
  else if (!strncasecmp(uri, "https:", 6))
    body = uri + 6;
  else if (!strncasecmp(uri, "ftp:", 4))
    body = uri + 4;
  else {
    set_error(error, error_size, "link URI scheme must be http, https, or ftp");
    return false;
  }
  if (*body == '\0') {
    set_error(error, error_size, "link URI must include a destination");
    return false;
  }
  if (length > OSC8_URI_LIMIT) {
    set_error(error, error_size, "link URI is too long");
    return false;
  }
  for (size_t index = 0; index < length; index++) {
    unsigned char byte = (unsigned char)uri[index];

    if (byte == '%' && index + 2 < length &&
        isxdigit((unsigned char)uri[index + 1]) &&
        isxdigit((unsigned char)uri[index + 2])) {
      index += 2;
      continue;
    }
    if (byte >= 0x80 || (!uri_unreserved(byte) && !uri_reserved(byte))) {
      set_error(error, error_size,
                "link URI contains a byte that must be percent encoded");
      return false;
    }
  }
  return true;
}

static bool command_uri_encode(StyledLinkKind kind, const char *command,
                               char *uri, size_t uri_size, char *error,
                               size_t error_size) {
  const char *scheme = kind == STYLED_LINK_SEND ? "send:" : "prompt:";
  size_t used = strlen(scheme);

  memcpy(uri, scheme, used);
  for (const unsigned char *cursor = (const unsigned char *)command; *cursor;
       cursor++) {
    if (uri_unreserved(*cursor)) {
      if (used + 1 >= uri_size)
        goto too_long;
      uri[used++] = (char)*cursor;
    } else {
      if (used + 3 >= uri_size)
        goto too_long;
      snprintf(uri + used, uri_size - used, "%%%02X", *cursor);
      used += 3;
    }
  }
  uri[used] = '\0';
  return true;

too_long:
  set_error(error, error_size, "encoded link URI is too long");
  return false;
}

static bool link_enabled(StyledLinkKind kind,
                         const StyledTextRenderOptions *options) {
  if (options == nullptr)
    return false;
  switch (kind) {
  case STYLED_LINK_EXTERNAL:
    return options->osc_hyperlinks;
  case STYLED_LINK_SEND:
    return options->osc_hyperlinks_send;
  case STYLED_LINK_PROMPT:
    return options->osc_hyperlinks_prompt;
  }
  return false;
}

static bool emit_link_open(const char *uri, char *output, size_t output_size,
                           size_t *used) {
  constexpr char prefix[] = "\033]8;;";
  constexpr char suffix[] = "\033\\";
  size_t length = sizeof(prefix) - 1 + strlen(uri) + sizeof(suffix) - 1;

  if (*used + length + OSC8_CLOSE_SIZE >= output_size)
    return false;
  return append_string(output, output_size, used, prefix) &&
         append_string(output, output_size, used, uri) &&
         append_string(output, output_size, used, suffix);
}

static bool emit_link_close(char *output, size_t output_size, size_t *used) {
  return append_string(output, output_size, used, "\033]8;;\033\\");
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

static bool parse_link_tag(const char *start, const char *end,
                           StyledLinkKind *kind, const char **target) {
  static const struct {
    const char *name;
    StyledLinkKind kind;
  } tags[] = {
      {"link=", STYLED_LINK_EXTERNAL},
      {"send=", STYLED_LINK_SEND},
      {"prompt=", STYLED_LINK_PROMPT},
  };

  for (size_t index = 0; index < sizeof(tags) / sizeof(tags[0]); index++) {
    size_t length = strlen(tags[index].name);

    if ((size_t)(end - start) > length &&
        !strncasecmp(start, tags[index].name, length)) {
      *kind = tags[index].kind;
      *target = start + length;
      return true;
    }
  }
  return false;
}

static bool apply_tag(const StyledTextPalette *palette, const char *tag,
                      StyledState *state, StyledState *stack,
                      size_t *stack_size, char *output, size_t output_size,
                      size_t *used, const StyledTextRenderOptions *options,
                      char *error, size_t error_size) {
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
    StyledState restored;

    if (*stack_size == 0) {
      set_error(error, error_size, "style close tag has no matching open tag");
      return false;
    }
    restored = stack[--*stack_size];
    bool closed_link = state->has_link && !restored.has_link;
    if (state->link_emitted && !restored.link_emitted) {
      if (!emit_link_close(output, output_size, used))
        return false;
    }
    if (closed_link && styled_format_equal(state, &restored)) {
      *state = restored;
      return true;
    }
    *state = restored;
    return emit_state(state, output, styled_output_size(state, output_size),
                      used);
  }
  if (end - start == 5 && !strncasecmp(start, "reset", 5)) {
    if (state->link_emitted && !emit_link_close(output, output_size, used))
      return false;
    *state = (StyledState){0};
    *stack_size = 0;
    return emit_state(state, output, output_size, used);
  }
  if (*stack_size >= STYLE_STACK_LIMIT) {
    set_error(error, error_size, "style nesting is too deep");
    return false;
  }

  StyledLinkKind link_kind;
  const char *target_start;
  if (parse_link_tag(start, end, &link_kind, &target_start)) {
    char target[OSC8_URI_LIMIT + 1];
    char uri[OSC8_URI_LIMIT + 1];

    if (state->has_link) {
      set_error(error, error_size, "links cannot be nested");
      return false;
    }
    if (!link_target_unquote(target_start, end, target, sizeof(target), error,
                             error_size))
      return false;
    if (link_kind == STYLED_LINK_EXTERNAL) {
      if (!external_uri_valid(target, error, error_size))
        return false;
      memcpy(uri, target, strlen(target) + 1);
    } else if (!command_uri_encode(link_kind, target, uri, sizeof(uri), error,
                                   error_size)) {
      return false;
    }
    stack[(*stack_size)++] = *state;
    state->has_link = true;
    state->link_emitted = link_enabled(link_kind, options);
    if (state->link_emitted &&
        !emit_link_open(uri, output, output_size, used)) {
      (*stack_size)--;
      *state = stack[*stack_size];
      set_error(error, error_size, "styled text is too long");
      return false;
    }
    return true;
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
    if (directive_length >= sizeof(directive)) {
      set_error(error, error_size, "style directive is too long");
      return false;
    }
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
  return emit_state(state, output, styled_output_size(state, output_size),
                    used);
}

bool styled_text_compile(const StyledTextPalette *palette, const char *markup,
                         char *output, size_t output_size, char *error,
                         size_t error_size) {
  static const StyledTextRenderOptions options = {
      .color_depth = TERMINAL_COLOR_TRUECOLOR,
      .osc_hyperlinks = true,
      .osc_hyperlinks_send = true,
      .osc_hyperlinks_prompt = true,
  };
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
      if (!append_bytes(output, styled_output_size(&state, output_size), &used,
                        cursor, decoded.length))
        goto too_long;
      cursor += decoded.length;
      continue;
    }
    if (cursor[1] == '[') {
      if (!append_bytes(output, styled_output_size(&state, output_size), &used,
                        "[", 1))
        goto too_long;
      cursor += 2;
      continue;
    }

    const char *close = find_tag_close(cursor + 1);
    char tag[OSC8_URI_LIMIT + 32];
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
                   output_size, &used, &options, error, error_size)) {
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
      if (!append_utf8_codepoint(output, available, &used, cursor, &consumed))
        break;
      cursor += consumed;
      continue;
    }
    int parameters[SGR_PARAMETER_LIMIT];
    size_t parameter_count;
    const char *end;
    if (parse_sgr(cursor, &end, parameters, &parameter_count)) {
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

        if (append_bytes(output, available, &used, cursor, length)) {
          link_open = !is_close;
          cursor = end;
          continue;
        }
        break;
      }
      cursor = skip_escape(cursor);
    }
  }
  if (link_open)
    emit_link_close(output, output_size, &used);
  if (saw_sgr && depth != TERMINAL_COLOR_NONE)
    append_string(output, output_size, &used, "\033[0m");
}

static void compile_markup_permissive(const StyledTextPalette *palette,
                                      const char *input, char *output,
                                      size_t output_size,
                                      const StyledTextRenderOptions *options) {
  StyledState state = {0};
  StyledState stack[STYLE_STACK_LIMIT];
  size_t stack_size = 0;
  size_t used = 0;

  output[0] = '\0';
  for (const char *cursor = input; *cursor;) {
    if (*cursor == '\033') {
      const char *end;
      int parameters[SGR_PARAMETER_LIMIT];
      size_t parameter_count;

      if (parse_sgr(cursor, &end, parameters, &parameter_count)) {
        if (!append_bytes(output, styled_output_size(&state, output_size),
                          &used, cursor, (size_t)(end - cursor)))
          return;
        cursor = end;
      } else {
        cursor = skip_escape(cursor);
      }
      continue;
    }
    if (*cursor != '[') {
      size_t consumed;
      if (!append_utf8_codepoint(output,
                                 styled_output_size(&state, output_size), &used,
                                 cursor, &consumed))
        return;
      cursor += consumed;
      continue;
    }
    if (cursor[1] == '[') {
      if (!append_bytes(output, styled_output_size(&state, output_size), &used,
                        "[", 1))
        return;
      cursor += 2;
      continue;
    }

    const char *close = find_tag_close(cursor + 1);
    size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
    if (close && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
      StyledState candidate_state = state;
      StyledState candidate_stack[STYLE_STACK_LIMIT];
      size_t candidate_stack_size = stack_size;
      char rendered[LBUF_SIZE] = "";
      size_t rendered_size = 0;
      char error[128] = "";
      char tag[OSC8_URI_LIMIT + 32];

      memcpy(tag, cursor + 1, tag_length);
      tag[tag_length] = '\0';
      memcpy(candidate_stack, stack, sizeof(stack));
      if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                    &candidate_stack_size, rendered, sizeof(rendered),
                    &rendered_size, options, error, sizeof(error))) {
        if (!append_string(output,
                           styled_output_size(&candidate_state, output_size),
                           &used, rendered))
          return;
        state = candidate_state;
        memcpy(stack, candidate_stack, sizeof(stack));
        stack_size = candidate_stack_size;
        cursor = close + 1;
        continue;
      }
    }
    if (!append_bytes(output, styled_output_size(&state, output_size), &used,
                      cursor, 1))
      return;
    cursor++;
  }
  if (state.link_emitted)
    emit_link_close(output, output_size, &used);
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
  compile_markup_permissive(palette, styled, compiled, sizeof(compiled),
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

void styled_text_truncate(const StyledTextPalette *palette, const char *styled,
                          size_t width, char *output, size_t output_size) {
  static const StyledTextRenderOptions options = {0};
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
      const char *close = find_tag_close(cursor + 1);
      size_t tag_length = close ? (size_t)(close - cursor - 1) : 0;
      bool applied = false;

      if (close && tag_length > 0 && tag_length < OSC8_URI_LIMIT + 32) {
        StyledState candidate_state = state;
        StyledState candidate_stack[STYLE_STACK_LIMIT];
        size_t candidate_stack_size = stack_size;
        char rendered[LBUF_SIZE] = "";
        size_t rendered_size = 0;
        char error[128] = "";
        char tag[OSC8_URI_LIMIT + 32];

        memcpy(tag, cursor + 1, tag_length);
        tag[tag_length] = '\0';
        memcpy(candidate_stack, stack, sizeof(stack));
        if (apply_tag(palette, tag, &candidate_state, candidate_stack,
                      &candidate_stack_size, rendered, sizeof(rendered),
                      &rendered_size, &options, error, sizeof(error)) &&
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
