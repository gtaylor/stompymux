#include "autopilot.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_scan_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/network/network_output.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum {
  SHOW_INFO = 1,
  SHOW_ARMOR = 2,
  SHOW_WEAPONS = 4,
};

void mech_scan(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int mapx = 0, mapy = 0;
  char targetID[2];
  DbRef target;
  int numargs;
  Mech *tempMech = NULL;
  float fx, fy, fz = 0.0;
  float range = 0.0, enemyX, enemyY, enemyZ;
  int dob = 0, doh = 0;
  int options = SHOW_INFO | SHOW_ARMOR | SHOW_WEAPONS;

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  cch(MECH_USUAL);
  numargs = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(mech_context(mech), numargs > 3,
                  "Wrong number of arguments to scan!");
  DOCHECK_CONTEXT(mech_context(mech), !mech_scanner_range(mech),
                  "Your system seems to be inoperational.");
  switch (numargs) {
  case 1:
    /* Scan Target */
    targetID[0] = args[0][0];
    if (args[0][1]) {
      targetID[1] = args[0][1];
      target = FindTargetDBREFFromMapNumber(mech, targetID);
      tempMech = btech_context_get_mech(mech_context(mech), target);
      DOCHECK_CONTEXT(mech_context(mech), !tempMech,
                      "Target is not in line of sight!");
      range = mech_range_to(mech, tempMech);
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                      mech_position_y(tempMech), range),
                      "Target is not in line of sight!");
      DOCHECK_CONTEXT(
          mech_context(mech),
          !mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
          "That target isn't seen well enough by the scanners for scanning!");
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_is_observer(mech) &&
                          (int)range > mech_scanner_range(mech),
                      "Target is out of scanner range.");
      break;
    } else { /* Default target */
      switch (toupper(args[0][0])) {
      case 'A':
        options = SHOW_ARMOR;
        break;
      case 'I':
        options = SHOW_INFO;
        break;
      case 'W':
        options = SHOW_WEAPONS;
        break;
      default:
        notify(evaluation, player, "Truly odd option!");
        return;
      }
    }
    [[fallthrough]];
  case 0:
    /* scan current target... */
    target = mech_target_dbref(mech);
    tempMech = btech_context_get_mech(mech_context(mech), target);
    if (tempMech) {
      range = mech_range_to(mech, tempMech);
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_is_observer(mech) &&
                          (int)range > mech_scanner_range(mech),
                      "Target is out of scanner range.");
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                      mech_position_y(tempMech), range),
                      "Target is not in line of sight!");
      DOCHECK_CONTEXT(
          mech_context(mech),
          !mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
          "That target isn't seen well enough by the scanners for scanning!");
    } else {
      if (!mech_targets_building(mech))
        DOCHECK_CONTEXT(mech_context(mech),
                        !FindTargetXY(mech, &enemyX, &enemyY, &enemyZ),
                        "No default target set!");
      mapx = mech_target_hex_x(mech);
      mapy = mech_target_hex_y(mech);
      MapCoordToRealCoord(mapx, mapy, &fx, &fy);
      fz = ZSCALE * battle_map_hex_elevation(mech_map, mapx, mapy);
      range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                        mech_position_real_z(mech), fx, fy, fz);
      DOCHECK_CONTEXT(mech_context(mech),
                      !battle_map_coordinate_is_valid(mech_map, mapx, mapy),
                      "Those coordinates are out of scanner range.");
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_is_observer(mech) &&
                          (int)range > mech_scanner_range(mech),
                      "Those coordinates are out of scanner range.");
      DOCHECK_CONTEXT(
          mech_context(mech),
          !mech_los_check_unblocked(mech, tempMech, mapx, mapy, range),
          "Target hex is not in line of sight!");
      /* look for enemies in that hex... */
      if (mech_targets_building(mech))
        dob = 1;
      else if (mech_targets_hex(mech)) {
        dob = 1;
        doh = 1;
      } else if (!(tempMech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1)))
        tempMech = (Mech *)NULL;
    }
    break;
  case 3:
    /* scan x, y b */
    mapx = atoi(args[0]);
    mapy = atoi(args[1]);
    DOCHECK_CONTEXT(mech_context(mech),
                    !battle_map_coordinate_is_valid(mech_map, mapx, mapy),
                    "Those coordinates are out of scanner range.");
    switch (toupper(args[2][0])) {
    case 'H':
      doh = 1;
      [[fallthrough]];
    case 'B':
      dob = 1;
      break;
    default:
      notify(evaluation, player, "Invalid 3rd argument!");
      return;
    }
    MapCoordToRealCoord(mapx, mapy, &fx, &fy);
    fz = ZSCALE * battle_map_hex_elevation(mech_map, mapx, mapy);
    range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), fx, fy, fz);
    DOCHECK_CONTEXT(mech_context(mech), (int)range > mech_scanner_range(mech),
                    "Those coordinates are out of scanner range.");
    DOCHECK_CONTEXT(mech_context(mech),
                    !mech_los_check(mech, tempMech, mapx, mapy, range),
                    "Coordinates are not in line of sight!");
    break;
  case 2:
    /* scan x, y */
    mapx = atoi(args[0]);
    mapy = atoi(args[1]);
    if (!mapx && strcmp(args[0], "0")) {
      targetID[0] = args[0][0];
      targetID[1] = args[0][1];
      target = FindTargetDBREFFromMapNumber(mech, targetID);
      tempMech = btech_context_get_mech(mech_context(mech), target);
      DOCHECK_CONTEXT(mech_context(mech), !tempMech,
                      "Target is not in line of sight!");
      range = mech_range_to(mech, tempMech);
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                      mech_position_y(tempMech), range),
                      "Target is not in line of sight!");
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_is_observer(mech) &&
                          (int)range > mech_scanner_range(mech),
                      "Target is out of scanner range.");
      switch (toupper(args[1][0])) {
      case 'A':
        options = SHOW_ARMOR;
        break;
      case 'I':
        options = SHOW_INFO;
        break;
      case 'W':
        options = SHOW_WEAPONS;
        break;
      default:
        notify(evaluation, player, "Truly odd option!");
        return;
      }
      break;
    }
    DOCHECK_CONTEXT(mech_context(mech),
                    !battle_map_coordinate_is_valid(mech_map, mapx, mapy),
                    "Those coordinates are out of scanner range.");
    MapCoordToRealCoord(mapx, mapy, &fx, &fy);
    range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), fx, fy, fz);
    DOCHECK_CONTEXT(mech_context(mech),
                    !mech_is_observer(mech) &&
                        (int)range > mech_scanner_range(mech),
                    "Those coordinates are out of scanner range.");
    DOCHECK_CONTEXT(mech_context(mech),
                    !mech_los_check(mech, tempMech, mapx, mapy, range),
                    "Coordinates are not in line of sight!");
    fz = ZSCALE * battle_map_hex_elevation(mech_map, mapx, mapy);
    /* look for enemies in that hex... */
    if (!(tempMech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1)))
      tempMech = (Mech *)NULL;
    break;
  }
  if (tempMech) {
    DOCHECK_CONTEXT(
        mech_context(mech),
        !mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                  mech_position_y(tempMech), range),
        "That target isn't seen well enough by the scanners for report!");
    DOCHECK_CONTEXT(
        mech_context(mech), mech_class(tempMech) == CLASS_MW,
        "Your scanners cannot give you precise information on targets that "
        "small!");
    mech_scan_print_enemy_status(evaluation, player, mech, tempMech, range,
                                 options);
    if (!mech_is_observer(mech)) {
      mech_printf(tempMech, MECHSTARTED, "You are being scanned by %s",
                  mech_to_mech_display_id(tempMech, mech).text);
      auto_reply(tempMech,
                 tprintf("%s just scanned me.",
                         mech_to_mech_display_id(tempMech, mech).text));
    }
    return;
  }
  if (!dob && !doh) {
    notify(evaluation, player, "You see nobody in the hex!");
    return;
  }
  if (dob)
    show_building_in_hex(mech, mapx, mapy);
  if (doh)
    show_mines_in_hex(player, mech, range, mapx, mapy);
}

