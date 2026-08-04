/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <ctype.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_contacts_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "registry_api.h"

static const char default_contactoptions[] = "!db";

static char *const ac_desc[] = {
    "0 - See enemies and friends, long text, color",
    "1 - See enemies and friends, short text, color",
    "2 - See enemies only, long text, color",
    "3 - See enemies only, short text, color",
    "4 - See enemies and friends, short text, no color",
    "5 - See enemies only, short text, no color",

    "6 - Disabled"};

static char *const c_desc[] = {
    "0 - Very verbose", "1 - Short form, the usual one",
    "2 - Short form, the usual one, but do not see buildings",
    "3 - Shorter form"};

void show_brief_flags(DbRef player, Mech *mech) {
  notify_printf(
      btech_context_evaluation(mech_context(mech)), player,
      "Brief status for %s:", mech_to_mech_display_id(mech, mech).text);
#ifdef ADVANCED_LOS
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "    (A)utocontacts: %s", ac_desc[mech_brief_mode(mech) / 4]);
#endif
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "    (C)ontacts:     %s", c_desc[mech_brief_mode(mech) % 4]);
}

void mech_brief(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char c;
  int v;

  cch(MECH_USUALSM);
  skipws(buffer);
  if (!*buffer) {
    show_brief_flags(player, mech);
    return;
  }
  c = *buffer;
  buffer++;
  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer, "Argument missing!");
  DOCHECK_CONTEXT(mech_context(mech), Readnum(v, buffer), "Invalid number!");
  switch (toupper(c)) {
#ifdef ADVANCED_LOS
  case 'A':
    DOCHECK_CONTEXT(mech_context(mech), v < 0 || v > 6, "Number out of range!");
    v = BOUNDED(0, v, 6);
    mech_brief_mode_set(mech, mech_brief_mode(mech) % 4 + v * 4);
    mech_printf(mech, MECHALL, "Autocontact brevity set to %s.", ac_desc[v]);
    return;
#endif
  case 'C':
    DOCHECK_CONTEXT(mech_context(mech), v < 0 || v > 3, "Number out of range!");
    v = BOUNDED(0, v, 3);
    mech_brief_mode_set(mech, ((mech_brief_mode(mech) / 4) * 4) + v);
    mech_printf(mech, MECHALL, "Contact brevity set to %s.", c_desc[v]);
    return;
  }
}

#define SEE_DEAD 0x01
#define SEE_SHUTDOWN 0x02
#define SEE_ALLY 0x04
#define SEE_ENEMA 0x08
#define SEE_TARGET 0x10
#define SEE_BUILDINGS 0x20
#define SEE_NEGNEXT 0x80

char getWeaponArc(Mech *mech, int arc) {
  if (arc & FORWARDARC)
    return '*';
  else if (arc & TURRETARC)
    return 't';
  else if (arc & RSIDEARC)
    return 'r';
  else if (arc & LSIDEARC)
    return 'l';
  else if (arc & REARARC)
    return 'v';
  else
    return '?';
}

