#include "mech_notify_internal.h"

void MechLOSBroadcast(Mech *mech, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  char buf[LBUF_SIZE];

  possibly_see_mech(mech);
  if (!mech_map)
    return;
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1 && mech_map->mechsOnMap[i] != mech->mynum)
      if ((tempMech = btech_context_get_mech(mech_map->xcode.context,
                                             mech_map->mechsOnMap[i])))
        if (InLineOfSight(tempMech, mech, MechX(mech), MechY(mech),
                          FlMechRange(mech_map, tempMech, mech))) {
          snprintf(buf, sizeof(buf), "%s%s%s",
                   mech_to_mech_display_id(tempMech, mech).text,
                   *message != '\'' ? " " : "", message);
          mech_notify(tempMech, MECHSTARTED, buf);
        }
}

int MechSeesHexF(Mech *mech, BattleMap *map, float x, float y, int ix, int iy) {
  return (InLineOfSight(mech, NULL, ix, iy,
                        FindRange(MechFX(mech), MechFY(mech), MechFZ(mech), x,
                                  y, ZSCALE * Elevation(map, ix, iy))));
}

int MechSeesHex(Mech *mech, BattleMap *map, int x, int y) {
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  return MechSeesHexF(mech, map, fx, fy, x, y);
}

void HexLOSBroadcast(BattleMap *mech_map, int x, int y, char *message) {
  int i;
  Mech *tempMech;
  float fx, fy;

  /* substitution:
     $h = !alarming ('your hex', '%d,%d')
     $H = alarming ('YOUR HEX', '%d,%d (%.2f away)')
   */
  if (!mech_map)
    return;
  MapCoordToRealCoord(x, y, &fx, &fy);
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1)
      if ((tempMech = btech_context_get_mech(mech_map->xcode.context,
                                             mech_map->mechsOnMap[i])))
        if (MechSeesHexF(tempMech, mech_map, fx, fy, x, y)) {
          char tbuf[LBUF_SIZE];
          char *c, *d = tbuf;
          int done;

          for (c = message; *c; c++) {
            done = 0;
            if (*c == '$') {
              if (*(c + 1) == 'h' || *(c + 1) == 'H') {
                c++;
                if (*c == 'h') {
                  if (x == MechX(tempMech) && y == MechY(tempMech))
                    strcpy(d, "your hex");
                  else
                    snprintf(d, sizeof(tbuf) - (tbuf - d), "%d,%d", x, y);
                  while (*d)
                    d++;
                } else {
                  /* Dangerous */
                  if (x == MechX(tempMech) && y == MechY(tempMech))
                    strcpy(d, "[fg=red bold]YOUR HEX[reset]");
                  else
                    snprintf(d, sizeof(tbuf) - (tbuf - d),
                             "[fg=yellow bold]%d,%d[reset]", x, y);
                  while (*d)
                    d++;
                }
                done = 1;
              }
            }
            if (!done)
              *(d++) = *c;
          }
          /* Apparently, it's necessary to remove trailing $'s ?? */
          if (d > tbuf && *(d - 1) == '$')
            d--;
          *d = '\0';
          mech_notify(tempMech, MECHSTARTED, tbuf);
        }
}

static void format_mech_los_message(char *buffer, size_t buffer_size,
                                    const char *message,
                                    const char *target_name) {
  const char *placeholder = strstr(message, "%s");

  if (!placeholder) {
    snprintf(buffer, buffer_size, "%s", message);
    return;
  }
  snprintf(buffer, buffer_size, "%.*s%s%s", (int)(placeholder - message),
           message, target_name, placeholder + 2);
}