void mech_report(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[3];
  int mapx = 0, mapy = 0;
  char targetID[2];
  DbRef target;
  int numargs;
  Mech *tempMech = NULL;
  float fx, fy, fz = 0.0;
  float range = 0.0, enemyX, enemyY, enemyZ;

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  cch(MECH_USUAL);
  numargs = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(mech_context(mech), numargs > 2,
                  "Wrong number of arguments to report!");
  DOCHECK_CONTEXT(mech_context(mech), !mech_scanner_range(mech),
                  "Your system seems to be inoperational.");
  switch (numargs) {
  case 1:
    /* Scan Target */
    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    target = FindTargetDBREFFromMapNumber(mech, targetID);
    tempMech = btech_context_get_mech(mech_context(mech), target);
    DOCHECK_CONTEXT(mech_context(mech), !tempMech,
                    "Target is not in line of sight!");
    range = mech_range_to(mech, tempMech);
    DOCHECK_CONTEXT(mech_context(mech),
                    !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
                    "Target is not in line of sight!");
    DOCHECK_CONTEXT(
        mech_context(mech),
        !mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                  mech_position_y(tempMech), range),
        "That target isn't seen well enough by the scanners for a report!");
    break;
  case 2:
    /* report x, y */
    mapx = atoi(args[0]);
    mapy = atoi(args[1]);
    MapCoordToRealCoord(mapx, mapy, &fx, &fy);
    range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                      mech_position_real_z(mech), fx, fy, fz);
    DOCHECK_CONTEXT(mech_context(mech),
                    !battle_map_coordinate_is_valid(mech_map, mapx, mapy),
                    "Those coordinates are out of scanner range.");
    DOCHECK_CONTEXT(mech_context(mech), (int)range > mech_scanner_range(mech),
                    "Those coordinates are out of scanner range.");
    DOCHECK_CONTEXT(mech_context(mech),
                    !mech_los_check(mech, tempMech, mapx, mapy, range),
                    "Coordinates are not in line of sight!");
    DOCHECK_CONTEXT(
        mech_context(mech),
        !mech_los_check_unblocked(mech, tempMech, mapx, mapy, range),
        "That target isn't seen well enough by the scanners for a report!");
    fz = ZSCALE * battle_map_hex_elevation(mech_map, mapx, mapy);
    /* look for enemies in that hex... */
    tempMech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
    DOCHECK_CONTEXT(mech_context(mech), !tempMech, "No target found.");
    break;
  case 0:
    /* report current target... */
    target = mech_target_dbref(mech);
    tempMech = btech_context_get_mech(mech_context(mech), target);
    if (tempMech) {
      range = mech_range_to(mech, tempMech);
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                      mech_position_y(tempMech), range),
                      "Target is not in line of sight!");
      DOCHECK_CONTEXT(
          mech_context(mech),
          !mech_los_check_unblocked(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
          "That target isn't seen well enough by the scanners for a report!");
    } else {
      DOCHECK_CONTEXT(mech_context(mech),
                      !FindTargetXY(mech, &enemyX, &enemyY, &enemyZ),
                      "No default target set!");
      /* look for enemies in that hex... */
      tempMech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
      DOCHECK_CONTEXT(mech_context(mech), !tempMech, "You don't see a thing.");
      DOCHECK_CONTEXT(mech_context(mech),
                      !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                      mech_position_y(tempMech), range),
                      "You don't see a thing.");
    }
  }
  if (tempMech)
    mech_scan_print_report(evaluation, player, mech, tempMech, range);
}