/* who: 0 for friend, 1 for enemy, 2 for 'self' */
MechStatusString mech_status_string(Mech *target, int who) {
  MechStatusString status = {0};
  char *statusstr = status.text;
  int sptr = 0;

  if (Destroyed(target))
    statusstr[sptr++] = 'D';

  if (mech_event_count(target, EVENT_STARTUP))
    statusstr[sptr++] = 's';
  else if (!Started(target))
    statusstr[sptr++] = 'S';

  if (mech_event_count(target, EVENT_STAND))
    statusstr[sptr++] = 'f';
  else if (Fallen(target))
    statusstr[sptr++] = 'F';

  if (mech_event_count(target, EVENT_CHANGING_HULLDOWN))
    statusstr[sptr++] = 'h';
  else if (IsHulldown(target))
    statusstr[sptr++] = 'H';

  if (Towed(target))
    statusstr[sptr++] = 'T';
  else if (MechCarrying(target) > 0)
    statusstr[sptr++] = 't';

  if (Jumping(target))
    statusstr[sptr++] = 'J';

  if (OODing(target))
    statusstr[sptr++] = 'O';

  if (MechHeat(target))
    statusstr[sptr++] = '+';

  if (Jellied(target))
    statusstr[sptr++] = 'I';

  if (mech_event_count(target, EVENT_VEHICLEBURN))
    statusstr[sptr++] = 'B';

  if (MechLites(target))
    statusstr[sptr++] = 'L';

  if (MechLit(target))
    statusstr[sptr++] = 'l';

  if (MechSwarmTarget(target) > 0)
    statusstr[sptr++] = 'W';

  if (CarryingClub(target))
    statusstr[sptr++] = 'C';

  if (checkAllSections(target, NARC_ATTACHED) ||
      checkAllSections(target, INARC_HOMING_ATTACHED)) {
    if (who == 1)
      statusstr[sptr++] = 'N';
    else
      statusstr[sptr++] = 'n';
  }
#ifndef ECM_ON_CONTACTS
  if (who > 1) {
#endif
    if (AnyECCMActive(target))
      statusstr[sptr++] = 'P';

    if (AnyECMActive(target))
      statusstr[sptr++] = 'E';

    if (AnyECMProtected(target))
      statusstr[sptr++] = 'p';

    if (AnyECMDisturbed(target))
      statusstr[sptr++] = 'e';
#ifndef ECM_ON_CONTACTS
  }
#endif

  if (Spinning(target))
    statusstr[sptr++] = 'X';

#ifdef BT_MOVEMENT_MODES
  if (Sprinting(target))
    statusstr[sptr++] = 'M';
  if (Evading(target))
    statusstr[sptr++] = 'm';
#endif

  statusstr[sptr] = '\0';
  return status;
}

char getStatusChar(Mech *mech, Mech *mechTarget, int wCharNum) {
  char cRet = ' ';

  switch (wCharNum) {
  case 1:
    cRet = MechSwarmTarget(mechTarget) > 0 ? 'W'
           : Towed(mechTarget)             ? 'T'
           : MechCarrying(mechTarget) > 0  ? 't'
           : CarryingClub(mechTarget)      ? 'C'
           :
#ifdef BT_MOVEMENT_MODES
           Sprinting(mechTarget) ? 'M'
           : Evading(mechTarget) ? 'm'
                                 :
#endif
                                 ' ';
    break;
  case 2:
    cRet = Destroyed(mechTarget)   ? 'D'
           : MechLites(mechTarget) ? 'L'
           : MechLit(mechTarget)   ? 'l'
                                   : ' ';
    break;
  case 3:
    cRet = Jumping(mechTarget)                                     ? 'J'
           : OODing(mechTarget)                                    ? 'O'
           : Fallen(mechTarget)                                    ? 'F'
           : mech_event_count(mechTarget, EVENT_STAND)             ? 'f'
           : mech_event_count(mechTarget, EVENT_CHANGING_HULLDOWN) ? 'h'
           : IsHulldown(mechTarget)                                ? 'H'
           : Spinning(mech)                                        ? 'X'
                                                                   : ' ';
    break;
  case 4:
    cRet = Started(mechTarget)
               ? (MechHeat(mechTarget)                              ? '+'
                  : Jellied(mechTarget)                             ? 'I'
                  : mech_event_count(mechTarget, EVENT_VEHICLEBURN) ? 'B'
                                                                    : ' ')
           : Staggering(mechTarget)                      ? 'G'
           : mech_event_count(mechTarget, EVENT_STARTUP) ? 's'
                                                         : 'S';
    break;
  case 5:
    cRet = (checkAllSections(mechTarget, NARC_ATTACHED) ||
            checkAllSections(mechTarget, INARC_HOMING_ATTACHED))
               ? (MechTeam(mechTarget) == MechTeam(mech) ? 'n' : 'N')
           :
#ifdef ECM_ON_CONTACTS
           AnyECCMActive(mechTarget)     ? 'P'
           : AnyECMActive(mechTarget)    ? 'E'
           : AnyECMProtected(mechTarget) ? 'p'
           : AnyECMDisturbed(mechTarget) ? 'e'
                                         :
#endif
                                         ' ';
    break;
  }

  return cRet;
}