void MechLOSBroadcasti(Mech *mech, Mech *target, const char *message) {
  /* Sends msg to everyone except the mech */
  int i, a, b;
  char oddbuff[LBUF_SIZE];
  char oddbuff2[LBUF_SIZE];
  Mech *tempMech;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);

  if (!mech_map)
    return;
  possibly_see_mech(mech);
  possibly_see_mech(target);
  for (i = 0; i < mech_map->first_free; i++)
    if (mech_map->mechsOnMap[i] != -1 &&
        mech_map->mechsOnMap[i] != mech->mynum &&
        mech_map->mechsOnMap[i] != target->mynum)
      if ((tempMech = btech_context_get_mech(mech->xcode.context,
                                             mech_map->mechsOnMap[i]))) {
        a = InLineOfSight(tempMech, mech, MechX(mech), MechY(mech),
                          FlMechRange(mech_map, tempMech, mech));
        b = InLineOfSight(tempMech, target, MechX(target), MechY(target),
                          FlMechRange(mech_map, tempMech, target));
        if (a || b) {
          char *obp = oddbuff2;

          format_mech_los_message(
              oddbuff, sizeof(oddbuff), message,
              b ? mech_to_mech_display_id(tempMech, target).text : "someone");
          safe_str((char *)(a ? mech_to_mech_display_id(tempMech, mech).text
                              : "Someone"),
                   oddbuff2, &obp);
          if (*oddbuff != '\'')
            safe_chr(' ', oddbuff2, &obp);
          safe_str(oddbuff, oddbuff2, &obp);
          *obp = '\0';
          mech_notify(tempMech, MECHSTARTED, oddbuff2);
        }
      }
}

void MapBroadcast(BattleMap *map, char *message) {
  /* Sends msg to everyone except the mech */
  int i;
  Mech *tempMech;

  for (i = 0; i < map->first_free; i++)
    if (map->mechsOnMap[i] != -1)
      if ((tempMech =
               btech_context_get_mech(map->xcode.context, map->mechsOnMap[i])))
        mech_notify(tempMech, MECHSTARTED, message);
}

void MechFireBroadcast(Mech *mech, Mech *target, int x, int y,
                       BattleMap *mech_map, char *weapname, int IsHit) {
  int loop, attacker, defender;
  float fx, fy, fz;
  int mapx, mapy;
  Mech *tempMech;
  char buff[SBUF_SIZE];

  /* Stat recording */
  if (target) { /* only if we have a mecha */
    if (IsHit)
      MechShotsHit(mech)++;
    else
      MechShotsMissed(mech)++;
  }

  possibly_see_mech(mech);
  if (target) {
    possibly_see_mech(target);
    mapx = MechX(target);
    mapy = MechY(target);
    fx = MechFX(target);
    fy = MechFY(target);
    fz = MechFZ(target);
    for (loop = 0; loop < mech_map->first_free; loop++)
      if (mech_map->mechsOnMap[loop] != mech->mynum &&
          mech_map->mechsOnMap[loop] != -1 &&
          mech_map->mechsOnMap[loop] != target->mynum) {
        attacker = 0;
        defender = 0;
        tempMech = (Mech *)btech_context_find_object(
            mech->xcode.context, mech_map->mechsOnMap[loop]);
        if (!tempMech)
          continue;
        if (InLineOfSight(tempMech, mech, MechX(mech), MechY(mech),
                          FlMechRange(mech_map, tempMech, mech)))
          attacker = 1;
        if (target) {
          if (InLineOfSight(tempMech, target, mapx, mapy,
                            FlMechRange(mech_map, tempMech, target)))
            defender = 1;
        } else if (InLineOfSight(tempMech, target, mapx, mapy,
                                 FindRange(MechFX(tempMech), MechFY(tempMech),
                                           MechFZ(tempMech), fx, fy, fz)))
          defender = 1;

        if (!attacker && !defender)
          continue;
        if (defender)
          snprintf(buff, sizeof(buff), "%s",
                   mech_to_mech_display_id(tempMech, target).text);
        if (attacker) {
          if (defender)
            mech_printf(tempMech, MECHSTARTED, "%s %s %s with a %s",
                        mech_to_mech_display_id(tempMech, mech).text,
                        IsHit ? "hits" : "misses", buff, weapname);
          else
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(tempMech, mech).text, weapname);
        } else
          mech_printf(tempMech, MECHSTARTED, "Something %s %s with a %s",
                      IsHit ? "hits" : "misses", buff, weapname);
      }
  } else {
    mapx = x;
    mapy = y;
    MapCoordToRealCoord(x, y, &fx, &fy);
    fz = ZSCALE * Elevation(mech_map, x, y);
    snprintf(buff, sizeof(buff), "hex %d %d!", mapx, mapy);
    for (loop = 0; loop < mech_map->first_free; loop++)
      if (mech_map->mechsOnMap[loop] != mech->mynum &&
          mech_map->mechsOnMap[loop] != -1) {
        attacker = 0;
        defender = 0;
        tempMech = (Mech *)btech_context_find_object(
            mech->xcode.context, mech_map->mechsOnMap[loop]);
        if (!tempMech)
          continue;
        if (InLineOfSight(tempMech, mech, MechX(mech), MechY(mech),
                          FlMechRange(mech_map, tempMech, mech)))
          attacker = 1;
        if (target) {
          if (InLineOfSight(tempMech, target, mapx, mapy,
                            FlMechRange(mech_map, tempMech, target)))
            defender = 1;
        } else if (InLineOfSight(tempMech, target, mapx, mapy,
                                 FindRange(MechFX(tempMech), MechFY(tempMech),
                                           MechFZ(tempMech), fx, fy, fz)))
          defender = 1;
        if (!attacker && !defender)
          continue;
        if (attacker) {
          if (defender) /* att + def */
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at %s",
                        mech_to_mech_display_id(tempMech, mech).text, weapname,
                        buff);
          else /* att */
            mech_printf(tempMech, MECHSTARTED, "%s fires a %s at something!",
                        mech_to_mech_display_id(tempMech, mech).text, weapname);
        } else /* def */
          mech_printf(tempMech, MECHSTARTED, "Something fires a %s at %s",
                      weapname, buff);
      }
  }
}

