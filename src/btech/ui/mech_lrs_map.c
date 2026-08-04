#include "mech_maps_internal.h"

char GetLRSMechChar(Mech *mech, Mech *other) {
  char c = 'u';

  if (mech == other)
    return '*';
  if (IsDS(other))
    c = 'd';
  switch (MechMove(other)) {
  case MOVE_FLY:
    c = 'a';
    break;
  case MOVE_BIPED:
    c = 'b';
    break;
  case MOVE_QUAD:
    c = 'q';
    break;
  case MOVE_TRACK:
    c = 't';
    break;
  case MOVE_WHEEL:
    c = 'w';
    break;
  case MOVE_HOVER:
    c = 'h';
    break;
  case MOVE_VTOL:
    c = 'v';
    break;
  case MOVE_HULL:
    c = 'n';
    break;
  case MOVE_SUB:
    c = 's';
    break;
  case MOVE_FOIL:
    c = 'f';
    break;
  }
  if (!MechSeemsFriend(mech, other))
    c = toupper(c);
  return c;
}

char map_terrain_color_char(const MapColorScheme *colors, char terrain,
                            int elev) {
  switch (terrain) {
  case HIGHWATER:
    return colors->values[DWATER_IDX];
  case WATER:
    if (elev < 2 || elev == '0' || elev == '1' || elev == '~')
      return colors->values[SWATER_IDX];
    return colors->values[DWATER_IDX];
  case BUILDING:
    return colors->values[BUILDING_IDX];
  case ROAD:
    return colors->values[ROAD_IDX];
  case DESERT:
  case ROUGH:
    return colors->values[ROUGH_IDX];
  case MOUNTAINS:
    return colors->values[MOUNTAIN_IDX];
  case FIRE:
    return colors->values[FIRE_IDX];
  case ICE:
    return colors->values[ICE_IDX];
  case WALL:
    return colors->values[WALL_IDX];
  case SNOW:
    return colors->values[SNOW_IDX];
  case SMOKE:
    return colors->values[SMOKE_IDX];
  case LIGHT_FOREST:
    return colors->values[LWOOD_IDX];
  case HEAVY_FOREST:
    return colors->values[HWOOD_IDX];
  case UNKNOWN_TERRAIN:
    return colors->values[UNKNOWN_IDX];
  }
  return '\0';
}

const char *map_color_markup(char color) {
  bool bold = isupper((unsigned char)color);

  switch (tolower((unsigned char)color)) {
  case 'x':
    return bold ? "[fg=black bold]" : "[fg=black]";
  case 'r':
    return bold ? "[fg=red bold]" : "[fg=red]";
  case 'g':
    return bold ? "[fg=green bold]" : "[fg=green]";
  case 'y':
    return bold ? "[fg=yellow bold]" : "[fg=yellow]";
  case 'b':
    return bold ? "[fg=blue bold]" : "[fg=blue]";
  case 'm':
    return bold ? "[fg=magenta bold]" : "[fg=magenta]";
  case 'c':
    return bold ? "[fg=cyan bold]" : "[fg=cyan]";
  case 'w':
    return bold ? "[fg=white bold]" : "[fg=white]";
  case 'f':
    return bold ? "[blink bold]" : "[blink]";
  case 'i':
    return bold ? "[inverse bold]" : "[inverse]";
  case 'h':
    return "[bold]";
  default:
    return "";
  }
}

static MapCellText map_cell_text(char newc, char *prevc, char c) {
  MapCellText result = {0};

  if (newc == *prevc) {
    result.text[0] = c;
    return result;
  }

  snprintf(result.text, sizeof(result.text), "[reset]%s%c",
           map_color_markup(newc), c);
  *prevc = newc;
  return result;
}

static MapCellText lrs_mech_text(const MapColorScheme *colors, Mech *mech,
                                 Mech *other, int docolor, char *prevc) {
  char c = GetLRSMechChar(mech, other);
  char newc;

  if (!docolor) {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  }

  if (mech == other)
    newc = colors->values[SELF_IDX];
  else if (!MechSeemsFriend(mech, other))
    newc = colors->values[ENEMY_IDX];
  else
    newc = colors->values[FRIEND_IDX];

  return map_cell_text(newc, prevc, c);
}

