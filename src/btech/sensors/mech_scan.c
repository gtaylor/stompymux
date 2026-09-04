#include "autopilot.h"
#include "btech/configuration.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_scan_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/network/network_output.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

#include <stdio.h>

static float mech_scan_hex_real_z(BattleMap *map, int x, int y) {
  const int ELEVATION = battle_map_hex_elevation(map, x, y);
  return ZSCALE * (float)ELEVATION;
}

enum {
  SHOW_INFO = 1,
  SHOW_ARMOR = 2,
  SHOW_WEAPONS = 4,
};

void mech_scan(DbRef player, Mech *mech, char *buffer) {
  char message_buffer[LBUF_SIZE];
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[4];
  int mapx = 0;
  int mapy = 0;
  char target_id[2];
  DbRef target;
  int numargs;
  Mech *temp_mech = nullptr;
  float fx;
  float fy;
  float fz = 0.0;
  float range = 0.0;
  int dob = 0;
  int doh = 0;
  int options = SHOW_INFO | SHOW_ARMOR | SHOW_WEAPONS;

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  numargs = mech_parseattributes(buffer, args, 4);
  if (numargs > 3) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Wrong number of arguments to scan!");
    return;
  }
  if (!mech_scanner_range(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your system seems to be inoperational.");
    return;
  }
  switch (numargs) {
  case 1:
    /* Scan Target */
    target_id[0] = args[0][0];
    if ((*checked_string_suffix(*args, 1))) {
      target_id[1] = (*checked_string_suffix(*args, 1));
      target = find_target_dbref_from_map_number(mech, target_id);
      temp_mech = btech_context_get_mech(mech_context(mech), target);
      if (!temp_mech) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      range = mech_range_to(mech, temp_mech);
      if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                          mech_position_y(temp_mech), range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                    mech_position_y(temp_mech), range)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "That target isn't seen well enough by the scanners for scanning!");
        return;
      }
      if (!mech_is_observer(mech) && (int)range > mech_scanner_range(mech)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is out of scanner range.");
        return;
      }
      break;
    } /* Default target */
    switch (ascii_to_upper(*checked_string_suffix(args[0], 0))) {
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
      mecha_notify(evaluation, player, "Truly odd option!");
      return;
    }

    [[fallthrough]];
  case 0:
    /* scan current target... */
    target = mech_target_dbref(mech);
    temp_mech = btech_context_get_mech(mech_context(mech), target);
    if (temp_mech) {
      range = mech_range_to(mech, temp_mech);
      if (!mech_is_observer(mech) && (int)range > mech_scanner_range(mech)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is out of scanner range.");
        return;
      }
      if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                          mech_position_y(temp_mech), range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                    mech_position_y(temp_mech), range)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "That target isn't seen well enough by the scanners for scanning!");
        return;
      }
    } else {
      if (!mech_targets_building(mech)) {
        const MechTargetPositionResult TARGET_POSITION =
            mech_target_position(mech);
        if (!TARGET_POSITION.found) {
          mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                       "No default target set!");
          return;
        }
      }
      mapx = mech_target_hex_x(mech);
      mapy = mech_target_hex_y(mech);
      map_coord_to_real_coord(mapx, mapy, &fx, &fy);
      fz = mech_scan_hex_real_z(mech_map, mapx, mapy);
      range = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(mech),
                    .y = mech_position_real_y(mech),
                    .z = mech_position_real_z(mech)},
          .end = {.x = fx, .y = fy, .z = fz},
      });
      if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Those coordinates are out of scanner range.");
        return;
      }
      if (!mech_is_observer(mech) && (int)range > mech_scanner_range(mech)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Those coordinates are out of scanner range.");
        return;
      }
      if (!mech_los_check_unblocked(mech, temp_mech, mapx, mapy, range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target hex is not in line of sight!");
        return;
      }
      /* look for enemies in that hex... */
      if (mech_targets_building(mech)) {
        dob = 1;
      } else if (mech_targets_hex(mech)) {
        dob = 1;
        doh = 1;
      } else {
        temp_mech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
      }
    }
    break;
  case 3:
    /* scan x, y b */
    if (!parse_int_checked(args[0], &mapx) ||
        !parse_int_checked(args[1], &mapy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    switch (ascii_to_upper(*checked_string_suffix(args[2], 0))) {
    case 'H':
      doh = 1;
      [[fallthrough]];
    case 'B':
      dob = 1;
      break;
    default:
      mecha_notify(evaluation, player, "Invalid 3rd argument!");
      return;
    }
    map_coord_to_real_coord(mapx, mapy, &fx, &fy);
    fz = mech_scan_hex_real_z(mech_map, mapx, mapy);
    range = map_spatial_range(&(MapSpatialSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech),
                  .z = mech_position_real_z(mech)},
        .end = {.x = fx, .y = fy, .z = fz},
    });
    if ((int)range > mech_scanner_range(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    if (!mech_los_check(mech, temp_mech, mapx, mapy, range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Coordinates are not in line of sight!");
      return;
    }
    break;
  case 2:
    /* scan x, y */
    if (!parse_int_checked(args[0], &mapx)) {
      target_id[0] = args[0][0];
      target_id[1] = (*checked_string_suffix(*args, 1));
      target = find_target_dbref_from_map_number(mech, target_id);
      temp_mech = btech_context_get_mech(mech_context(mech), target);
      if (!temp_mech) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      range = mech_range_to(mech, temp_mech);
      if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                          mech_position_y(temp_mech), range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      if (!mech_is_observer(mech) && (int)range > mech_scanner_range(mech)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is out of scanner range.");
        return;
      }
      switch (ascii_to_upper(*checked_string_suffix(args[1], 0))) {
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
        mecha_notify(evaluation, player, "Truly odd option!");
        return;
      }
      break;
    }
    if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    map_coord_to_real_coord(mapx, mapy, &fx, &fy);
    range = map_spatial_range(&(MapSpatialSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech),
                  .z = mech_position_real_z(mech)},
        .end = {.x = fx, .y = fy, .z = fz},
    });
    if (!mech_is_observer(mech) && (int)range > mech_scanner_range(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    if (!mech_los_check(mech, temp_mech, mapx, mapy, range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Coordinates are not in line of sight!");
      return;
    }
    /* look for enemies in that hex... */
    temp_mech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
    break;
  }
  if (temp_mech) {
    if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                  mech_position_y(temp_mech), range)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "That target isn't seen well enough by the scanners for report!");
      return;
    }
    if (mech_class(temp_mech) == CLASS_MW) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "Your scanners cannot give you precise information on targets that "
          "small!");
      return;
    }
    mech_scan_print_enemy_status(&(ScanEnemyStatusRequest){
        .evaluation = evaluation,
        .player = player,
        .observer = mech,
        .target = temp_mech,
        .range = range,
        .options = options,
    });
    if (!mech_is_observer(mech)) {
      mech_printf(temp_mech, MECHSTARTED, "You are being scanned by %s",
                  mech_to_mech_display_id(temp_mech, mech).text);
      (void)snprintf(message_buffer, sizeof(message_buffer),
                     "%s just scanned me.",
                     mech_to_mech_display_id(temp_mech, mech).text);
      auto_reply(temp_mech, message_buffer);
    }
    return;
  }
  if (!dob && !doh) {
    mecha_notify(evaluation, player, "You see nobody in the hex!");
    return;
  }
  if (dob)
    show_building_in_hex(mech, mapx, mapy);
  if (doh) {
    mine_field_scan(
        &(MineFieldScanRequest){.player = player,
                                .mech = mech,
                                .range = range,
                                .position = {.x = mapx, .y = mapy}});
  }
}

