#include "mech_status_api.h"
#include "mech_status_render_internal.h"

#include <ctype.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_conditions_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_contacts_api.h"
#include "mech_crew_api.h"
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
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

static int effective_jump_speed_mp(Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (mech_is_under_gravity(mech) && map)
    speed = speed * 100 / MAX(50, battle_map_gravity(map));
  return (int)(speed * MP_PER_KPH);
}

void append_status(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size);

void append_status(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
  vsnprintf(buffer + len, size - len, fmt, ap);
  va_end(ap);
}

void DisplayTarget(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  int arc;
  Mech *tempMech = nullptr;
  char location[50] = {0};
  char buff[MBUF_SIZE] = {0};
  char buff1[100] = {0};

  DbRef target_dbref = mech_target_dbref(mech);
  if (target_dbref != -1) {
    tempMech = btech_context_get_mech(mech_context(mech), target_dbref);
    if (tempMech) {
      float range = mech_range_to(mech, tempMech);
      if (InLineOfSight(mech, tempMech, mech_position_x(tempMech),
                        mech_position_y(tempMech), range)) {
        snprintf(buff, sizeof(buff),
                 "Target: %s\t   Range: %.1f hexes   Bearing: %d deg\n",
                 mech_to_mech_display_id(mech, tempMech).text, range,
                 FindBearing(mech_position_real_x(mech),
                             mech_position_real_y(mech),
                             mech_position_real_x(tempMech),
                             mech_position_real_y(tempMech)));
        notify(evaluation, player, buff);
        arc = InWeaponArc(mech, mech_position_real_x(tempMech),
                          mech_position_real_y(tempMech));
        strcpy(buff,
               tprintf("Target in %s Weapons Arc",
                       (arc & TURRETARC) ? "Turret" : GetArcID(mech, arc)));
        if (mech_aim_section(mech) == NUM_SECTIONS ||
            mech_aim_unit_class(mech) != mech_class(tempMech))
          strcpy(location, "None");
        else
          ArmorStringFromIndex(mech_aim_section(mech), location,
                               mech_class(tempMech),
                               mech_movement_type(tempMech));
        snprintf(buff1, sizeof(buff1), "\t   Aimed Shot Location: %s",
                 location);
        strcat(buff, buff1);
      } else
        snprintf(buff, sizeof(buff), "Target: NOT in line of sight!\n");
    }
    notify(evaluation, player, buff);
  } else if (mech_target_hex_x(mech) != -1 && mech_target_hex_y(mech) != -1) {
    if (mech_targets_building(mech))
      notify_printf(evaluation, player, "Target: Building at %d %d\n",
                    mech_target_hex_x(mech), mech_target_hex_y(mech));
    else if (mech_targets_hex(mech))
      notify_printf(evaluation, player, "Target: Hex %d %d\n",
                    mech_target_hex_x(mech), mech_target_hex_y(mech));
    else
      notify_printf(evaluation, player, "Target: %d %d\n",
                    mech_target_hex_x(mech), mech_target_hex_y(mech));
  }
  MechConditionSummary conditions = mech_condition_summary(mech);
  if (conditions.player_killer)
    notify(evaluation, player,
           "Weapon Safeties are [fg=red bold]OFF[reset].\n");
  if (mech_has_pilot(mech) &&
      HasBoolAdvantage(mech_context(mech), mech_pilot_dbref(mech),
                       "maneuvering_ace"))
    notify_printf(evaluation, player, "Turn Mode: %s",
                  conditions.tight_turn_mode ? "TIGHT" : "NORMAL");
  if (mech_charge_target_dbref(mech) > 0 &&
      mech_context(mech)->configuration->btech_newcharge) {
    tempMech = btech_context_get_mech(mech_context(mech),
                                      mech_charge_target_dbref(mech));
    if (!tempMech)
      return;
    if (InLineOfSight(mech, tempMech, mech_position_x(tempMech),
                      mech_position_y(tempMech),
                      mech_range_to(mech, tempMech))) {
      notify_printf(evaluation, player, "ChargeTarget: %s\t  ChargeTimer: %d\n",
                    mech_to_mech_display_id(mech, tempMech).text,
                    mech_charge_timer(mech) / 2);
    } else {
      notify_printf(evaluation, player,
                    "ChargeTarget: NOT in line of sight!\t Timer: %d\n",
                    mech_charge_timer(mech) / 2);
    }
  }
}

