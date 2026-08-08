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
#include "map_building_query_api.h"
#include "map_los_types.h"
#include "map_object_query_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_contacts_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/styled_text/markup.h"
#include "mux/world/access.h"
#include "registry_api.h"
#include "section_types.h"

static const char default_contactoptions[] = "!db";

static bool mech_contact_is_friend(Mech *observer, Mech *target) {
  return mech_team(observer) == mech_team(target) &&
         mech_los_check_unblocked(observer, target, 0, 0, 0);
}

static int mech_contact_heading(const Mech *mech) {
  return AcceptableDegree(mech_heading_degrees(mech) +
                          mech_lateral_movement(mech));
}

static bool mech_contact_carries_club(const Mech *mech) {
  return mech_section_carries_club(mech, RARM) ||
         mech_section_carries_club(mech, LARM);
}

static const char *const ac_desc[] = {
    "0 - See enemies and friends, long text, color",
    "1 - See enemies and friends, short text, color",
    "2 - See enemies only, long text, color",
    "3 - See enemies only, short text, color",
    "4 - See enemies and friends, short text, no color",
    "5 - See enemies only, short text, no color",

    "6 - Disabled"};

static const char *const c_desc[] = {
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

  if (!common_checks(player, mech, MECH_USUALSM))
    return;
  while (buffer && *buffer && isspace((unsigned char)*buffer))
    buffer++;
  if (!buffer || !*buffer) {
    show_brief_flags(player, mech);
    return;
  }
  c = *buffer;
  buffer++;
  while (buffer && *buffer && isspace((unsigned char)*buffer))
    buffer++;
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Argument missing!");
    return;
  }
  if ((!((v) = atoi(buffer)) && strcmp((buffer), "0"))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number!");
    return;
  }
  switch (toupper(c)) {
#ifdef ADVANCED_LOS
  case 'A':
    if (v < 0 || v > 6) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Number out of range!");
      return;
    }
    v = BOUNDED(0, v, 6);
    mech_brief_mode_set(mech, mech_brief_mode(mech) % 4 + v * 4);
    mech_printf(mech, MECHALL, "Autocontact brevity set to %s.", ac_desc[v]);
    return;
#endif
  case 'C':
    if (v < 0 || v > 3) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Number out of range!");
      return;
    }
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

char mech_contact_weapon_arc(int arc) {
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

  const MechConditionSummary condition = mech_condition_summary(target);

  if (mech_is_destroyed(target))
    statusstr[sptr++] = 'D';

  if (mech_event_count(target, EVENT_STARTUP))
    statusstr[sptr++] = 's';
  else if (!mech_is_started(target))
    statusstr[sptr++] = 'S';

  if (mech_event_count(target, EVENT_STAND))
    statusstr[sptr++] = 'f';
  else if (mech_is_fallen(target))
    statusstr[sptr++] = 'F';

  if (mech_event_count(target, EVENT_CHANGING_HULLDOWN))
    statusstr[sptr++] = 'h';
  else if (condition.hull_down)
    statusstr[sptr++] = 'H';

  if (mech_is_towed(target))
    statusstr[sptr++] = 'T';
  else if (mech_carried_dbref(target) > 0)
    statusstr[sptr++] = 't';

  if (mech_is_jumping(target))
    statusstr[sptr++] = 'J';

  if (mech_is_out_of_control(target))
    statusstr[sptr++] = 'O';

  if (mech_excess_heat(target) != 0.0F)
    statusstr[sptr++] = '+';

  if (mech_is_jellied(target))
    statusstr[sptr++] = 'I';

  if (mech_event_count(target, EVENT_VEHICLEBURN))
    statusstr[sptr++] = 'B';

  if (mech_searchlight_active(target))
    statusstr[sptr++] = 'L';

  if (condition.illuminated)
    statusstr[sptr++] = 'l';

  if (condition.swarm_target > 0)
    statusstr[sptr++] = 'W';

  if (mech_contact_carries_club(target))
    statusstr[sptr++] = 'C';

  if (mech_has_attached_homing_beacon(target)) {
    if (who == 1)
      statusstr[sptr++] = 'N';
    else
      statusstr[sptr++] = 'n';
  }
#ifndef ECM_ON_CONTACTS
  if (who > 1) {
#endif
    if (condition.eccm_enabled || condition.angel_eccm_enabled)
      statusstr[sptr++] = 'P';

    if (condition.ecm_active || condition.angel_ecm_active)
      statusstr[sptr++] = 'E';

    if (condition.ecm_protected || condition.angel_ecm_protected)
      statusstr[sptr++] = 'p';

    if (mech_is_any_ecm_disturbed(target))
      statusstr[sptr++] = 'e';
#ifndef ECM_ON_CONTACTS
  }
#endif

  if (condition.spinning)
    statusstr[sptr++] = 'X';

#ifdef BT_MOVEMENT_MODES
  if (condition.sprinting)
    statusstr[sptr++] = 'M';
  if (condition.evading)
    statusstr[sptr++] = 'm';
#endif

  statusstr[sptr] = '\0';
  return status;
}

