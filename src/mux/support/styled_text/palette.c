/* palette.c - Styled-text palette and color parsing. */

#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/support/checked_storage.h"
#include "mux/support/styled_text/internal.h"
#include "mux/support/styled_text/palette.h"

typedef struct NamedColor {
  const char *name;
  int red;
  int green;
  int blue;
} NamedColor;

static const NamedColor built_in_colors[] = {
    {"aliceblue", 240, 248, 255},
    {"antiquewhite", 250, 235, 215},
    {"aqua", 0, 255, 255},
    {"aquamarine", 127, 255, 212},
    {"azure", 240, 255, 255},
    {"beige", 245, 245, 220},
    {"bisque", 255, 228, 196},
    {"black", 0, 0, 0},
    {"blanchedalmond", 255, 235, 205},
    {"blue", 0, 0, 255},
    {"blueviolet", 138, 43, 226},
    {"brown", 165, 42, 42},
    {"burlywood", 222, 184, 135},
    {"cadetblue", 95, 158, 160},
    {"chartreuse", 127, 255, 0},
    {"chocolate", 210, 105, 30},
    {"coral", 255, 127, 80},
    {"cornflowerblue", 100, 149, 237},
    {"cornsilk", 255, 248, 220},
    {"crimson", 220, 20, 60},
    {"cyan", 0, 255, 255},
    {"darkblue", 0, 0, 139},
    {"darkcyan", 0, 139, 139},
    {"darkgoldenrod", 184, 134, 11},
    {"darkgray", 169, 169, 169},
    {"darkgreen", 0, 100, 0},
    {"darkgrey", 169, 169, 169},
    {"darkkhaki", 189, 183, 107},
    {"darkmagenta", 139, 0, 139},
    {"darkolivegreen", 85, 107, 47},
    {"darkorange", 255, 140, 0},
    {"darkorchid", 153, 50, 204},
    {"darkred", 139, 0, 0},
    {"darksalmon", 233, 150, 122},
    {"darkseagreen", 143, 188, 143},
    {"darkslateblue", 72, 61, 139},
    {"darkslategray", 47, 79, 79},
    {"darkslategrey", 47, 79, 79},
    {"darkturquoise", 0, 206, 209},
    {"darkviolet", 148, 0, 211},
    {"deeppink", 255, 20, 147},
    {"deepskyblue", 0, 191, 255},
    {"dimgray", 105, 105, 105},
    {"dimgrey", 105, 105, 105},
    {"dodgerblue", 30, 144, 255},
    {"firebrick", 178, 34, 34},
    {"floralwhite", 255, 250, 240},
    {"forestgreen", 34, 139, 34},
    {"fuchsia", 255, 0, 255},
    {"gainsboro", 220, 220, 220},
    {"ghostwhite", 248, 248, 255},
    {"gold", 255, 215, 0},
    {"goldenrod", 218, 165, 32},
    {"gray", 128, 128, 128},
    {"green", 0, 128, 0},
    {"greenyellow", 173, 255, 47},
    {"grey", 128, 128, 128},
    {"honeydew", 240, 255, 240},
    {"hotpink", 255, 105, 180},
    {"indianred", 205, 92, 92},
    {"indigo", 75, 0, 130},
    {"ivory", 255, 255, 240},
    {"khaki", 240, 230, 140},
    {"lavender", 230, 230, 250},
    {"lavenderblush", 255, 240, 245},
    {"lawngreen", 124, 252, 0},
    {"lemonchiffon", 255, 250, 205},
    {"lightblue", 173, 216, 230},
    {"lightcoral", 240, 128, 128},
    {"lightcyan", 224, 255, 255},
    {"lightgoldenrodyellow", 250, 250, 210},
    {"lightgray", 211, 211, 211},
    {"lightgreen", 144, 238, 144},
    {"lightgrey", 211, 211, 211},
    {"lightpink", 255, 182, 193},
    {"lightsalmon", 255, 160, 122},
    {"lightseagreen", 32, 178, 170},
    {"lightskyblue", 135, 206, 250},
    {"lightslategray", 119, 136, 153},
    {"lightslategrey", 119, 136, 153},
    {"lightsteelblue", 176, 196, 222},
    {"lightyellow", 255, 255, 224},
    {"lime", 0, 255, 0},
    {"limegreen", 50, 205, 50},
    {"linen", 250, 240, 230},
    {"magenta", 255, 0, 255},
    {"maroon", 128, 0, 0},
    {"mediumaquamarine", 102, 205, 170},
    {"mediumblue", 0, 0, 205},
    {"mediumorchid", 186, 85, 211},
    {"mediumpurple", 147, 112, 219},
    {"mediumseagreen", 60, 179, 113},
    {"mediumslateblue", 123, 104, 238},
    {"mediumspringgreen", 0, 250, 154},
    {"mediumturquoise", 72, 209, 204},
    {"mediumvioletred", 199, 21, 133},
    {"midnightblue", 25, 25, 112},
    {"mintcream", 245, 255, 250},
    {"mistyrose", 255, 228, 225},
    {"moccasin", 255, 228, 181},
    {"navajowhite", 255, 222, 173},
    {"navy", 0, 0, 128},
    {"oldlace", 253, 245, 230},
    {"olive", 128, 128, 0},
    {"olivedrab", 107, 142, 35},
    {"orange", 255, 165, 0},
    {"orangered", 255, 69, 0},
    {"orchid", 218, 112, 214},
    {"palegoldenrod", 238, 232, 170},
    {"palegreen", 152, 251, 152},
    {"paleturquoise", 175, 238, 238},
    {"palevioletred", 219, 112, 147},
    {"papayawhip", 255, 239, 213},
    {"peachpuff", 255, 218, 185},
    {"peru", 205, 133, 63},
    {"pink", 255, 192, 203},
    {"plum", 221, 160, 221},
    {"powderblue", 176, 224, 230},
    {"purple", 128, 0, 128},
    {"rebeccapurple", 102, 51, 153},
    {"red", 255, 0, 0},
    {"rosybrown", 188, 143, 143},
    {"royalblue", 65, 105, 225},
    {"saddlebrown", 139, 69, 19},
    {"salmon", 250, 128, 114},
    {"sandybrown", 244, 164, 96},
    {"seagreen", 46, 139, 87},
    {"seashell", 255, 245, 238},
    {"sienna", 160, 82, 45},
    {"silver", 192, 192, 192},
    {"skyblue", 135, 206, 235},
    {"slateblue", 106, 90, 205},
    {"slategray", 112, 128, 144},
    {"slategrey", 112, 128, 144},
    {"snow", 255, 250, 250},
    {"springgreen", 0, 255, 127},
    {"steelblue", 70, 130, 180},
    {"tan", 210, 180, 140},
    {"teal", 0, 128, 128},
    {"thistle", 216, 191, 216},
    {"tomato", 255, 99, 71},
    {"turquoise", 64, 224, 208},
    {"violet", 238, 130, 238},
    {"wheat", 245, 222, 179},
    {"white", 255, 255, 255},
    {"whitesmoke", 245, 245, 245},
    {"yellow", 255, 255, 0},
    {"yellowgreen", 154, 205, 50},
    {nullptr, 0, 0, 0},
};