static MapCellText lrs_terrain_text(const MapColorScheme *colors,
                                    BattleMap *map, int x, int y, int docolor,
                                    char *prevc) {
  char c = map_terrain_get(map, x, y);
  char newc;

  if (!c || !docolor || c == ' ') {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  } else
    newc = map_terrain_color_char(colors, c, map_elevation_get(map, x, y));

  return map_cell_text(newc, prevc, c);
}

static MapCellText lrs_elevation_text(const MapColorScheme *colors,
                                      BattleMap *map, int x, int y, int docolor,
                                      char *prevc) {
  int e = map_elevation_get(map, x, y);
  char c = (e || docolor) ? '0' + e : ' ';
  char newc;

  if (!docolor) {
    MapCellText result = {0};
    result.text[0] = c;
    return result;
  } else
    newc = map_terrain_color_char(colors, map_terrain_get(map, x, y), e);

  return map_cell_text(newc, prevc, c);
}

#define LRS_TERRAINMODE 1
#define LRS_ELEVMODE 2
#define LRS_MECHMODE 4
#define LRS_LOSMODE 8
#define LRS_COLORMODE 16
#define LRS_ELEVCOLORMODE 32

static MapCellText lrs_hex_text(const MapColorScheme *colors, Mech *mech,
                                BattleMap *map, int x, int y, char *prevc,
                                int mode, Mech **mechs, int lm,
                                HexLosMap *losmap) {
  int losflag = MAPLOSHEX_SEE | MAPLOSHEX_SEEN;

  if (mode & LRS_MECHMODE) {
    while (mechs[lm] && MechY(mechs[lm]) < y)
      lm++;
    while (mechs[lm] && MechY(mechs[lm]) == y && MechX(mechs[lm]) < x)
      lm++;
    if (mechs[lm] && MechY(mechs[lm]) == y && MechX(mechs[lm]) == x)
      return lrs_mech_text(colors, mech, mechs[lm], mode & LRS_COLORMODE,
                           prevc);
  }

  if (losmap)
    losflag = LOS_MAP_GET_FLAG(losmap, x, y);

  /* If the losmap doesn't contain this hex, we return X in bold red
   * in both terrain and elevation mode.
   */
  if (!(losflag & MAPLOSHEX_SEEN))
    return map_cell_text('R', prevc, 'X');

  if (((mode & LRS_TERRAINMODE) && !(losflag & MAPLOSHEX_SEETERRAIN)) ||
      ((mode & LRS_ELEVMODE) && !(losflag & MAPLOSHEX_SEEELEV)))
    return map_cell_text(map_terrain_color_char(colors, UNKNOWN_TERRAIN, 0),
                         prevc, '?');

  if (mode & LRS_ELEVMODE)
    return lrs_elevation_text(colors, map, x, y, mode & LRS_ELEVCOLORMODE,
                              prevc);
  if (mode & LRS_TERRAINMODE)
    return lrs_terrain_text(colors, map, x, y, mode & LRS_COLORMODE, prevc);

  btech_channel_send(
      mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
      tprintf("Unknown LRS mode, mech #%ld mode 0x%x.", mech->mynum, mode));
  return map_cell_text('R', prevc, 'Y');
}