void show_miscbrands(Mech *mech, DbRef player) {
  (void)mech;
  (void)player;
}

void PrintGenericStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                        int own, int usex) {
  Mech *tempMech = nullptr;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_find_object(context, mech_map_dbref(mech));
  char buff[SBUF_SIZE];
  char mech_name[100] = {0};
  char mech_ref[100] = {0};
  char move_type[50] = {0};

  strcpy(mech_name,
         usex ? mech_model_name(mech)
              : btech_attribute_read(context->database, mech_dbref(mech),
                                     A_MECHNAME, (char[LBUF_SIZE]){0}));
  strcpy(mech_ref,
         usex ? mech_model_reference(mech)
              : btech_attribute_read(context->database, mech_dbref(mech),
                                     A_MECHREF, (char[LBUF_SIZE]){0}));

  switch (mech_class(mech)) {
  case CLASS_MW:
    notify_printf(evaluation, player, "MechWarrior: %-18.18s ID:[%s]",
                  game_object_name(context->database, player),
                  mech_id(mech, false).text);
    notify_printf(evaluation, player, "MaxSpeed: %3d",
                  (int)mech_effective_maximum_speed(mech));
    break;
  case CLASS_BSUIT:
    snprintf(buff, sizeof(buff),
             "%s Name: %-18.18s  ID:[%s]   %s Reference: %s",
             GetBSuitName(mech), mech_name, mech_id(mech, false).text,
             GetBSuitName(mech), mech_ref);
    notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "MaxSpeed: %3d                  JumpRange: %d",
                  (int)mech_effective_maximum_speed(mech),
                  effective_jump_speed_mp(mech, map));
    show_miscbrands(mech, player);
    if (mech_pilot_dbref(mech) == -1)
      notify(evaluation, player, "Leader: NONE");
    else {
      snprintf(buff, sizeof(buff),
               "%s Leader Name: %-16.16s %s Leader injury: %d",
               GetBSuitName(mech),
               game_object_name(context->database, mech_pilot_dbref(mech)),
               GetBSuitName(mech), mech_pilot_status(mech));
      notify(evaluation, player, buff);
    }

    snprintf(buff, sizeof(buff), "Max Suits: %d",
             mech_maximum_battle_suits(mech));
    notify(evaluation, player, buff);

    Mech_ShowFlags(evaluation, player, mech, 0, 0);

    if (mech_is_jumping(mech)) {
      snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d",
               mech_jump_destination_x(mech), mech_jump_destination_y(mech));
      if (mech_condition_summary(mech).dfa_attacking &&
          mech_dfa_target_dbref(mech) != -1) {
        tempMech = btech_context_get_mech(context, mech_dfa_target_dbref(mech));
        snprintf(buff + strlen(buff), sizeof(buff) - strlen(buff),
                 "  Death From Above Target: %s",
                 mech_to_mech_display_id(mech, tempMech).text);
      }
      notify(evaluation, player, buff);
    }
    break;
  case CLASS_MECH:
    snprintf(buff, sizeof(buff),
             "Mech Name: %-18.18s  ID:[%s]   Mech Reference: %s", mech_name,
             mech_id(mech, false).text, mech_ref);
    notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "Tonnage:   %3d     MaxSpeed: %3d       JumpRange: %d",
                  mech_tonnage(mech), (int)mech_effective_maximum_speed(mech),
                  effective_jump_speed_mp(mech, map));
    show_miscbrands(mech, player);
    if (mech_pilot_dbref(mech) == -1)
      notify(evaluation, player, "Pilot: NONE");
    else {
      snprintf(buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
               game_object_name(context->database, mech_pilot_dbref(mech)),
               mech_pilot_status(mech));
      notify(evaluation, player, buff);
    }
    Mech_ShowFlags(evaluation, player, mech, 0, 0);
    if (!mech_is_jumping(mech) && !mech_is_fallen(mech) &&
        mech_is_started(mech) && mech_charge_target_dbref(mech) != -1) {
      tempMech =
          btech_context_get_mech(context, mech_charge_target_dbref(mech));
      if (tempMech) {
        snprintf(buff, sizeof(buff), "CHARGING --> %s",
                 mech_to_mech_display_id(mech, tempMech).text);
        notify(evaluation, player, buff);
      }
    }
    if (mech_is_jumping(mech)) {
      snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d",
               mech_jump_destination_x(mech), mech_jump_destination_y(mech));
      if (mech_condition_summary(mech).dfa_attacking &&
          mech_dfa_target_dbref(mech) != -1) {
        tempMech = btech_context_get_mech(context, mech_dfa_target_dbref(mech));
        snprintf(buff + strlen(buff), sizeof(buff) - strlen(buff),
                 "  Death From Above Target: %s",
                 mech_to_mech_display_id(mech, tempMech).text);
      }
      notify(evaluation, player, buff);
    }
    break;
  case CLASS_VTOL:
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    switch (mech_movement_type(mech)) {
    case MOVE_TRACK:
      strcpy(move_type, "Tracked");
      break;
    case MOVE_WHEEL:
      strcpy(move_type, "Wheeled");
      break;
    case MOVE_HOVER:
      strcpy(move_type, "Hover");
      break;
    case MOVE_VTOL:
      strcpy(move_type, "VTOL");
      break;
    case MOVE_FLY:
      strcpy(move_type, "Flight");
      break;
    case MOVE_HULL:
      strcpy(move_type, "Displacement Hull");
      break;
    case MOVE_SUB:
      strcpy(move_type, "Submarine");
      break;
    case MOVE_FOIL:
      strcpy(move_type, "Hydrofoil");
      break;
    default:
      strcpy(move_type, "Magic");
      break;
    }
    if (mech_movement_type(mech) != MOVE_NONE) {
      snprintf(buff, sizeof(buff),
               "Vehicle Name: %-15.15s  ID:[%s]   Vehicle Reference: %s",
               mech_name, mech_id(mech, false).text, mech_ref);
      notify(evaluation, player, buff);
      snprintf(buff, sizeof(buff),
               "Tonnage:   %3d      %s: %3d       Movement Type: %s",
               mech_tonnage(mech),
               mech_is_aerospace_unit(mech) ? "Max thrust" : "FlankSpeed",
               (int)mech_effective_maximum_speed(mech), move_type);
      notify(evaluation, player, buff);
      show_miscbrands(mech, player);
      if (mech_pilot_dbref(mech) == -1)
        notify(evaluation, player, "Pilot: NONE");
      else {
        snprintf(buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
                 game_object_name(context->database, mech_pilot_dbref(mech)),
                 mech_pilot_status(mech));
        notify(evaluation, player, buff);
      }
    } else {
      snprintf(buff, sizeof(buff), "Name: %-15.15s  ID:[%s]   Reference: %s",
               mech_name, mech_id(mech, false).text, mech_ref);
      notify(evaluation, player, buff);
    }
    if (mech_class(mech) != CLASS_VTOL && !mech_is_aerospace_unit(mech))
      if (mech_section_internal(mech, TURRET)) {
        MechConditionSummary conditions = mech_condition_summary(mech);
        if (conditions.turret_jammed)
          notify(evaluation, player, "     TURRET JAMMED");
        else if (conditions.turret_locked)
          notify(evaluation, player, "     TURRET LOCKED");
      }
    if (mech_is_flying_type(mech) && mech_is_landed(mech))
      notify(evaluation, player, "LANDED");
    Mech_ShowFlags(evaluation, player, mech, 0, 0);
  }
}