void mech_report(DbRef player, Mech *mech, char *buffer) {
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  BattleMap *mech_map;
  char *args[3];
  int mapx = 0;
  int mapy = 0;
  char target_id[2];
  DbRef target;
  int numargs;
  Mech *temp_mech = nullptr;
  float fx;
  float fy;
  float fz = 0.0;
  float range = 0.0;

  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!common_checks(player, mech, MECH_USUAL))
    return;
  numargs = mech_parseattributes(buffer, args, 3);
  if (numargs > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Wrong number of arguments to report!");
    return;
  }
  if (!mech_scanner_range(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your system seems to be inoperational.");
    return;
  }
  switch (numargs) {
  case 1:
    /* Scan Target */
    target_id[0] = args[0][0];
    target_id[1] = (*checked_string_suffix(*args, 1));
    target = find_target_dbref_from_map_number(mech, target_id);
    temp_mech = btech_context_get_mech(mech_context(mech), target);
    if (!temp_mech) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is not in line of sight!");
      return;
    }
    range = mech_range_to(mech, temp_mech);
    if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                        mech_position_y(temp_mech), range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Target is not in line of sight!");
      return;
    }
    if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                  mech_position_y(temp_mech), range)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "That target isn't seen well enough by the scanners for a report!");
      return;
    }
    break;
  case 2:
    /* report x, y */
    if (!parse_int_checked(args[0], &mapx) ||
        !parse_int_checked(args[1], &mapy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    map_coord_to_real_coord(mapx, mapy, &fx, &fy);
    range = map_spatial_range(&(MapSpatialSegment){
        .start = {.x = mech_position_real_x(mech),
                  .y = mech_position_real_y(mech),
                  .z = mech_position_real_z(mech)},
        .end = {.x = fx, .y = fy, .z = fz},
    });
    if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    if ((int)range > mech_scanner_range(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Those coordinates are out of scanner range.");
      return;
    }
    if (!mech_los_check(mech, temp_mech, mapx, mapy, range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Coordinates are not in line of sight!");
      return;
    }
    if (!mech_los_check_unblocked(mech, temp_mech, mapx, mapy, range)) {
      mecha_notify(
          btech_context_evaluation(mech_context(mech)), player,
          "That target isn't seen well enough by the scanners for a report!");
      return;
    }
    /* look for enemies in that hex... */
    temp_mech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
    if (!temp_mech) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "No target found.");
      return;
    }
    break;
  case 0:
    /* report current target... */
    target = mech_target_dbref(mech);
    temp_mech = btech_context_get_mech(mech_context(mech), target);
    if (temp_mech) {
      range = mech_range_to(mech, temp_mech);
      if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                          mech_position_y(temp_mech), range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "Target is not in line of sight!");
        return;
      }
      if (!mech_los_check_unblocked(mech, temp_mech, mech_position_x(temp_mech),
                                    mech_position_y(temp_mech), range)) {
        mecha_notify(
            btech_context_evaluation(mech_context(mech)), player,
            "That target isn't seen well enough by the scanners for a report!");
        return;
      }
    } else {
      const MechTargetPositionResult TARGET_POSITION =
          mech_target_position(mech);
      if (!TARGET_POSITION.found) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "No default target set!");
        return;
      }
      /* look for enemies in that hex... */
      temp_mech = find_mech_in_hex(mech, mech_map, mapx, mapy, 1);
      if (!temp_mech) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "You don't see a thing.");
        return;
      }
      if (!mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                          mech_position_y(temp_mech), range)) {
        mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                     "You don't see a thing.");
        return;
      }
    }
  }
  if (temp_mech)
    mech_scan_print_report(evaluation, player, mech, temp_mech, range);
}