static void show_lrs_map(const MapColorScheme *colors, DbRef player, Mech *mech,
                         BattleMap *map, int x, int y, int displayHeight,
                         int mode) {
  int loop, b_width, e_width, b_height, e_height, i;
  Mech *oMech;

  /* These buffers hold styled map cells and their coordinate labels. */
  char topbuff[LBUF_SIZE] = "    ";
  char botbuff[LBUF_SIZE] = "    ";
  char midbuff[LBUF_SIZE] = "    ";
  char trash1[16]; /* temp var to hold the map coordinate label */
  short oddcol = 0;
  Mech *mechs[MAX_MECHS_PER_MAP];
  int last_mech = 0;
  char prevct = 0, prevcb = 0;
  HexLosMap los_map_storage;
  HexLosMap *losmap = nullptr;

  /* x and y hold the viewing center of the map */
  b_width = x - LRS_DISPLAY_WIDTH / 2;
  b_width = MAX(b_width, 0);
  e_width = b_width + LRS_DISPLAY_WIDTH;
  if (e_width >= map->map_width) {
    e_width = map->map_width - 1;
    b_width = e_width - LRS_DISPLAY_WIDTH;
    b_width = MAX(b_width, 0);
  }

  if (b_width % 2)
    oddcol = 1;

  b_height = y - displayHeight / 2;
  b_height = MAX(b_height, 0);
  e_height = b_height + displayHeight;
  if (e_height > map->map_height) {
    e_height = map->map_height;
    b_height = e_height - displayHeight;
    b_height = MAX(b_height, 0);
  }

  /* Display the top labels */
  for (i = b_width; i <= e_width; i++) {
    snprintf(trash1, sizeof(trash1), "%3d", i);
    snprintf(topbuff + strlen(topbuff), sizeof(topbuff) - strlen(topbuff), "%c",
             trash1[0]);
    snprintf(midbuff + strlen(midbuff), sizeof(midbuff) - strlen(midbuff), "%c",
             trash1[1]);
    snprintf(botbuff + strlen(botbuff), sizeof(botbuff) - strlen(botbuff), "%c",
             trash1[2]);
  }
  notify(btech_context_evaluation(mech->xcode.context), player, topbuff);
  notify(btech_context_evaluation(mech->xcode.context), player, midbuff);
  notify(btech_context_evaluation(mech->xcode.context), player, botbuff);

  if (mode & LRS_MECHMODE) {
    for (i = 0; i < map->first_free; i++) {
      if ((oMech = btech_context_get_mech(mech->xcode.context,
                                          map->mechsOnMap[i]))) {
        if ((mech == oMech) ||
            (MechY(oMech) >= b_height && MechY(oMech) <= e_height &&
             MechX(oMech) >= b_width && MechX(oMech) <= e_width &&
             InLineOfSight(mech, oMech, MechX(oMech), MechY(oMech),
                           FlMechRange(map, mech, oMech))))
          mechs[last_mech++] = oMech;
      }
    }
    for (i = 0; i < (last_mech - 1); i++) /* Bubble-sort the list
                                           *  to y/x order */
      for (loop = (i + 1); loop < last_mech; loop++) {
        if (MechY(mechs[i]) > MechY(mechs[loop])) {
          oMech = mechs[i];
          mechs[i] = mechs[loop];
          mechs[loop] = oMech;
        } else if (MechY(mechs[i]) == MechY(mechs[loop]) &&
                   MechX(mechs[i]) > MechX(mechs[loop])) {
          oMech = mechs[i];
          mechs[i] = mechs[loop];
          mechs[loop] = oMech;
        }
      }
    mechs[last_mech] = NULL;
    last_mech = 0;
  }

  if ((mode & LRS_LOSMODE) &&
      los_map_calculate(&los_map_storage, map, mech, b_width, b_height,
                        e_width - b_width, e_height - b_height))
    losmap = &los_map_storage;

  for (loop = b_height; loop < e_height; loop++) {
    snprintf(topbuff, sizeof(topbuff), "%3d ", loop);
    strcpy(botbuff, "    ");
    if (mode & LRS_MECHMODE)
      while (mechs[last_mech] && MechY(mechs[last_mech]) < loop)
        last_mech++;

    for (i = b_width; i < e_width; i += 2) {
      snprintf(topbuff + strlen(topbuff), sizeof(topbuff) - strlen(topbuff),
               oddcol ? "%s " : " %s",
               lrs_hex_text(colors, mech, map, i + !oddcol, loop, &prevct, mode,
                            mechs, last_mech, losmap)
                   .text);

      snprintf(botbuff + strlen(botbuff), sizeof(botbuff) - strlen(botbuff),
               oddcol ? " %s" : "%s ",
               lrs_hex_text(colors, mech, map, i + oddcol, loop, &prevcb, mode,
                            mechs, last_mech, losmap)
                   .text);
    }
    if (i == e_width && !oddcol) {
      snprintf(botbuff + strlen(botbuff), sizeof(botbuff) - strlen(botbuff),
               "%s",
               lrs_hex_text(colors, mech, map, i, loop, &prevcb, mode, mechs,
                            last_mech, losmap)
                   .text);
    } else if (i == e_width) {
      snprintf(topbuff + strlen(topbuff), sizeof(topbuff) - strlen(topbuff),
               "%s",
               lrs_hex_text(colors, mech, map, i, loop, &prevct, mode, mechs,
                            last_mech, losmap)
                   .text);
      strcat(botbuff, " ");
    }

    if (mode & (LRS_COLORMODE | LRS_ELEVCOLORMODE)) {
      if (prevct) {
        strcat(topbuff, "[reset]");
        prevct = 0;
      }
      if (prevcb) {
        strcat(botbuff, "[reset]");
        prevcb = 0;
      }
    }
    snprintf(botbuff + strlen(botbuff), sizeof(botbuff) - strlen(botbuff),
             " %-3d", loop);
    notify(btech_context_evaluation(mech->xcode.context), player, topbuff);
    notify(btech_context_evaluation(mech->xcode.context), player, botbuff);
  }
}