static const NamedColor *built_in_color(size_t index) {
  return checked_storage_at_const(
      built_in_colors, sizeof(built_in_colors) / sizeof(built_in_colors[0]),
      sizeof(*built_in_colors), index);
}

static char palette_character(const char *text, size_t length, size_t index) {
  return *(const char *)checked_storage_at_const(text, length + 1, sizeof(char),
                                                 index);
}

StyledTextPalette *styled_text_palette_create(void) {
  return calloc(1, sizeof(StyledTextPalette));
}

void styled_text_palette_destroy(StyledTextPalette *palette) {
  if (!palette)
    return;
  for (size_t index = 0; index < palette->count; index++)
    free(styled_palette_color(palette, index)->name);
  for (size_t index = 0; index < palette->preset_count; index++) {
    StyledTextPreset *preset = styled_palette_preset(palette, index);
    free(preset->name);
    styled_link_config_destroy(&preset->config);
  }
  free(palette->colors);
  free(palette->presets);
  free(palette);
}

static bool styled_text_color_name_valid(const char *name) {
  size_t length;

  if (!name || !*name)
    return false;
  length = strlen(name);
  if (length > 60)
    return false;
  for (size_t index = 0; index < length; index++) {
    unsigned char ch = (unsigned char)palette_character(name, length, index);
    if (!(isalnum)(ch) && ch != '-' && ch != '_')
      return false;
  }
  return true;
}

static const NamedColor *styled_text_builtin_color(const char *name) {
  const size_t count = sizeof(built_in_colors) / sizeof(built_in_colors[0]);
  for (size_t index = 0; index + 1 < count; index++) {
    const NamedColor *color = built_in_color(index);
    if (!strcasecmp(name, color->name))
      return color;
  }
  return nullptr;
}