void mech_scan_show_turret_facing(EvaluationContext *evaluation, DbRef player,
                                  Mech *mech) {
  char message_buffer[128];
  int i;
  int j;
  char buff[MBUF_SIZE] = {0};

  if (mech_section_internal(mech, TURRET) &&
      !(mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_BSUIT ||
        mech_class(mech) == CLASS_MW) &&
      !mech_is_aerospace_unit(mech)) {
    i = acceptable_degree(mech_turret_heading_degrees(mech));
    if (i > 180)
      i -= 360;
    j = acceptable_degree(mech_turret_heading_degrees(mech) +
                          mech_heading_degrees(mech));
    if (mech_movement_type(mech) != MOVE_NONE) {
      (void)snprintf(message_buffer, sizeof(message_buffer),
                     " (%d offset from heading)", i);
      (void)snprintf(buff, sizeof(buff), "      Turret Facing: %d degrees%s", j,
                     i ? message_buffer : "");
    } else {
      (void)snprintf(buff, sizeof(buff), "      Turret Facing: %d degrees", j);
    }
    mecha_notify(evaluation, player, buff);
  }
}

void mech_scan_print_report(EvaluationContext *evaluation, DbRef player,
                            Mech *mech, Mech *temp_mech, float range) {
  int bearing;
  char buff[100] = {0};
  int weaponarc;
  const char *mech_name;

  mech_name =
      btech_unit_display_name(mech_context(temp_mech), mech_dbref(temp_mech));
  (void)snprintf(buff, sizeof(buff), "[%s]  %-25.25s Tonnage: %d",
                 mech_id(temp_mech, (mech_team(mech) == mech_team(temp_mech) &&
                                     mech_los_check_unblocked(mech, temp_mech,
                                                              0, 0, 0)) != 0)
                     .text,
                 mech_name, mech_tonnage(temp_mech));
  mecha_notify(evaluation, player, buff);
  bearing = map_bearing(
      &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                  .y = mech_position_real_y(mech)},
                        .end = {.x = mech_position_real_x(temp_mech),
                                .y = mech_position_real_y(temp_mech)}});
  (void)snprintf(buff, sizeof(buff),
                 "      Range: %.1f hex\t\tBearing: %d degrees", (double)range,
                 bearing);
  mecha_notify(evaluation, player, buff);
  (void)snprintf(buff, sizeof(buff),
                 "      Speed: %.1f KPH\t\tHeading: %d degrees",
                 (double)mech_current_speed(temp_mech),
                 acceptable_degree(mech_heading_degrees(temp_mech) +
                                   mech_lateral_movement(temp_mech)));
  mecha_notify(evaluation, player, buff);
  if (mech_is_flying_type(temp_mech))
    notify_printf(evaluation, player, "      Vertical speed: %.1f KPH",
                  (double)mech_vertical_speed(temp_mech));
  (void)snprintf(buff, sizeof(buff),
                 "      X, Y, Z: %3d, %3d, %3d\tHeat: %.0f deg C.",
                 mech_position_x(temp_mech), mech_position_y(temp_mech),
                 mech_position_z(temp_mech),
                 (double)(10.0F * mech_excess_heat(temp_mech)));
  mecha_notify(evaluation, player, buff);
  if (mech_lateral_movement(temp_mech))
    notify_printf(evaluation, player, "      Mech is moving laterally %s",
                  mech_lateral_description(temp_mech));
  mech_scan_show_turret_facing(evaluation, player, temp_mech);

  switch (mech_movement_type(temp_mech)) {
  case MOVE_NONE:
    mecha_notify(evaluation, player, "      Type: INSTALLATION");
    break;
  case MOVE_BIPED:
    switch (mech_class(temp_mech)) {
    case CLASS_MW:
      mecha_notify(evaluation, player,
                   "      Type: MECHWARRIOR         Movement: BIPED");
      break;
    case CLASS_MECH:
      mecha_notify(evaluation, player,
                   "      Type: MECH                Movement: BIPED");
      break;
    case CLASS_BSUIT:
      mecha_notify(evaluation, player,
                   "      Type: BATTLESUIT(S)       Movement: BIPED");
      break;
    case CLASS_VEH_GROUND:
    case CLASS_VTOL:
    case CLASS_VEH_NAVAL:
    case CLASS_SPHEROID_DS:
    case CLASS_AERO:
    case CLASS_DS:
      break;
    default:
      break;
    }
    break;
  case MOVE_QUAD:
    mecha_notify(evaluation, player,
                 "      Type: MECH                Movement: QUAD");
    break;
  case MOVE_TRACK:
    mecha_notify(evaluation, player,
                 "      Type: VEHICLE             Movement: TRACKED");
    break;
  case MOVE_WHEEL:
    mecha_notify(evaluation, player,
                 "      Type: VEHICLE             Movement: WHEELED");
    break;
  case MOVE_HOVER:
    mecha_notify(evaluation, player,
                 "      Type: VEHICLE             Movement: HOVER");
    break;
  case MOVE_VTOL:
    mecha_notify(evaluation, player,
                 "      Type: VTOL                Movement: VTOL");
    break;
  case MOVE_FLY:
    notify_printf(
        evaluation, player, "      Type: %-9s             Movement: FLIGHT",
        mech_class(temp_mech) == CLASS_AERO ? "AEROSPACE" : "DROPSHIP");
    break;
  case MOVE_HULL:
    mecha_notify(evaluation, player,
                 "      Type: NAVAL               Movement: HULL");
    break;
  case MOVE_SUB:
    mecha_notify(evaluation, player,
                 "      Type: NAVAL               Movement: SUBMARINE");
    break;
  case MOVE_FOIL:
    mecha_notify(evaluation, player,
                 "      Type: NAVAL               Movement: HYDROFOIL");
    break;
  }

  weaponarc = in_weapon_arc(mech, mech_position_real_x(temp_mech),
                            mech_position_real_y(temp_mech));
  if (weaponarc & TURRETARC) {
    mecha_notify(evaluation, player, "      In Turret Arc");
    weaponarc &= ~TURRETARC;
  }
  notify_printf(evaluation, player, "      In %s Weapons Arc",
                get_arc_id(mech, weaponarc));
  mech_show_flags(&(MechFlagDisplayRequest){.evaluation = evaluation,
                                            .player = player,
                                            .mech = temp_mech,
                                            .indentation = 6,
                                            .detail_level = 1});
  if (mech_is_jumping(temp_mech))
    notify_printf(evaluation, player,
                  "      Mech is Jumping!\tJump Heading: %d",
                  mech_jump_heading_degrees(temp_mech));
  mecha_notify(evaluation, player, " ");
}