void mech_contacts(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *tempMech;
  BattleMap *mech_map =
                btech_context_get_map(mech_context(mech), mech_map_dbref(mech)),
            *tmp_map;
  MapObject *building;
  int loop, i, j, argc, bearing, buffindex = 0;
  char *args[1], bufflist[MAX_MECHS_PER_MAP][120], buff[100];
  int sbuff[MAX_MECHS_PER_MAP];
  float range, rangelist[MAX_MECHS_PER_MAP], fx, fy;
  char weaponarc;
  char *mech_name;
  unsigned char see_what;
  char *str;
  char move_type[30];
  char cStatus1, cStatus2, cStatus3, cStatus4, cStatus5;
  int losflag;
  int isvb;
  int inlos;
  char new[LBUF_SIZE];
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  cch(MECH_USUAL);
  argc = mech_parseattributes(buffer, args, 1);

  isvb = (mech_brief_mode(mech) % 4);
  if (argc > 0) {
    if (args[0][0] == '+') {
      str = btech_attribute_read(mech_context(mech)->database, player,
                                 A_CONTACTOPT, (char[LBUF_SIZE]){0});
      if (!*str)
        strcpy(buff, default_contactoptions);
      else {
        strncpy(buff, str, 50);
        buff[49] = 0;

        if (strlen(buff) == 0)
          strcpy(buff, default_contactoptions);
      }
    } else {
      strncpy(buff, args[0], 50);
      buff[49] = 0;
    }

    if (isvb == 1)
      see_what = SEE_BUILDINGS;
    else
      see_what = 0x0;

    for (loop = 0; loop < 50 && buff[loop]; loop++) {
      char c;

      c = buff[loop];

      if (c == 'd')

        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_DEAD)
                                 : (see_what |= SEE_DEAD);
      else if (c == 's')
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_SHUTDOWN)
                                 : (see_what |= SEE_SHUTDOWN);
      else if (c == 'b')
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_BUILDINGS)
                                 : (see_what |= SEE_BUILDINGS);
      else if (c == 'e')
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_ENEMA)
                                 : (see_what |= SEE_ENEMA);
      else if (c == 'a')
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_ALLY)
                                 : (see_what |= SEE_ALLY);
      else if (c == 't')
        (see_what & SEE_NEGNEXT) ? (see_what &= ~SEE_TARGET)
                                 : (see_what |= SEE_TARGET);
      else if (c == '!') {
        see_what = (SEE_NEGNEXT | SEE_DEAD | SEE_SHUTDOWN | SEE_ENEMA |
                    SEE_ALLY | SEE_TARGET);
      } else
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      "Ignoring %c as contact option.", c);
    }
  } else {
    see_what = (SEE_DEAD | SEE_SHUTDOWN | SEE_ENEMA | SEE_ALLY | SEE_TARGET);
    if (isvb == 1)
      see_what |= SEE_BUILDINGS;
  }

  if (isvb <= 2)
    notify(btech_context_evaluation(mech_context(mech)), player,
           "Line of Sight Contacts:");

  for (loop = 0; loop < mech_map->first_free; loop++) {
    if (!(mech_map->mechsOnMap[loop] != mech_dbref(mech) &&
          mech_map->mechsOnMap[loop] != -1))
      continue;

    tempMech = (Mech *)btech_context_find_object(mech_context(mech),
                                                 mech_map->mechsOnMap[loop]);

    if (!tempMech)
      continue;
    if (argc) {
      if (!((MechSeemsFriend(mech, tempMech) ? (see_what & SEE_ALLY)
                                             : (see_what & SEE_ENEMA)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == MechTarget(mech)))))
        continue;
      if (!(((see_what & SEE_SHUTDOWN) || Started(tempMech)) ||
            Destroyed(tempMech) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == MechTarget(mech)))))
        continue;
      if (!(((see_what & SEE_DEAD) || !Destroyed(tempMech)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == MechTarget(mech)))))
        continue;
    }
    range = FlMechRange(mech_map, mech, tempMech);
    if (!(losflag = InLineOfSight(mech, tempMech, MechX(tempMech),
                                  MechY(tempMech), range)))
      continue;
    if (is_good_obj(mech_context(mech)->database, mech_dbref(tempMech))) {
      if (!InLineOfSight_NB(mech, tempMech, MechX(tempMech), MechY(tempMech),
                            0.0)) {
        mech_name = "something";
        inlos = 0;
      } else {
        mech_name = btech_attribute_read(mech_context(tempMech)->database,
                                         mech_dbref(tempMech), A_MECHNAME,
                                         (char[LBUF_SIZE]){0});
        inlos = 1;
      }
    } else
      continue;
    bearing = FindBearing(MechFX(mech), MechFY(mech), MechFX(tempMech),
                          MechFY(tempMech));
    weaponarc = getWeaponArc(
        mech, InWeaponArc(mech, MechFX(tempMech), MechFY(tempMech)));

    strcpy(move_type, GetMoveTypeID(MechMove(tempMech)));

    if (isvb) {
      if (!inlos) {
        cStatus1 = ' ';
        cStatus2 = ' ';
        cStatus3 = ' ';
        cStatus4 = ' ';
        cStatus5 = ' ';
      } else {
        cStatus1 = getStatusChar(mech, tempMech, 1);
        cStatus2 = getStatusChar(mech, tempMech, 2);
        cStatus3 = getStatusChar(mech, tempMech, 3);
        cStatus4 = getStatusChar(mech, tempMech, 4);
        cStatus5 = getStatusChar(mech, tempMech, 5);
      }

      snprintf(buff, sizeof(buff),
               "%s%c%c%c[%s]%c %-12.12s x:%3d y:%3d z:%3d r:%4.1f b:%3d "
               "s:%5.1f h:%3d S:%c%c%c%c%c%s",
               mech_dbref(tempMech) == MechTarget(mech) ? "[fg=red bold]"
               : !MechSeemsFriend(mech, tempMech)       ? "[fg=yellow bold]"
                                                        : "",
               (losflag & MECHLOSFLAG_SEESP) ? 'P' : ' ',
               (losflag & MECHLOSFLAG_SEESS) ? 'S' : ' ', weaponarc,
               mech_id(tempMech, MechSeemsFriend(mech, tempMech)).text,
               move_type[0], mech_name, MechX(tempMech), MechY(tempMech),
               MechZ(tempMech), range, bearing, MechSpeed(tempMech),
               MechVFacing(tempMech), cStatus1, cStatus2, cStatus3, cStatus4,
               cStatus5,
               (mech_dbref(tempMech) == MechTarget(mech) ||
                !MechSeemsFriend(mech, tempMech))
                   ? "[reset]"
                   : "");

      rangelist[buffindex] = range;
      rangelist[buffindex] += (MechStatus(tempMech) & DESTROYED) ? 10000 : 0;
      strcpy(bufflist[buffindex++], buff);
    } else {
      snprintf(buff, sizeof(buff), "[%s] %-17s  Tonnage: %d",
               mech_id(tempMech, MechSeemsFriend(mech, tempMech)).text,
               mech_name, MechTons(tempMech));
      notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Range: %.1f hex\tBearing: %d degrees",
               range, bearing);
      notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Speed: %.1f KPH\tHeading: %d degrees",
               MechSpeed(tempMech), MechVFacing(tempMech));
      notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      X, Y: %3d, %3d \tHeat: %.0f deg C.",
               MechX(tempMech), MechY(tempMech), MechHeat(tempMech));
      notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Movement Type: %s", move_type);
      notify(btech_context_evaluation(mech_context(mech)), player, buff);
      notify_printf(btech_context_evaluation(mech_context(mech)), player,
                    "      Mech is in %s Arc",
                    GetArcID(mech, InWeaponArc(mech, MechFX(tempMech),
                                               MechFY(tempMech))));
      if (MechStatus(tempMech) & DESTROYED)
        notify(btech_context_evaluation(mech_context(mech)), player,
               "      Mech Destroyed");
      if (!(MechStatus(tempMech) & STARTED))
        notify(btech_context_evaluation(mech_context(mech)), player,
               "      Mech Shutdown");
      if (Fallen(tempMech))
        notify(btech_context_evaluation(mech_context(mech)), player,
               "      Mech has Fallen!");
      if (Jumping(tempMech))
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      "      Mech is Jumping!\tJump Heading: %d",
                      MechJumpHeading(tempMech));
      notify(btech_context_evaluation(mech_context(mech)), player, " ");
    }
  }

  if (see_what & SEE_BUILDINGS) {
    for (building = first_mapobj(mech_map, TYPE_BUILD); building;
         building = next_mapobj(building)) {

      MapCoordToRealCoord(building->x, building->y, &fx, &fy);
      range = FindRange(
          MechFX(mech), MechFY(mech), MechFZ(mech), fx, fy,
          ZSCALE * ((i = Elevation(mech_map, building->x, building->y)) + 1));

      losflag = InLineOfSight(mech, NULL, building->x, building->y, range);
      if (!losflag || (losflag & MECHLOSFLAG_BLOCK))
        continue;

      if (!(building->obj && (tmp_map = btech_context_get_map(
                                  mech_context(mech), building->obj))))
        continue;
      if (BuildIsInvis(tmp_map))
        continue;
      if ((j = !lock_test(btech_context_evaluation(mech_context(mech)), player,
                          player, mech_dbref(mech), tmp_map->mynum,
                          LUA_LOCK_ENTER, LUA_LOCK_OPERATION_BTECH_CONTACT,
                          true, &lock, &lock_result)) &&
          BuildIsHidden(tmp_map))
        continue;
      bearing = FindBearing(MechFX(mech), MechFY(mech), fx, fy);
      weaponarc = getWeaponArc(mech, InWeaponArc(mech, fx, fy));

      mech_name =
          btech_attribute_read(mech_context(mech)->database, building->obj,
                               A_MECHNAME, (char[LBUF_SIZE]){0});
      if (!mech_name || !*mech_name) {
        strncpy(new,
                game_object_name(mech_context(mech)->database, building->obj),
                LBUF_SIZE - 1);
        styled_text_strip(
            mech_context(mech)->database->styled_text_palette,
            game_object_name(mech_context(mech)->database, building->obj), new,
            sizeof(new));
        mech_name = new;
      }

      snprintf(buff, sizeof(buff),
               "%s%c%c%c %-23.23s x:%3d y:%3d z:%2d r:%4.1f b:%3d CF:%4d /%4d "
               "S:%c%c%s",
               j ? "[fg=yellow bold]" : "",
               (losflag & MECHLOSFLAG_SEESP) ? 'P' : ' ',
               (losflag & MECHLOSFLAG_SEESS) ? 'S' : ' ', weaponarc, mech_name,
               building->x, building->y, i, range, bearing, tmp_map->cf,
               tmp_map->cfmax,
               (BuildIsSafe(tmp_map) || (j && BuildIsCS(tmp_map))) ? 'X'
               : j                                                 ? 'x'
               : BuildIsCS(tmp_map)                                ? 'C'
                                                                   : ' ',
               BuildIsHidden(tmp_map) ? 'H' : ' ', j ? "[reset]" : "");
      rangelist[buffindex] = range + 20000;
      strcpy(bufflist[buffindex++], buff);
    }
  }

  if (isvb) {
    for (i = 0; i < buffindex; i++)
      sbuff[i] = i;
    /* print a sorted list of detected mechs */
    /* use the ever-popular bubble sort */
    for (i = 0; i < (buffindex - 1); i++)
      for (j = (i + 1); j < buffindex; j++)
        if (rangelist[sbuff[j]] > rangelist[sbuff[i]]) {
          loop = sbuff[i];
          sbuff[i] = sbuff[j];
          sbuff[j] = loop;
        }
    for (loop = 0; loop < buffindex; loop++)
      notify(btech_context_evaluation(mech_context(mech)), player,
             bufflist[sbuff[loop]]);
  }

  if (isvb <= 2)
    notify(btech_context_evaluation(mech_context(mech)), player,
           "End Contact List");
}

#undef SEE_DEAD
#undef SEE_SHUTDOWN
#undef SEE_ALLY
#undef SEE_ENEMA
#undef SEE_TARGET
#undef SEE_BUILDINGS
#undef SEE_NEGNEXT