void mech_lrsmap(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  MapColorScheme colors;
  BattleMap *map;
  int argc, mode = 0;
  short x, y;
  char *args[5], *str;
  int displayHeight = LRS_DISPLAY_HEIGHT;

  cch(MECH_USUAL);

  if (is_ansi(mech->xcode.context->database, player))
    mode |= LRS_COLORMODE;

  map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(mech->xcode.context, !MechLRSRange(mech),
                  "Your system seems to be inoperational.");
  if (!parse_tacargs(player, mech, &args[1], argc - 1, MechLRSRange(mech), &x,
                     &y))
    return;
  switch (args[0][0]) {
  case 'M':
  case 'm':
    mode |= LRS_MECHMODE | LRS_TERRAINMODE;
    break;
  case 'E':
  case 'e':
    mode |= LRS_ELEVMODE;
    break;
  case 'C':
  case 'c':
    mode |= LRS_ELEVMODE | LRS_ELEVCOLORMODE;
    break;
  case 'T':
  case 't':
    mode |= LRS_TERRAINMODE;
    break;
  case 'L':
  case 'l':
    mode |= LRS_LOSMODE | LRS_TERRAINMODE;
    break;
  case 'H':
  case 'h':
    mode |= LRS_LOSMODE | LRS_ELEVMODE;
    break;
  case 'S':
  case 's':
    mode |= LRS_LOSMODE | LRS_MECHMODE | LRS_TERRAINMODE;
    break;
  default:
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Unknown LRS sensor type '%s'!", args[0]);
    return;
  }

  if (MapIsDark(map) || (MechType(mech) == CLASS_MW &&
                         mech->xcode.context->configuration->btech_mw_losmap))
    mode |= LRS_LOSMODE;

  str = btech_attribute_read(mech->xcode.context->database, player, A_LRSHEIGHT,
                             (char[LBUF_SIZE]){0});
  if (*str) {
    displayHeight = atoi(str);
    if (displayHeight < 10 || displayHeight > 40) {
      notify(btech_context_evaluation(mech->xcode.context), player,
             "Illegal LRSHeight attribute.  Must be between 10 and 40");
      displayHeight = LRS_DISPLAY_HEIGHT;
    }
  }

  displayHeight = MIN(displayHeight, 2 * MechLRSRange(mech));
  displayHeight = MIN(displayHeight, map->map_height);

  if (!(displayHeight % 2))
    displayHeight++;

  map_color_scheme_load(&colors, mech->xcode.context, player);

  show_lrs_map(&colors, player, mech, map, x, y, displayHeight, mode);
}