void PrintShortInfo(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char buff[MBUF_SIZE] = {0};
  char typespecific[50] = {0};

  switch (mech_class(mech)) {
  case CLASS_VTOL:
    snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
             mech_vertical_speed(mech));
    break;
  case CLASS_MECH:
    snprintf(typespecific, sizeof(typespecific), " HT: %3d/%3d/%-3d ",
             (int)(10. * mech_heat_production(mech)),
             (int)(10. * mech_active_heat_sinks(mech)),
             (int)(10. * mech_heat_dissipation(mech)));
    break;
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    snprintf(typespecific, sizeof(typespecific),
             " VSPD: %3.1f  ANG: %2d  HT: %3d/%3d ", mech_vertical_speed(mech),
             mech_desired_angle(mech), (int)(10 * mech_heat_production(mech)),
             (int)(10 * mech_active_heat_sinks(mech)));
    break;
  case CLASS_VEH_NAVAL:
    if (mech_movement_type(mech) == MOVE_FOIL)
      snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
               mech_vertical_speed(mech));
    /* FALLTHROUGH */
  case CLASS_VEH_GROUND:
    /* XXX This won't work for subs with turrets.. are they possible ? */
    if (mech_section_original_internal(mech, TURRET)) {
      snprintf(typespecific, sizeof(typespecific), " TUR: %3d ",
               AcceptableDegree(mech_turret_heading_degrees(mech) +
                                mech_heading_degrees(mech)));
      break;
    }
    /* FALLTHROUGH */
  default:
    typespecific[0] = '\0';
    break;
  }

  snprintf(buff, sizeof(buff),
           "LOC: %3d,%3d,%3d  HD: %3d/%3d  SP: %3.1f/%3.1f %s ST:%s",
           mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
           mech_heading_degrees(mech), mech_desired_heading_degrees(mech),
           mech_current_speed(mech), mech_desired_speed(mech), typespecific,
           mech_status_string(mech, 2).text);
  notify(evaluation, player, buff);
  DisplayTarget(evaluation, player, mech);
}

