#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_notify_api.h"
#include "mech_status_types.h"
#include "mux/objects/attrs.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

#include <stdio.h>

static char *tactical_command_argument(char *const *arguments, size_t index) {
  return *(char *const *)checked_storage_at_const(arguments, 4,
                                                  sizeof(*arguments), index);
}

static bool ascii_is_alpha(char value) {
  return (value >= 'A' && value <= 'Z') || (value >= 'a' && value <= 'z');
}

void mech_tacmap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int argc;
  short x, y;
  char *args_vec[4];
  BattleMap *mech_map;
  int displayHeight = MAP_DISPLAY_HEIGHT, displayWidth = MAP_DISPLAY_WIDTH;
  char *str;
  MapText *map_text;
  int flags = 3, dohexlos = 0;

  /* Basic checks for pilot and mech */
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));

  /* Get the map info */
  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  /* Various checks for conditions and system of mech */
  argc = mech_parseattributes(buffer, args_vec, 4);
  if (!mech_tactical_range(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your system seems to be inoperational.");
    return;
  }

  if (battle_map_is_dark(mech_map) ||
      (mech_class(mech) == CLASS_MW &&
       mech_context(mech)->configuration->btech_mw_losmap))
    dohexlos = 1;

  /* Check to see which type of tactical to display
   * if they specified a particular one */
  size_t first_argument = 0;
  char *first = argc > 0 ? tactical_command_argument(args_vec, 0) : nullptr;
  if (first != nullptr && ascii_is_alpha(*first) &&
      *checked_string_suffix(first, 1) == '\0') {

    switch (ascii_to_lower(*first)) {
    case 'c':
      flags |= 8; /* Show cliffs */
      break;

    case 't':
      flags |= 16; /* Show tank cliffs */
      break;

    case 'l':
      dohexlos = 1;
      break;

    case 'b':
      flags |= 32;
      break;

    case 'u':
      flags |= 64; /* Show underlying terrain */
      break;

    case 'm':
      flags |= 128; /* Show mines */
      break;

    default:
      mecha_notify(evaluation, player, "Invalid tactical map flag.");
      return;
    }

    first_argument = 1;
    argc--;
  }

  if (dohexlos && (flags & (8 | 16 | 32))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't see that much here!");
    return;
  }

  if (!parse_tacargs(player, mech, args_vec, 4, first_argument, argc,
                     mech_tactical_range(mech), &x, &y))
    return;

  /* Get the Tacsize attribute from
   * the player, if doesn't exist set the height and width to
   * default params. If it does exist, check the values and
   * make sure they are legit. */
  str = btech_attribute_read(mech_context(mech)->database, player, A_TACSIZE,
                             (char[LBUF_SIZE]){0});
  if (!*str) {
    displayHeight = MAP_DISPLAY_HEIGHT;
    displayWidth = MAP_DISPLAY_WIDTH;
  } else if (!parse_int_checked(strtok(str, " \t"), &displayHeight) ||
             !parse_int_checked(strtok(nullptr, " \t"), &displayWidth) ||
             strtok(nullptr, " \t") != nullptr || displayHeight > 24 ||
             displayHeight < 5 || displayWidth > 40 || displayWidth < 5) {

    mecha_notify(evaluation, player,
                 "Illegal Tacsize attribute. Must be in format "
                 "'Height Width' . Height : 5-24 Width : 5-40");
    displayHeight = MAP_DISPLAY_HEIGHT;
    displayWidth = MAP_DISPLAY_WIDTH;
  }

  /* Everything worked but lets check the mech's tac range
   * and the map size */
  displayHeight = (displayHeight <= 2 * mech_tactical_range(mech)
                       ? displayHeight
                       : 2 * mech_tactical_range(mech));
  displayWidth = (displayWidth <= 2 * mech_tactical_range(mech)
                      ? displayWidth
                      : 2 * mech_tactical_range(mech));

  displayHeight = (displayHeight <= mech_map->map_height)
                      ? displayHeight
                      : mech_map->map_height;
  displayWidth = (displayWidth <= mech_map->map_width) ? displayWidth
                                                       : mech_map->map_width;

  /* Get the data to draw the map */
  map_text = map_text_create(player, mech, mech_map, x, y, displayWidth,
                             displayHeight, flags, dohexlos);
  if (map_text == nullptr) {
    mecha_notify(evaluation, player, "Unable to render the tactical map.");
    return;
  }
  /* Draw the map for the player */
  for (size_t line = 0; line < map_text_line_count(map_text); line++)
    mecha_notify(evaluation, player, map_text_line(map_text, line));
  map_text_destroy(map_text);
}

/* XXX Fix 'enterbase <dir>' */