void mech_notify(Mech *mech, int type, char *buffer) {
  int i;

  if (Uncon(mech))
    return;
  if (Blinded(mech))
    return;
  if (mech->mynum < 0)
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  /* Let's do colorization too, just in case. */

  if (type == MECHPILOT) {
    if (mech_has_pilot(mech))
      notify(evaluation, MechPilot(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((type == MECHALL && !Destroyed(mech)) ||
             (type == MECHSTARTED && Started(mech))) {
    notify_except(evaluation, mech->mynum, NOTHING, mech->mynum, buffer);
    if (mech->xcode.context->combat_overrides.arcs)
      for (i = 0; i < NUM_TURRETS; i++)
        if (AeroTurret(mech, i) > 0)
          notify_except(evaluation, AeroTurret(mech, i), NOTHING,
                        AeroTurret(mech, i), buffer);
  }
}

void mech_printf(Mech *mech, int type, char *format, ...) {
  char buffer[LBUF_SIZE];
  int i;
  va_list ap;

  if (Uncon(mech))
    return;
  if (Blinded(mech))
    return;
  if (mech->mynum < 0)
    return;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  /* Let's do colorization too, just in case. */

  va_start(ap, format);
  vsnprintf(buffer, LBUF_SIZE, format, ap);
  va_end(ap);

  if (type == MECHPILOT) {
    if (mech_has_pilot(mech))
      notify(evaluation, MechPilot(mech), buffer);
    else
      mech_notify(mech, MECHALL, buffer);
  } else if ((type == MECHALL && !Destroyed(mech)) ||
             (type == MECHSTARTED && Started(mech))) {
    notify_except(evaluation, mech->mynum, NOTHING, mech->mynum, buffer);
    if (mech->xcode.context->combat_overrides.arcs)
      for (i = 0; i < NUM_TURRETS; i++)
        if (AeroTurret(mech, i) > 0)
          notify_except(evaluation, AeroTurret(mech, i), NOTHING,
                        AeroTurret(mech, i), buffer);
  }
}
