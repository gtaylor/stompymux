#include <string.h>

#include "btech/configuration.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"
#include "map_conditions_api.h"
#include "mech_classification_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_map_render_internal.h"
#include "mech_maps_api.h"
#include "mech_notify_api.h"
#include "mech_status_types.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

#include <stdio.h>

static char *tactical_command_argument(char *const *arguments, size_t index) {
  return *(char *const *)checked_storage_at_const((const void *)arguments, 4,
                                                  sizeof(*arguments), index);
}

static bool ascii_is_alpha(char value) {
  return ((value >= 'A' && value <= 'Z') || (value >= 'a' && value <= 'z')) !=
         0;
}

void mech_tacmap(DbRef player, Mech *mech, char *buffer) {
  int argc;
  int x;
  int y;
  char *args_vec[4];
  BattleMap *mech_map;
  int display_height = MAP_DISPLAY_HEIGHT;
  int display_width = MAP_DISPLAY_WIDTH;
  MapText *map_text;
  int flags = 3;
  int dohexlos = 0;

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

  const TacticalArgumentParseResult PARSED =
      tactical_arguments_parse(&(TacticalArgumentParseRequest){
          .player = player,
          .mech = mech,
          .arguments = args_vec,
          .argument_capacity = 4,
          .first_argument = first_argument,
          .argument_count = argc,
          .maximum_range = mech_tactical_range(mech),
      });
  if (!PARSED.valid)
    return;
  x = PARSED.position.x;
  y = PARSED.position.y;

  BtechPlayerUiPreferences preferences =
      btech_player_ui_preferences(mech_context(mech), player);
  display_height = preferences.tactical_height;
  display_width = preferences.tactical_width;

  /* Everything worked but lets check the mech's tac range
   * and the map size */
  display_height = (display_height <= 2 * mech_tactical_range(mech)
                        ? display_height
                        : 2 * mech_tactical_range(mech));
  display_width = (display_width <= 2 * mech_tactical_range(mech)
                       ? display_width
                       : 2 * mech_tactical_range(mech));

  display_height = (display_height <= mech_map->map_height)
                       ? display_height
                       : mech_map->map_height;
  display_width = (display_width <= mech_map->map_width) ? display_width
                                                         : mech_map->map_width;

  /* Get the data to draw the map */
  MapTextRequest request = {
      .player = player,
      .mech = mech,
      .map = mech_map,
      .center_x = x,
      .center_y = y,
      .width = display_width,
      .height = display_height,
      .labels = flags,
      .calculate_los = dohexlos != 0,
  };
  map_text = map_text_create(&request);
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
