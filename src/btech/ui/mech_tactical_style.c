#include "mech_map_render_internal.h"

#include "mux/support/checked_storage.h"

#include <string.h>

typedef struct MapTextBuilder {
  MapText *text;
  size_t length;
} MapTextBuilder;

static bool map_text_builder_append(MapTextBuilder *builder, const void *source,
                                    size_t length) {
  if (builder->length > builder->text->buffer_capacity ||
      length > builder->text->buffer_capacity - builder->length)
    return false;
  memcpy(checked_storage_region(builder->text->buffer,
                                builder->text->buffer_capacity, builder->length,
                                length),
         source, length);
  builder->length += length;
  return true;
}

static bool map_text_builder_append_char(MapTextBuilder *builder, char value) {
  return map_text_builder_append(builder, &value, sizeof(value));
}

static bool map_text_set_line(MapText *text, size_t line, size_t offset) {
  if (line >= text->line_capacity || offset >= text->buffer_capacity)
    return false;
  char **line_slot = (char **)checked_storage_at(
      (void *)text->lines, text->line_capacity, sizeof(*text->lines), line);
  *line_slot = checked_storage_at(text->buffer, text->buffer_capacity,
                                  sizeof(*text->buffer), offset);
  return true;
}

static bool map_text_end_lines(MapText *text, size_t line) {
  if (line >= text->line_capacity)
    return false;
  char **line_slot = (char **)checked_storage_at(
      (void *)text->lines, text->line_capacity, sizeof(*text->lines), line);
  *line_slot = nullptr;
  return true;
}

static bool ascii_is_lower(unsigned char value) {
  return value >= 'a' && value <= 'z';
}

static bool ascii_is_upper(unsigned char value) {
  return value >= 'A' && value <= 'Z';
}

static bool ascii_is_digit(unsigned char value) {
  return value >= '0' && value <= '9';
}

bool style_tac_map(MapText *text, const MapColorScheme *colors,
                   const char *sketch, int dispcols, int disprows) {
  if (dispcols <= 0 || disprows < 0)
    return false;
  const size_t SKETCH_CAPACITY = (size_t)dispcols * (size_t)disprows;
  MapTextBuilder builder = {.text = text};
  int line = 0;
  int column = 0;
  char cur_colour = '\0';

  if (!map_text_set_line(text, 0, 0))
    return false;
  while (line < disprows) {
    char new_colour;
    const size_t SOURCE_OFFSET =
        (size_t)line * (size_t)dispcols + (size_t)column;
    const unsigned char INPUT =
        (unsigned char)*(const char *)checked_storage_at_const(
            sketch, SKETCH_CAPACITY, sizeof(char), SOURCE_OFFSET);
    column++;
    char c = (char)INPUT;

    if (INPUT == '\0') {
      /*
       * End of line.
       */
      if (cur_colour != '\0') {
        if (!map_text_builder_append(&builder, "[reset]", 7))
          return false;
      }
      if (!map_text_builder_append_char(&builder, '\0'))
        return false;
      line++;
      if (line >= disprows) {
        break; /* Done */
      }
      column = 0;
      if (!map_text_set_line(text, (size_t)line, builder.length))
        return false;
      continue;
    }

    switch (INPUT) {
    case (unsigned char)'\242': /* Colour Hack: Deep Water */
      c = '~';
      new_colour = colors->values[DWATER_IDX];
      break;

    case (unsigned char)'\241': /* Colour Hack: improper LZ */
      c = 'X';
      new_colour = colors->values[BADLZ_IDX];
      break;
    case (unsigned char)'\240': /* Colour Hack: proper LZ */
      c = 'O';
      new_colour = colors->values[GOODLZ_IDX];
      break;
    case '?':
      c = '?';
      new_colour = colors->values[UNKNOWN_IDX];
      break;

    case '$': /* Colour Hack: Drop Ship */
      c = 'X';
      new_colour = colors->values[DS_IDX];
      break;

    case '!': /* Cliff hex edge */
      c = '/';
      new_colour = colors->values[CLIFF_IDX];
      break;

    case '|': /* Cliff hex edge */
      c = '\\';
      new_colour = colors->values[CLIFF_IDX];
      break;

    case ',': /* Cliff hex edge */
      c = '_';
      new_colour = colors->values[CLIFF_IDX];
      break;
    case '*': /* mech itself. */
      new_colour = colors->values[SELF_IDX];
      break;

    default:
      if (ascii_is_lower(INPUT)) { /* Friendly con */
        new_colour = colors->values[FRIEND_IDX];
      } else if (ascii_is_upper(INPUT)) { /* Enemy con */
        new_colour = colors->values[ENEMY_IDX];
      } else if (ascii_is_digit(INPUT)) { /* Elevation */
        new_colour = cur_colour;
      } else {
        new_colour = map_terrain_color_char(&(TerrainColorRequest){
            .colors = colors, .terrain = c, .elevation = 0});
      }
      break;
    }

    if (new_colour != cur_colour) {
      const char *markup = map_color_markup(new_colour);
      if (!map_text_builder_append(&builder, "[reset]", 7) ||
          !map_text_builder_append(&builder, markup, strlen(markup)))
        return false;
      cur_colour = new_colour;
    }
    if (!map_text_builder_append_char(&builder, c))
      return false;
  }
  return map_text_end_lines(text, (size_t)line);
}

/*
 * Draw a tac map for the TACTICAL and NAVIGATE commands.
 *
 * This used to be "one MOFO of a function" but has been simplified
 * in a number of ways.  One is that it used to statically allocated
 * buffers which limit the map drawn to MAP_DISPLAY_WIDTH hexes across
 * and 24 hexes down in size.  The return value should no longer be
 * freed with KillText().
 *
 * player   = dbref of player wanting map (mostly irrelevant)
 * mech     = mech player's in (or NULL, if on map)
 * map      = map obj itself
 * cx       = middle of the map (x)
 * cy       = middle of the map (y)
 * wx       = width in x
 * wy       = width in y
 * labels   = bit array
 *    1 = the 'top numbers'
 *    2 = the 'side numbers'
 *    4 = navigate mode
 *    8 = show mech cliffs
 *   16 = show tank cliffs
 *   32 = show DS LZ's
 *   64 = show underlying terrain
 *  128 = show minefields and strength
 *
 * If navigate mode, wx and wy should be equal and odd.  Navigate maps
 * cannot have top or side labels.
 *
 */
