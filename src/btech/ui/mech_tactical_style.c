#include "mech_map_render_internal.h"

#include <assert.h>
#include <ctype.h>
#include <string.h>

bool style_tac_map(MapText *text, const MapColorScheme *colors,
                   const char *sketch, int dispcols, int disprows) {
  size_t pos = 0;
  int line = 0;
  char cur_colour = '\0';
  const char *line_start;
  const char *src = sketch;

  line_start = src;
  text->lines[0] = text->buffer;
  while (line < disprows) {
    char new_colour;
    const unsigned char input = (unsigned char)*src++;
    char c = (char)input;

    if (input == '\0') {
      /*
       * End of line.
       */
      if (cur_colour != '\0') {
        memcpy(text->buffer + pos, "[reset]", 7);
        pos += 7;
      }
      text->buffer[pos++] = '\0';
      line++;
      if (line >= disprows) {
        break; /* Done */
      }
      line_start += dispcols;
      src = line_start;
      text->lines[line] = text->buffer + pos;
      continue;
    }

    switch (input) {
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
      if (islower(input)) { /* Friendly con */
        new_colour = colors->values[FRIEND_IDX];
      } else if (isupper(input)) { /* Enemy con */
        new_colour = colors->values[ENEMY_IDX];
      } else if (isdigit(input)) { /* Elevation */
        new_colour = cur_colour;
      } else {
        new_colour = map_terrain_color_char(colors, c, 0);
      }
      break;
    }

    if (new_colour != cur_colour) {
      const char *markup = map_color_markup(new_colour);
      memcpy(text->buffer + pos, "[reset]", 7);
      pos += 7;
      memcpy(text->buffer + pos, markup, strlen(markup));
      pos += strlen(markup);
      cur_colour = new_colour;
    }
    text->buffer[pos++] = c;
    assert(pos + 32 <= text->buffer_capacity);
  }
  assert((size_t)line < text->line_capacity);
  text->lines[line] = nullptr;
  return true;
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