char mech_contact_status_character(Mech *mech, Mech *mechTarget, int wCharNum) {
  char cRet = ' ';
  const MechConditionSummary condition = mech_condition_summary(mechTarget);

  switch (wCharNum) {
  case 1:
    cRet = condition.swarm_target > 0              ? 'W'
           : mech_is_towed(mechTarget)             ? 'T'
           : mech_carried_dbref(mechTarget) > 0    ? 't'
           : mech_contact_carries_club(mechTarget) ? 'C'
           :
#ifdef BT_MOVEMENT_MODES
           condition.sprinting ? 'M'
           : condition.evading ? 'm'
                               :
#endif
                               ' ';
    break;
  case 2:
    cRet = mech_is_destroyed(mechTarget)         ? 'D'
           : mech_searchlight_active(mechTarget) ? 'L'
           : condition.illuminated               ? 'l'
                                                 : ' ';
    break;
  case 3:
    cRet = mech_is_jumping(mechTarget)                             ? 'J'
           : mech_is_out_of_control(mechTarget)                    ? 'O'
           : mech_is_fallen(mechTarget)                            ? 'F'
           : mech_event_count(mechTarget, EVENT_STAND)             ? 'f'
           : mech_event_count(mechTarget, EVENT_CHANGING_HULLDOWN) ? 'h'
           : condition.hull_down                                   ? 'H'
           : mech_condition_summary(mech).spinning                 ? 'X'
                                                                   : ' ';
    break;
  case 4:
    cRet = mech_is_started(mechTarget)
               ? (mech_excess_heat(mechTarget) != 0.0F              ? '+'
                  : mech_is_jellied(mechTarget)                     ? 'I'
                  : mech_event_count(mechTarget, EVENT_VEHICLEBURN) ? 'B'
                                                                    : ' ')
           : condition.staggering                        ? 'G'
           : mech_event_count(mechTarget, EVENT_STARTUP) ? 's'
                                                         : 'S';
    break;
  case 5:
    cRet = mech_has_attached_homing_beacon(mechTarget)
               ? (mech_team(mechTarget) == mech_team(mech) ? 'n' : 'N')
           :
#ifdef ECM_ON_CONTACTS
           (condition.eccm_enabled || condition.angel_eccm_enabled)     ? 'P'
           : (condition.ecm_active || condition.angel_ecm_active)       ? 'E'
           : (condition.ecm_protected || condition.angel_ecm_protected) ? 'p'
           : mech_is_any_ecm_disturbed(mechTarget)                      ? 'e'
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
  char *args[1], bufflist[BATTLE_MAP_UNIT_CAPACITY][120], buff[100];
  int sbuff[BATTLE_MAP_UNIT_CAPACITY];
  float range, rangelist[BATTLE_MAP_UNIT_CAPACITY], fx, fy;
  char weaponarc;
  const char *mech_name;
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

  if (!common_checks(player, mech, MECH_USUAL))
    return;
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
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Line of Sight Contacts:");

  for (loop = 0; loop < battle_map_unit_count(mech_map); loop++) {
    const DbRef contact_dbref = battle_map_unit_dbref(mech_map, loop);
    if (!(contact_dbref != mech_dbref(mech) && contact_dbref != -1))
      continue;

    tempMech = btech_context_get_mech(mech_context(mech), contact_dbref);

    if (!tempMech)
      continue;
    if (argc) {
      if (!((mech_contact_is_friend(mech, tempMech) ? (see_what & SEE_ALLY)
                                                    : (see_what & SEE_ENEMA)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == mech_target_dbref(mech)))))
        continue;
      if (!(((see_what & SEE_SHUTDOWN) || mech_is_started(tempMech)) ||
            mech_is_destroyed(tempMech) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == mech_target_dbref(mech)))))
        continue;
      if (!(((see_what & SEE_DEAD) || !mech_is_destroyed(tempMech)) ||
            ((see_what & SEE_TARGET) &&
             (mech_dbref(tempMech) == mech_target_dbref(mech)))))
        continue;
    }
    range = mech_range_to(mech, tempMech);
    if (!(losflag = mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                   mech_position_y(tempMech), range)))
      continue;
    if (is_good_obj(mech_context(mech)->database, mech_dbref(tempMech))) {
      if (!mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), 0.0)) {
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
    bearing = FindBearing(
        mech_position_real_x(mech), mech_position_real_y(mech),
        mech_position_real_x(tempMech), mech_position_real_y(tempMech));
    weaponarc = mech_contact_weapon_arc(InWeaponArc(
        mech, mech_position_real_x(tempMech), mech_position_real_y(tempMech)));

    strcpy(move_type, GetMoveTypeID(mech_movement_type(tempMech)));

    if (isvb) {
      if (!inlos) {
        cStatus1 = ' ';
        cStatus2 = ' ';
        cStatus3 = ' ';
        cStatus4 = ' ';
        cStatus5 = ' ';
      } else {
        cStatus1 = mech_contact_status_character(mech, tempMech, 1);
        cStatus2 = mech_contact_status_character(mech, tempMech, 2);
        cStatus3 = mech_contact_status_character(mech, tempMech, 3);
        cStatus4 = mech_contact_status_character(mech, tempMech, 4);
        cStatus5 = mech_contact_status_character(mech, tempMech, 5);
      }

      snprintf(buff, sizeof(buff),
               "%s%c%c%c[%s]%c %-12.12s x:%3d y:%3d z:%3d r:%4.1f b:%3d "
               "s:%5.1f h:%3d S:%c%c%c%c%c%s",
               mech_dbref(tempMech) == mech_target_dbref(mech) ? "[fg=red bold]"
               : !mech_contact_is_friend(mech, tempMech) ? "[fg=yellow bold]"
                                                         : "",
               (losflag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
               (losflag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
               mech_id(tempMech, mech_contact_is_friend(mech, tempMech)).text,
               move_type[0], mech_name, mech_position_x(tempMech),
               mech_position_y(tempMech), mech_position_z(tempMech),
               (double)range, bearing, (double)mech_current_speed(tempMech),
               mech_contact_heading(tempMech), cStatus1, cStatus2, cStatus3,
               cStatus4, cStatus5,
               (mech_dbref(tempMech) == mech_target_dbref(mech) ||
                !mech_contact_is_friend(mech, tempMech))
                   ? "[reset]"
                   : "");

      rangelist[buffindex] = range;
      rangelist[buffindex] += mech_is_destroyed(tempMech) ? 10000 : 0;
      strcpy(bufflist[buffindex++], buff);
    } else {
      snprintf(buff, sizeof(buff), "[%s] %-17s  Tonnage: %d",
               mech_id(tempMech, mech_contact_is_friend(mech, tempMech)).text,
               mech_name, mech_tonnage(tempMech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Range: %.1f hex\tBearing: %d degrees",
               (double)range, bearing);
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Speed: %.1f KPH\tHeading: %d degrees",
               (double)mech_current_speed(tempMech),
               mech_contact_heading(tempMech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      X, Y: %3d, %3d \tHeat: %.0f deg C.",
               mech_position_x(tempMech), mech_position_y(tempMech),
               (double)mech_excess_heat(tempMech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      snprintf(buff, sizeof(buff), "      Movement Type: %s", move_type);
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
      notify_printf(
          btech_context_evaluation(mech_context(mech)), player,
          "      Mech is in %s Arc",
          GetArcID(mech, InWeaponArc(mech, mech_position_real_x(tempMech),
                                     mech_position_real_y(tempMech))));
      if (mech_is_destroyed(tempMech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech Destroyed");
      if (!mech_is_started(tempMech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech Shutdown");
      if (mech_is_fallen(tempMech))
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "      Mech has Fallen!");
      if (mech_is_jumping(tempMech))
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      "      Mech is Jumping!\tJump Heading: %d",
                      mech_jump_heading_degrees(tempMech));
      mecha_notify(btech_context_evaluation(mech_context(mech)), player, " ");
    }
  }

  if (see_what & SEE_BUILDINGS) {
    for (building =
             battle_map_object_first(mech_map, BATTLE_MAP_OBJECT_BUILDING);
         building; building = battle_map_object_next(building)) {
      const int building_x = battle_map_object_x(building);
      const int building_y = battle_map_object_y(building);
      const DbRef building_dbref = battle_map_object_dbref(building);

      MapCoordToRealCoord(building_x, building_y, &fx, &fy);
      const int building_elevation =
          battle_map_hex_elevation(mech_map, building_x, building_y);
      i = building_elevation + 1;
      const float building_real_z = ZSCALE * (float)i;
      range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                        mech_position_real_z(mech), fx, fy, building_real_z);

      losflag = mech_los_check(mech, nullptr, building_x, building_y, range);
      if (!losflag || (losflag & BATTLE_MAP_LOS_BLOCKED))
        continue;

      if (!(building_dbref && (tmp_map = btech_context_get_map(
                                   mech_context(mech), building_dbref))))
        continue;
      if (battle_map_building_is_invisible(tmp_map))
        continue;
      if ((j = !lock_test(btech_context_evaluation(mech_context(mech)), player,
                          player, mech_dbref(mech), battle_map_dbref(tmp_map),
                          LUA_LOCK_ENTER, LUA_LOCK_OPERATION_BTECH_CONTACT,
                          true, &lock, &lock_result)) &&
          battle_map_building_is_hidden(tmp_map))
        continue;
      bearing = FindBearing(mech_position_real_x(mech),
                            mech_position_real_y(mech), fx, fy);
      weaponarc = mech_contact_weapon_arc(InWeaponArc(mech, fx, fy));

      mech_name =
          btech_attribute_read(mech_context(mech)->database, building_dbref,
                               A_MECHNAME, (char[LBUF_SIZE]){0});
      if (!mech_name || !*mech_name) {
        strncpy(new,
                game_object_name(mech_context(mech)->database, building_dbref),
                LBUF_SIZE - 1);
        styled_text_strip(
            mech_context(mech)->database->styled_text_palette,
            game_object_name(mech_context(mech)->database, building_dbref), new,
            sizeof(new));
        mech_name = new;
      }

      snprintf(buff, sizeof(buff),
               "%s%c%c%c %-23.23s x:%3d y:%3d z:%2d r:%4.1f b:%3d CF:%4d /%4d "
               "S:%c%c%s",
               j ? "[fg=yellow bold]" : "",
               (losflag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
               (losflag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
               mech_name, building_x, building_y, i, (double)range, bearing,
               battle_map_building_integrity(tmp_map),
               battle_map_building_maximum_integrity(tmp_map),
               (battle_map_building_is_safe(tmp_map) ||
                (j && battle_map_building_is_command_center(tmp_map)))
                   ? 'X'
               : j                                              ? 'x'
               : battle_map_building_is_command_center(tmp_map) ? 'C'
                                                                : ' ',
               battle_map_building_is_hidden(tmp_map) ? 'H' : ' ',
               j ? "[reset]" : "");
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
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   bufflist[sbuff[loop]]);
  }

  if (isvb <= 2)
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "End Contact List");
}

#undef SEE_DEAD
#undef SEE_SHUTDOWN
#undef SEE_ALLY
#undef SEE_ENEMA
#undef SEE_TARGET
#undef SEE_BUILDINGS
#undef SEE_NEGNEXT