void mech_scan_show_turret_facing(EvaluationContext *evaluation, DbRef player,
                                  int spaces, Mech *mech) {
  int i;
  int j;
  char buff[MBUF_SIZE] = {0};

  if (mech_section_internal(mech, TURRET) &&
      !(mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_BSUIT ||
        mech_class(mech) == CLASS_MW) &&
      !mech_is_aerospace_unit(mech)) {
    i = AcceptableDegree(mech_turret_heading_degrees(mech));
    if (i > 180)
      i -= 360;
    j = AcceptableDegree(mech_turret_heading_degrees(mech) +
                         mech_heading_degrees(mech));
    if (mech_movement_type(mech) != MOVE_NONE)
      snprintf(buff, sizeof(buff), "      Turret Facing: %d degrees%s", j,
               i ? tprintf(" (%d offset from heading)", i) : "");
    else
      snprintf(buff, sizeof(buff), "      Turret Facing: %d degrees", j);
    notify(evaluation, player, buff);
  }
}

void mech_scan_print_report(EvaluationContext *evaluation, DbRef player,
                            Mech *mech, Mech *tempMech, float range) {
  int bearing;
  char buff[100] = {0};
  int weaponarc;
  char *mech_name;

  mech_name = btech_attribute_read(
      btech_context_database(mech_context(tempMech)), mech_dbref(tempMech),
      A_MECHNAME, (char[LBUF_SIZE]){0});
  snprintf(
      buff, sizeof(buff), "[%s]  %-25.25s Tonnage: %d",
      mech_id(tempMech, mech_team(mech) == mech_team(tempMech) &&
                            mech_los_check_unblocked(mech, tempMech, 0, 0, 0))
          .text,
      mech_name, mech_tonnage(tempMech));
  notify(evaluation, player, buff);
  bearing = FindBearing(mech_position_real_x(mech), mech_position_real_y(mech),
                        mech_position_real_x(tempMech),
                        mech_position_real_y(tempMech));
  snprintf(buff, sizeof(buff), "      Range: %.1f hex\t\tBearing: %d degrees",
           range, bearing);
  notify(evaluation, player, buff);
  snprintf(buff, sizeof(buff), "      Speed: %.1f KPH\t\tHeading: %d degrees",
           mech_current_speed(tempMech),
           AcceptableDegree(mech_heading_degrees(tempMech) +
                            mech_lateral_movement(tempMech)));
  notify(evaluation, player, buff);
  if (mech_is_flying_type(tempMech))
    notify_printf(evaluation, player, "      Vertical speed: %.1f KPH",
                  mech_vertical_speed(tempMech));
  snprintf(buff, sizeof(buff),
           "      X, Y, Z: %3d, %3d, %3d\tHeat: %.0f deg C.",
           mech_position_x(tempMech), mech_position_y(tempMech),
           mech_position_z(tempMech), 10. * mech_excess_heat(tempMech));
  notify(evaluation, player, buff);
  if (mech_lateral_movement(tempMech))
    notify_printf(evaluation, player, "      Mech is moving laterally %s",
                  mech_lateral_description(tempMech));
  mech_scan_show_turret_facing(evaluation, player, 6, tempMech);

  switch (mech_movement_type(tempMech)) {
  case MOVE_NONE:
    notify(evaluation, player, "      Type: INSTALLATION");
    break;
  case MOVE_BIPED:
    switch (mech_class(tempMech)) {
    case CLASS_MW:
      notify(evaluation, player,
             "      Type: MECHWARRIOR         Movement: BIPED");
      break;
    case CLASS_MECH:
      notify(evaluation, player,
             "      Type: MECH                Movement: BIPED");
      break;
    case CLASS_BSUIT:
      notify(evaluation, player,
             "      Type: BATTLESUIT(S)       Movement: BIPED");
    }
    break;
  case MOVE_QUAD:
    notify(evaluation, player,
           "      Type: MECH                Movement: QUAD");
    break;
  case MOVE_TRACK:
    notify(evaluation, player,
           "      Type: VEHICLE             Movement: TRACKED");
    break;
  case MOVE_WHEEL:
    notify(evaluation, player,
           "      Type: VEHICLE             Movement: WHEELED");
    break;
  case MOVE_HOVER:
    notify(evaluation, player,
           "      Type: VEHICLE             Movement: HOVER");
    break;
  case MOVE_VTOL:
    notify(evaluation, player,
           "      Type: VTOL                Movement: VTOL");
    break;
  case MOVE_FLY:
    notify_printf(
        evaluation, player, "      Type: %-9s             Movement: FLIGHT",
        mech_class(tempMech) == CLASS_AERO ? "AEROSPACE" : "DROPSHIP");
    break;
  case MOVE_HULL:
    notify(evaluation, player,
           "      Type: NAVAL               Movement: HULL");
    break;
  case MOVE_SUB:
    notify(evaluation, player,
           "      Type: NAVAL               Movement: SUBMARINE");
    break;
  case MOVE_FOIL:
    notify(evaluation, player,
           "      Type: NAVAL               Movement: HYDROFOIL");
    break;
  }

  weaponarc = InWeaponArc(mech, mech_position_real_x(tempMech),
                          mech_position_real_y(tempMech));
  if (weaponarc & TURRETARC) {
    notify(evaluation, player, "      In Turret Arc");
    weaponarc &= ~TURRETARC;
  }
  notify_printf(evaluation, player, "      In %s Weapons Arc",
                GetArcID(mech, weaponarc));
  Mech_ShowFlags(evaluation, player, tempMech, 6, 1);
  if (mech_is_jumping(tempMech))
    notify_printf(evaluation, player,
                  "      Mech is Jumping!\tJump Heading: %d",
                  mech_jump_heading_degrees(tempMech));
  notify(evaluation, player, " ");
}

void mech_scan_print_enemy_status(EvaluationContext *evaluation, DbRef player,
                                  Mech *mymech, Mech *mech, float range,
                                  int opt) {
  Mech *tempMech;
  int owner = 0;

  if (mech_is_observer(mymech))
    owner = 1;
  mech_scan_print_report(evaluation, player, mymech, mech, range);
  if (opt & SHOW_ARMOR)
    PrintArmorStatus(evaluation, player, mech, owner);
  if (opt & SHOW_INFO) {
    if (mech_condition_summary(mech).torso_right)
      notify(evaluation, player, "Torso is 60 degrees right");
    if (mech_condition_summary(mech).torso_left)
      notify(evaluation, player, "Torso is 60 degrees left");
    if (mech_carried_dbref(mech) > 0)
      if ((tempMech = btech_context_get_mech(mech_context(mech),
                                             mech_carried_dbref(mech))))
        notify_printf(evaluation, player, "Towing %s.",
                      mech_to_mech_display_id(mech, tempMech).text);
    notify(evaluation, player, " ");
  }
  if (opt & SHOW_WEAPONS) {
    if (owner)
      PrintWeaponStatus(evaluation, mech, player);
    else
      PrintEnemyWeaponStatus(mech, player);
  }
}