void mech_scan_print_enemy_status(const ScanEnemyStatusRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  const DbRef PLAYER = request->player;
  Mech *mymech = request->observer;
  Mech *mech = request->target;
  const float RANGE = request->range;
  const int OPT = request->options;
  Mech *temp_mech;
  int owner = 0;

  if (mech_is_observer(mymech))
    owner = 1;
  mech_scan_print_report(evaluation, PLAYER, mymech, mech, RANGE);
  if (OPT & SHOW_ARMOR)
    print_armor_status(evaluation, PLAYER, mech, owner);
  if (OPT & SHOW_INFO) {
    if (mech_condition_summary(mech).torso_right)
      mecha_notify(evaluation, PLAYER, "Torso is 60 degrees right");
    if (mech_condition_summary(mech).torso_left)
      mecha_notify(evaluation, PLAYER, "Torso is 60 degrees left");
    if (mech_carried_dbref(mech) > 0) {
      temp_mech =
          btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
      if (temp_mech)
        notify_printf(evaluation, PLAYER, "Towing %s.",
                      mech_to_mech_display_id(mech, temp_mech).text);
    }
    mecha_notify(evaluation, PLAYER, " ");
  }
  if (OPT & SHOW_WEAPONS) {
    if (owner)
      print_weapon_status_summary(evaluation, mech, PLAYER);
    else
      print_enemy_weapon_status(mech, PLAYER);
  }
}