bool styled_text_palette_set_rgb(StyledTextPalette *palette, const char *name,
                                 int red, int green, int blue, char *error,
                                 size_t error_size) {
  CustomNamedColor *entry;

  if (error && error_size > 0)
    error[0] = '\0';
  if (!palette) {
    styled_set_error(error, error_size, "color palette is not available");
    return false;
  }
  if (!styled_text_color_name_valid(name)) {
    styled_set_error(error, error_size,
                     "color name must use 1-60 letters, digits, '-' or '_'");
    return false;
  }
  if (styled_text_builtin_color(name)) {
    styled_set_error(
        error, error_size,
        "custom color name conflicts with a built-in CSS/X11 color");
    return false;
  }
  if (red < 0 || red > 255 || green < 0 || green > 255 || blue < 0 ||
      blue > 255) {
    styled_set_error(error, error_size,
                     "RGB channels must be between 0 and 255");
    return false;
  }
  for (size_t index = 0; index < palette->count; index++) {
    CustomNamedColor *color = styled_palette_color(palette, index);
    if (!strcasecmp(name, color->name)) {
      color->red = red;
      color->green = green;
      color->blue = blue;
      return true;
    }
  }
  if (palette->count == palette->capacity) {
    size_t capacity = palette->capacity ? palette->capacity * 2 : 16;
    CustomNamedColor *colors =
        realloc(palette->colors, capacity * sizeof(*colors));
    if (!colors) {
      styled_set_error(error, error_size, "unable to allocate named color");
      return false;
    }
    palette->colors = colors;
    palette->capacity = capacity;
  }
  entry = styled_palette_color(palette, palette->count);
  entry->name = strdup(name);
  if (!entry->name) {
    styled_set_error(error, error_size, "unable to allocate named color");
    return false;
  }
  entry->red = red;
  entry->green = green;
  entry->blue = blue;
  palette->count++;
  return true;
}

static bool parse_hex_byte(const char *value, int *result) {
  const unsigned char first = (unsigned char)palette_character(value, 2, 0);
  const unsigned char second = (unsigned char)palette_character(value, 2, 1);

  if (!(isxdigit)(first) || !(isxdigit)(second))
    return false;
  int high = (isdigit)(first) ? first - '0' : (tolower)(first) - 'a' + 10;
  int low = (isdigit)(second) ? second - '0' : (tolower)(second) - 'a' + 10;
  *result = high * 16 + low;
  return true;
}

static bool parse_rgb_channel(const char *text, size_t length, size_t *offset,
                              int *result, char terminator) {
  int value = 0;
  int digits = 0;

  while (*offset < length &&
         (isdigit)((unsigned char)palette_character(text, length, *offset))) {
    if (value > 255)
      return false;
    value = value * 10 + (palette_character(text, length, *offset) - '0');
    (*offset)++;
    digits++;
  }
  if (digits == 0 || value > 255 || *offset >= length ||
      palette_character(text, length, *offset) != terminator)
    return false;
  (*offset)++;
  *result = value;
  return true;
}

static bool parse_rgb_function(const char *value, StyledColor *color) {
  const size_t length = strlen(value);
  size_t offset = 4;

  if (strncasecmp(value, "rgb(", 4))
    return false;
  if (!parse_rgb_channel(value, length, &offset, &color->red, ',') ||
      !parse_rgb_channel(value, length, &offset, &color->green, ',') ||
      !parse_rgb_channel(value, length, &offset, &color->blue, ')') ||
      offset != length)
    return false;
  color->kind = STYLED_COLOR_RGB;
  return true;
}

bool styled_color_parse(const StyledTextPalette *palette, const char *value,
                        StyledColor *color) {
  const size_t length = strlen(value);
  if (length == 7 && palette_character(value, length, 0) == '#') {
    if (!parse_hex_byte(checked_string_suffix(value, 1), &color->red) ||
        !parse_hex_byte(checked_string_suffix(value, 3), &color->green) ||
        !parse_hex_byte(checked_string_suffix(value, 5), &color->blue))
      return false;
    color->kind = STYLED_COLOR_RGB;
    return true;
  }

  if (parse_rgb_function(value, color))
    return true;

  const NamedColor *built_in = styled_text_builtin_color(value);
  if (built_in) {
    *color = (StyledColor){
        .kind = STYLED_COLOR_RGB,
        .red = built_in->red,
        .green = built_in->green,
        .blue = built_in->blue,
    };
    return true;
  }

  if (palette) {
    for (size_t index = 0; index < palette->count; index++) {
      const CustomNamedColor *named =
          styled_palette_color_const(palette, index);
      if (strcasecmp(value, named->name) != 0)
        continue;
      *color = (StyledColor){
          .kind = STYLED_COLOR_RGB,
          .red = named->red,
          .green = named->green,
          .blue = named->blue,
      };
      return true;
    }
  }

  return false;
}
