#include "mech_maps_internal.h"

void mech_tacmap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int argc, i;
  short x, y;
  char *args_vec[4];
  char **args = args_vec;
  BattleMap *mech_map;
  int displayHeight = MAP_DISPLAY_HEIGHT, displayWidth = MAP_DISPLAY_WIDTH;
  char *str;
  char *const *maptext;
  MapText *map_text;
  int flags = 3, dohexlos = 0;

  /* Basic checks for pilot and mech */
  cch(MECH_USUAL);
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);

  /* Get the map info */
  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  /* Various checks for conditions and system of mech */
  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(mech->xcode.context, !MechTacRange(mech),
                  "Your system seems to be inoperational.");

  if (MapIsDark(mech_map) ||
      (MechType(mech) == CLASS_MW &&
       mech->xcode.context->configuration->btech_mw_losmap))
    dohexlos = 1;

  /* Check to see which type of tactical to display
   * if they specified a particular one */
  if (argc > 0 && isalpha((unsigned char)args[0][0]) && args[0][1] == '\0') {

    switch (tolower((unsigned char)args[0][0])) {
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
      notify(evaluation, player, "Invalid tactical map flag.");
      return;
    }

    args++;
    argc--;
  }

  DOCHECK_CONTEXT(mech->xcode.context, dohexlos && (flags & (8 | 16 | 32)),
                  "You can't see that much here!");

  if (!parse_tacargs(player, mech, args, argc, MechTacRange(mech), &x, &y))
    return;

  /* Get the Tacsize attribute from
   * the player, if doesn't exist set the height and width to
   * default params. If it does exist, check the values and
   * make sure they are legit. */
  str = btech_attribute_read(mech->xcode.context->database, player, A_TACSIZE,
                             (char[LBUF_SIZE]){0});
  if (!*str) {
    displayHeight = MAP_DISPLAY_HEIGHT;
    displayWidth = MAP_DISPLAY_WIDTH;
  } else if (sscanf(str, "%d %d", &displayHeight, &displayWidth) != 2 ||
             displayHeight > 24 || displayHeight < 5 || displayWidth > 40 ||
             displayWidth < 5) {

    notify(evaluation, player,
           "Illegal Tacsize attribute. Must be in format "
           "'Height Width' . Height : 5-24 Width : 5-40");
    displayHeight = MAP_DISPLAY_HEIGHT;
    displayWidth = MAP_DISPLAY_WIDTH;
  }

  /* Everything worked but lets check the mech's tac range
   * and the map size */
  displayHeight =
      (displayHeight <= 2 * MechTacRange(mech) ? displayHeight
                                               : 2 * MechTacRange(mech));
  displayWidth =
      (displayWidth <= 2 * MechTacRange(mech) ? displayWidth
                                              : 2 * MechTacRange(mech));

  displayHeight = (displayHeight <= mech_map->map_height)
                      ? displayHeight
                      : mech_map->map_height;
  displayWidth = (displayWidth <= mech_map->map_width) ? displayWidth
                                                       : mech_map->map_width;

  /* Get the data to draw the map */
  map_text = map_text_create(player, mech, mech_map, x, y, displayWidth,
                             displayHeight, flags, dohexlos);
  if (map_text == nullptr) {
    notify(evaluation, player, "Unable to render the tactical map.");
    return;
  }
  maptext = map_text_lines(map_text);

  /* Draw the map for the player */
  for (i = 0; maptext[i]; i++)
    notify(evaluation, player, maptext[i]);
  map_text_destroy(map_text);
}

/* XXX Fix 'enterbase <dir>' */