#define HEAT_LEVEL_LGREEN 0
#define HEAT_LEVEL_BGREEN 7
#define HEAT_LEVEL_LYELLOW 13
#define HEAT_LEVEL_BYELLOW 16
#define HEAT_LEVEL_LRED 18
#define HEAT_LEVEL_BRED 24
#define HEAT_LEVEL_TOP 40

#define HEAT_LEVEL_NONE 27

static char *MakeHeatScaleInfo(Mech *mech, char *fillchar, char *heatstr,
                               int length) {
  int counter = 0, heat = mech_heat_production(mech),
      minheat = mech_heat_dissipation(mech), start = 0;
  char state = 1;

  memset(heatstr, 0, sizeof(char) * length);

  strcat(heatstr, "[fg=black bold]");

  if (minheat > HEAT_LEVEL_NONE)
    start = minheat - HEAT_LEVEL_NONE;

  if (heat <= start) {
    heat = 0;
    state = 0;
  } else
    heat -= start;

  if (start)
    strcat(heatstr, "<[fg=black bold]");
  else
    strcat(heatstr, " [fg=black bold]");

  for (counter = start; counter < minheat; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[fg=green bold]|[reset][fg=green]");
  for (; counter < minheat + HEAT_LEVEL_BGREEN; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LYELLOW; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[reset][fg=yellow bold]|[reset][fg=yellow]");
  for (; counter < minheat + HEAT_LEVEL_BYELLOW; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LRED; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[reset][fg=red bold]|[reset][fg=red]");
  for (; counter < minheat + HEAT_LEVEL_BRED; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_TOP; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  strcat(heatstr, "[fg=white bold]|[reset]");
  return heatstr;
}

void PrintHeatBar(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char subbuff[256];
  char buff[sizeof(subbuff) + sizeof("Temp:")];
  char heatstr[9] = ".:::::::";

  MakeHeatScaleInfo(mech, heatstr, subbuff, 256);
  snprintf(buff, sizeof(buff), "Temp:%s", subbuff);
  notify(evaluation, player, buff);
}

void PrintInfoStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                     int own) {
  char buff[256];
  Mech *tempMech;
  int f;

  switch (mech_class(mech)) {
  case CLASS_MECH:
    snprintf(buff, 256,
             "X, Y, Z:%3d,%3d,%3d  Excess Heat:  %3d deg C.  Heat Production:  "
             "%3d deg C.",
             mech_position_x(mech), mech_position_y(mech),
             mech_position_z(mech), (int)(10. * mech_excess_heat(mech)),
             (int)(10. * mech_heat_production(mech)));
    notify(evaluation, player, buff);
    snprintf(buff, 256,
             "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
             "[fg=green bold]%3d[reset] "
             "deg     Heat Sinks:       %3d",
             (int)mech_current_speed(mech), mech_heading_degrees(mech),
             (int)mech_active_heat_sinks(mech));
    notify(evaluation, player, buff);
    snprintf(buff, sizeof(buff),
             "Des. Speed: %3d KPH  Des. Heading: %3d deg     Heat Dissipation: "
             "%3d deg C.",
             (int)mech_desired_speed(mech), mech_desired_heading_degrees(mech),
             (int)(10. * mech_heat_dissipation(mech)));
    notify(evaluation, player, buff);

    if (mech_lateral_movement(mech))
      notify_printf(evaluation, player, "You are moving laterally %s",
                    LateralDesc(mech));
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    snprintf(
        buff, 256, "X, Y, Z:%3d,%3d,%3d  Heat Sinks:          %3d       %s",
        mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
        (int)mech_active_heat_sinks(mech),
        mech_is_aerospace_unit(mech)
            ? tprintf("%s angle: [fg=green bold]%d[reset]",
                      mech_desired_angle(mech) >= 0 ? "Climbing" : "Diving",
                      abs(mech_desired_angle(mech)))
            : "");
    notify(evaluation, player, buff);
    if (mech_is_flying_type(mech) || mech_movement_type(mech) == MOVE_SUB) {
      snprintf(
          buff, sizeof(buff),
          "Speed:      [fg=green bold]%3d[reset] KPH  Vertical Speed:      "
          "[fg=green bold]%3d[reset] KPH   Des. Speed %3d KPH",
          (int)mech_current_speed(mech), (int)mech_vertical_speed(mech),
          (int)mech_desired_speed(mech));
      notify(evaluation, player, buff);
      f = MAX(0, mech_fuel(mech));
      if (mech_movement_type(mech) == MOVE_SUB) {
        snprintf(buff, sizeof(buff), "Heading: %3d KPH  Des. Heading: %3d deg",
                 mech_heading_degrees(mech),
                 mech_desired_heading_degrees(mech));
      } else if (mech_aero_has_free_fuel(mech)) {
        snprintf(buff, sizeof(buff),
                 "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
                 "    %3d "
                 "deg   Fuel: Unlimited",
                 mech_heading_degrees(mech),
                 mech_desired_heading_degrees(mech));
      } else {
        snprintf(buff, sizeof(buff),
                 "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
                 "    %3d "
                 "deg   Fuel: %d (%.2f %%)",
                 mech_heading_degrees(mech), mech_desired_heading_degrees(mech),
                 f, 100.0 * f / mech_original_fuel(mech));
      }

      notify(evaluation, player, buff);
    } else if (mech_movement_type(mech) != MOVE_NONE) {
      snprintf(buff, sizeof(buff),
               "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
               "[fg=green bold]%3d[reset] deg",
               (int)mech_current_speed(mech), mech_heading_degrees(mech));
      notify(evaluation, player, buff);
      snprintf(buff, sizeof(buff), "Des. Speed: %3d KPH  Des. Heading: %3d deg",
               (int)mech_desired_speed(mech),
               mech_desired_heading_degrees(mech));
      notify(evaluation, player, buff);
    }
    ShowTurretFacing(evaluation, player, 0, mech);
    if (mech_uses_heat(mech)) {
      notify_printf(evaluation, player,
                    "Excess Heat:%3d deg  Heat Production:     %3d deg   Heat "
                    "Dissipation: %3d deg",
                    (int)(10. * mech_excess_heat(mech)),
                    (int)(10. * mech_heat_production(mech)),
                    (int)(10. * mech_heat_dissipation(mech)));
    }
    break;
  case CLASS_MW:
  case CLASS_BSUIT:
    snprintf(buff, sizeof(buff),
             "X, Y, Z:%3d,%3d,%3d  Speed:      [fg=green bold]%3d[reset] KPH  "
             "Heading:   "
             "   [fg=green bold]%3d[reset] deg",
             mech_position_x(mech), mech_position_y(mech),
             mech_position_z(mech), (int)mech_current_speed(mech),
             mech_heading_degrees(mech));
    notify(evaluation, player, buff);
    snprintf(buff, sizeof(buff),
             "                     Des. Speed: %3d KPH  Des. Heading: %3d deg",
             (int)mech_desired_speed(mech), mech_desired_heading_degrees(mech));
    notify(evaluation, player, buff);
    break;
  }

  if (mech_uses_heat(mech)) {
    PrintHeatBar(evaluation, player, mech);
  }
  notify(evaluation, player, "  ");
  // Show our locked target info (hex or unit).
  DisplayTarget(evaluation, player, mech);

  if (mech_carried_dbref(mech) > 0)
    if ((tempMech = btech_context_get_mech(mech_context(mech),
                                           mech_carried_dbref(mech))))
      notify_printf(evaluation, player, "Towing %s.",
                    mech_to_mech_display_id(mech, tempMech).text);
}

/* Status commands! */
void mech_status(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  int doweap = 0, doinfo = 0, doarmor = 0, doshort = 0, doheat = 0, loop;
  int i;
  int usex = 0;
  bool weird = false;
  char buf[LBUF_SIZE] = {0};
  char weird_buffer[LBUF_SIZE] = {0};

  cch(MECH_USUALSM);
  if (!buffer || !strlen(buffer))
    // No arguments, we'll go with our default 'status' output.
    doweap = doinfo = doarmor = doheat = 1;
  else {
    // Argument provided, only show certain parts.
    for (loop = 0; buffer[loop]; loop++) {
      switch (toupper(buffer[loop])) {
      case 'R':
        doweap = doinfo = doarmor = doheat = usex = 1;
        break;
      case 'A':
        // Armor status
        if (toupper(buffer[loop + 1]) == 'R')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        doarmor = 1;
        break;
      case 'I':
        // Speed/Heading/Heat
        doinfo = 1;
        if (toupper(buffer[loop + 1]) == 'N')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        break;
      case 'W':
        // Weapons list.
        doweap = 1;
        if (toupper(buffer[loop + 1]) == 'E')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        break;
      case 'N':
        // Really weird status display.
        weird = true;
        break;
      case 'S':
        // Very short one-line status.
        doshort = 1;
        break;
      case 'H':
        // Just the heat bar.
        doheat = 1;
        break;
      }
    }
  }

  // Very short one-line status.
  if (doshort) {
    PrintShortInfo(evaluation, player, mech);
    return;
  }

  // Really weird status display.
  if (weird) {
    snprintf(buf, sizeof(buf), "%s %s %d %d/%d/%d %d ",
             mech_model_reference(mech), mech_model_name(mech),
             mech_tonnage(mech), (int)(mech_maximum_speed(mech) / MP1) * 2 / 3,
             (int)(mech_maximum_speed(mech) / MP1),
             (int)(mech_jump_speed(mech) / MP1),
             (int)mech_active_heat_sinks(mech));
    memcpy(weird_buffer, buf, sizeof(weird_buffer));

  } else if (!doheat || (doarmor | doinfo | doweap))
    PrintGenericStatus(evaluation, player, mech, 1, usex);

  // Show our armor diagram.
  if (doarmor) {
    if (!weird) {
      PrintArmorStatus(evaluation, player, mech, 1);
      notify(evaluation, player, " ");
    } else {
      for (i = 0; i < NUM_SECTIONS; i++)
        if (mech_section_original_armor(mech, i)) {
          if (mech_section_original_rear_armor(mech, i))
            snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "%d|%d|%d ",
                     mech_section_original_armor(mech, i),
                     mech_section_original_internal(mech, i),
                     mech_section_original_rear_armor(mech, i));
          else
            snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "%d|%d ",
                     mech_section_original_armor(mech, i),
                     mech_section_original_internal(mech, i));
        }
    }
  }

  // Standard heat/heading/dive/etc.
  if (doinfo && !weird) {
    PrintInfoStatus(evaluation, player, mech, 1);
    // notify(evaluation, player, " ");
  }

  // Show our heat bar by itself.
  if (!doinfo && doheat && mech_uses_heat(mech)) {
    PrintHeatBar(evaluation, player, mech);
  }

  // Weapons readout.
  if (doweap)
    print_weapon_status(evaluation, mech, player, weird, weird_buffer,
                        sizeof(weird_buffer));

  // Really strange, short status info.
  if (weird)
    notify(evaluation, player, weird_buffer);
}
