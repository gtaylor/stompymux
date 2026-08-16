#include "equipment_types.h"
#include "mech_status_api.h"
#include "mech_status_render_internal.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_text_builder.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map_conditions_api.h"
#include "map_coordinates.h"
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
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_scan_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

static int effective_jump_speed_mp(Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (mech_is_under_gravity(mech) && map) {
    const int GRAVITY = max(50, battle_map_gravity(map));

    speed = speed * 100.0F / (float)GRAVITY;
  }
  return clamp_float_to_int(speed * MP_PER_KPH);
}

static int displayed_speed(float speed) { return clamp_float_to_int(speed); }

static int displayed_heat(float heat) {
  return clamp_float_to_int(heat * 10.0F);
}

void append_status(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(checked_storage_region(buffer, size, len, size - len),
                  size - len, fmt, ap);
  va_end(ap);
}

void display_target(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char message_buffer[LBUF_SIZE];
  int arc;
  Mech *temp_mech = nullptr;
  char location[50] = {0};
  char buff[MBUF_SIZE] = {0};
  char buff1[100] = {0};

  DbRef target_dbref = mech_target_dbref(mech);
  if (target_dbref != -1) {
    temp_mech = btech_context_get_mech(mech_context(mech), target_dbref);
    if (temp_mech) {
      float range = mech_range_to(mech, temp_mech);
      if (mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                         mech_position_y(temp_mech), range)) {
        (void)snprintf(buff, sizeof(buff),
                       "Target: %s\t   Range: %.1f hexes   Bearing: %d deg\n",
                       mech_to_mech_display_id(mech, temp_mech).text,
                       (double)range,
                       map_bearing(&(MapRealSegment){
                           .start = {.x = mech_position_real_x(mech),
                                     .y = mech_position_real_y(mech)},
                           .end = {.x = mech_position_real_x(temp_mech),
                                   .y = mech_position_real_y(temp_mech)}}));
        mecha_notify(evaluation, player, buff);
        arc = in_weapon_arc(mech, mech_position_real_x(temp_mech),
                            mech_position_real_y(temp_mech));
        (void)snprintf(message_buffer, sizeof(message_buffer),
                       "Target in %s Weapons Arc",
                       (arc & TURRETARC) ? "Turret" : get_arc_id(mech, arc));
        (void)string_copy_bounded(buff, sizeof(buff), message_buffer);
        if (mech_aim_section(mech) == NUM_SECTIONS ||
            mech_aim_unit_class(mech) != mech_class(temp_mech))
          (void)string_copy_bounded(location, sizeof(location), "None");
        else
          armor_string_from_index(mech_aim_section(mech), location,
                                  mech_class(temp_mech),
                                  mech_movement_type(temp_mech));
        (void)snprintf(buff1, sizeof(buff1), "\t   Aimed Shot Location: %s",
                       location);
        (void)string_append_bounded(buff, sizeof(buff), buff1);
      } else {
        (void)snprintf(buff, sizeof(buff), "Target: NOT in line of sight!\n");
      }
    }
    mecha_notify(evaluation, player, buff);
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
    mecha_notify(evaluation, player,
                 "Weapon Safeties are [fg=red bold]OFF[reset].\n");
  if (mech_has_pilot(mech) &&
      has_bool_advantage(mech_context(mech), mech_pilot_dbref(mech),
                         "maneuvering_ace"))
    notify_printf(evaluation, player, "Turn Mode: %s",
                  conditions.tight_turn_mode ? "TIGHT" : "NORMAL");
  if (mech_charge_target_dbref(mech) > 0 &&
      mech_context(mech)->configuration->btech_newcharge) {
    temp_mech = btech_context_get_mech(mech_context(mech),
                                       mech_charge_target_dbref(mech));
    if (!temp_mech)
      return;
    if (mech_los_check(mech, temp_mech, mech_position_x(temp_mech),
                       mech_position_y(temp_mech),
                       mech_range_to(mech, temp_mech))) {
      notify_printf(evaluation, player, "ChargeTarget: %s\t  ChargeTimer: %d\n",
                    mech_to_mech_display_id(mech, temp_mech).text,
                    mech_charge_timer(mech) / 2);
    } else {
      notify_printf(evaluation, player,
                    "ChargeTarget: NOT in line of sight!\t Timer: %d\n",
                    mech_charge_timer(mech) / 2);
    }
  }
}

void show_miscbrands(Mech *mech [[maybe_unused]],
                     DbRef player [[maybe_unused]]) {}

void print_generic_status(EvaluationContext *evaluation, DbRef player,
                          Mech *mech, bool use_model_reference) {
  Mech *temp_mech = nullptr;
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_find_object(context, mech_map_dbref(mech));
  char buff[SBUF_SIZE];
  char mech_name[100] = {0};
  char mech_ref[100] = {0};
  char move_type[50] = {0};

  (void)string_copy_bounded(
      mech_name, sizeof(mech_name),
      use_model_reference
          ? mech_model_name(mech)
          : btech_attribute_read(context->database, mech_dbref(mech),
                                 A_MECHNAME, (char[LBUF_SIZE]){0}));
  (void)string_copy_bounded(
      mech_ref, sizeof(mech_ref),
      use_model_reference
          ? mech_model_reference(mech)
          : btech_attribute_read(context->database, mech_dbref(mech),
                                 A_MECHTYPE, (char[LBUF_SIZE]){0}));

  switch (mech_class(mech)) {
  case CLASS_MW:
    notify_printf(evaluation, player, "MechWarrior: %-18.18s ID:[%s]",
                  game_object_name(context->database, player),
                  mech_id(mech, false).text);
    notify_printf(evaluation, player, "MaxSpeed: %3d",
                  displayed_speed(mech_effective_maximum_speed(mech)));
    break;
  case CLASS_BSUIT:
    (void)snprintf(
        buff, sizeof(buff), "%s Name: %-18.18s  ID:[%s]   %s Reference: %s",
        bsuit_formation_name(mech), mech_name, mech_id(mech, false).text,
        bsuit_formation_name(mech), mech_ref);
    mecha_notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "MaxSpeed: %3d                  JumpRange: %d",
                  displayed_speed(mech_effective_maximum_speed(mech)),
                  effective_jump_speed_mp(mech, map));
    show_miscbrands(mech, player);
    if (mech_pilot_dbref(mech) == -1) {
      mecha_notify(evaluation, player, "Leader: NONE");
    } else {
      (void)snprintf(
          buff, sizeof(buff), "%s Leader Name: %-16.16s %s Leader injury: %d",
          bsuit_formation_name(mech),
          game_object_name(context->database, mech_pilot_dbref(mech)),
          bsuit_formation_name(mech), mech_pilot_status(mech));
      mecha_notify(evaluation, player, buff);
    }

    (void)snprintf(buff, sizeof(buff), "Max Suits: %d",
                   mech_maximum_battle_suits(mech));
    mecha_notify(evaluation, player, buff);

    mech_show_flags(&(MechFlagDisplayRequest){
        .evaluation = evaluation, .player = player, .mech = mech});

    if (mech_is_jumping(mech)) {
      (void)snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d",
                     mech_jump_destination_x(mech),
                     mech_jump_destination_y(mech));
      if (mech_condition_summary(mech).dfa_attacking &&
          mech_dfa_target_dbref(mech) != -1) {
        temp_mech =
            btech_context_get_mech(context, mech_dfa_target_dbref(mech));
        append_status(buff, sizeof(buff), "  Death From Above Target: %s",
                      mech_to_mech_display_id(mech, temp_mech).text);
      }
      mecha_notify(evaluation, player, buff);
    }
    break;
  case CLASS_MECH:
    (void)snprintf(buff, sizeof(buff),
                   "Mech Name: %-18.18s  ID:[%s]   Mech Reference: %s",
                   mech_name, mech_id(mech, false).text, mech_ref);
    mecha_notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "Tonnage:   %3d     MaxSpeed: %3d       JumpRange: %d",
                  mech_tonnage(mech),
                  displayed_speed(mech_effective_maximum_speed(mech)),
                  effective_jump_speed_mp(mech, map));
    show_miscbrands(mech, player);
    if (mech_pilot_dbref(mech) == -1) {
      mecha_notify(evaluation, player, "Pilot: NONE");
    } else {
      (void)snprintf(
          buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
          game_object_name(context->database, mech_pilot_dbref(mech)),
          mech_pilot_status(mech));
      mecha_notify(evaluation, player, buff);
    }
    mech_show_flags(&(MechFlagDisplayRequest){
        .evaluation = evaluation, .player = player, .mech = mech});
    if (!mech_is_jumping(mech) && !mech_is_fallen(mech) &&
        mech_is_started(mech) && mech_charge_target_dbref(mech) != -1) {
      temp_mech =
          btech_context_get_mech(context, mech_charge_target_dbref(mech));
      if (temp_mech) {
        (void)snprintf(buff, sizeof(buff), "CHARGING --> %s",
                       mech_to_mech_display_id(mech, temp_mech).text);
        mecha_notify(evaluation, player, buff);
      }
    }
    if (mech_is_jumping(mech)) {
      (void)snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d",
                     mech_jump_destination_x(mech),
                     mech_jump_destination_y(mech));
      if (mech_condition_summary(mech).dfa_attacking &&
          mech_dfa_target_dbref(mech) != -1) {
        temp_mech =
            btech_context_get_mech(context, mech_dfa_target_dbref(mech));
        append_status(buff, sizeof(buff), "  Death From Above Target: %s",
                      mech_to_mech_display_id(mech, temp_mech).text);
      }
      mecha_notify(evaluation, player, buff);
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
      (void)string_copy_bounded(move_type, sizeof(move_type), "Tracked");
      break;
    case MOVE_WHEEL:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Wheeled");
      break;
    case MOVE_HOVER:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Hover");
      break;
    case MOVE_VTOL:
      (void)string_copy_bounded(move_type, sizeof(move_type), "VTOL");
      break;
    case MOVE_FLY:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Flight");
      break;
    case MOVE_HULL:
      (void)string_copy_bounded(move_type, sizeof(move_type),
                                "Displacement Hull");
      break;
    case MOVE_SUB:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Submarine");
      break;
    case MOVE_FOIL:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Hydrofoil");
      break;
    case MOVE_BIPED:
    case MOVE_QUAD:
    case MOVE_NONE:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Magic");
      break;
    default:
      (void)string_copy_bounded(move_type, sizeof(move_type), "Magic");
      break;
    }
    if (mech_movement_type(mech) != MOVE_NONE) {
      (void)snprintf(buff, sizeof(buff),
                     "Vehicle Name: %-15.15s  ID:[%s]   Vehicle Reference: %s",
                     mech_name, mech_id(mech, false).text, mech_ref);
      mecha_notify(evaluation, player, buff);
      (void)snprintf(buff, sizeof(buff),
                     "Tonnage:   %3d      %s: %3d       Movement Type: %s",
                     mech_tonnage(mech),
                     mech_is_aerospace_unit(mech) ? "Max thrust" : "FlankSpeed",
                     displayed_speed(mech_effective_maximum_speed(mech)),
                     move_type);
      mecha_notify(evaluation, player, buff);
      show_miscbrands(mech, player);
      if (mech_pilot_dbref(mech) == -1) {
        mecha_notify(evaluation, player, "Pilot: NONE");
      } else {
        (void)snprintf(
            buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
            game_object_name(context->database, mech_pilot_dbref(mech)),
            mech_pilot_status(mech));
        mecha_notify(evaluation, player, buff);
      }
    } else {
      (void)snprintf(buff, sizeof(buff),
                     "Name: %-15.15s  ID:[%s]   Reference: %s", mech_name,
                     mech_id(mech, false).text, mech_ref);
      mecha_notify(evaluation, player, buff);
    }
    if (mech_class(mech) != CLASS_VTOL && !mech_is_aerospace_unit(mech)) {
      if (mech_section_internal(mech, TURRET)) {
        MechConditionSummary conditions = mech_condition_summary(mech);
        if (conditions.turret_jammed)
          mecha_notify(evaluation, player, "     TURRET JAMMED");
        else if (conditions.turret_locked)
          mecha_notify(evaluation, player, "     TURRET LOCKED");
      }
    }
    if (mech_is_flying_type(mech) && mech_is_landed(mech))
      mecha_notify(evaluation, player, "LANDED");
    mech_show_flags(&(MechFlagDisplayRequest){
        .evaluation = evaluation, .player = player, .mech = mech});
  }
}

void print_short_info(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char buff[MBUF_SIZE] = {0};
  char typespecific[50] = {0};

  switch (mech_class(mech)) {
  case CLASS_VTOL:
    (void)snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
                   (double)mech_vertical_speed(mech));
    break;
  case CLASS_MECH:
    (void)snprintf(typespecific, sizeof(typespecific), " HT: %3d/%3d/%-3d ",
                   displayed_heat(mech_heat_production(mech)),
                   displayed_heat(mech_active_heat_sinks(mech)),
                   displayed_heat(mech_heat_dissipation(mech)));
    break;
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    (void)snprintf(typespecific, sizeof(typespecific),
                   " VSPD: %3.1f  ANG: %2d  HT: %3d/%3d ",
                   (double)mech_vertical_speed(mech), mech_desired_angle(mech),
                   displayed_heat(mech_heat_production(mech)),
                   displayed_heat(mech_active_heat_sinks(mech)));
    break;
  case CLASS_VEH_NAVAL:
    if (mech_movement_type(mech) == MOVE_FOIL)
      (void)snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
                     (double)mech_vertical_speed(mech));
    [[fallthrough]];
  case CLASS_VEH_GROUND:
    /* XXX This won't work for subs with turrets.. are they possible ? */
    if (mech_section_original_internal(mech, TURRET)) {
      (void)snprintf(typespecific, sizeof(typespecific), " TUR: %3d ",
                     acceptable_degree(mech_turret_heading_degrees(mech) +
                                       mech_heading_degrees(mech)));
      break;
    }
    [[fallthrough]];
  case CLASS_MW:
  case CLASS_BSUIT:
  default:
    typespecific[0] = '\0';
    break;
  }

  (void)snprintf(
      buff, sizeof(buff),
      "LOC: %3d,%3d,%3d  HD: %3d/%3d  SP: %3.1f/%3.1f %s ST:%s",
      mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
      mech_heading_degrees(mech), mech_desired_heading_degrees(mech),
      (double)mech_current_speed(mech), (double)mech_desired_speed(mech),
      typespecific, mech_status_string(mech, 2).text);
  mecha_notify(evaluation, player, buff);
  display_target(evaluation, player, mech);
}

static constexpr int HEAT_LEVEL_BGREEN = 7;
static constexpr int HEAT_LEVEL_LYELLOW = 13;
static constexpr int HEAT_LEVEL_BYELLOW = 16;
static constexpr int HEAT_LEVEL_LRED = 18;
static constexpr int HEAT_LEVEL_BRED = 24;
static constexpr int HEAT_LEVEL_TOP = 40;

static constexpr int HEAT_LEVEL_NONE = 27;

static char heat_fill_character(const char *fill, char state) {
  if (state < 0)
    abort();
  return *checked_string_suffix(fill, (size_t)state);
}

static char *make_heat_scale_info(Mech *mech, const char *fillchar,
                                  char *heatstr, int length) {
  int counter = 0;
  int heat = displayed_speed(mech_heat_production(mech));
  int minheat = displayed_speed(mech_heat_dissipation(mech));
  int start = 0;
  char state = 1;

  BtechTextBuilder text;
  btech_text_builder_initialize(&text, heatstr, (size_t)length);
  btech_text_builder_append(&text, "[fg=black bold]");

  if (minheat > HEAT_LEVEL_NONE)
    start = minheat - HEAT_LEVEL_NONE;

  if (heat <= start) {
    heat = 0;
    state = 0;
  } else {
    heat -= start;
  }

  if (start)
    btech_text_builder_append(&text, "<[fg=black bold]");
  else
    btech_text_builder_append(&text, " [fg=black bold]");

  for (counter = start; counter < minheat; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text, "[fg=green bold]|[reset][fg=green]");
  for (; counter < minheat + HEAT_LEVEL_BGREEN; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LYELLOW; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text,
                            "[reset][fg=yellow bold]|[reset][fg=yellow]");
  for (; counter < minheat + HEAT_LEVEL_BYELLOW; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LRED; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text, "[reset][fg=red bold]|[reset][fg=red]");
  for (; counter < minheat + HEAT_LEVEL_BRED; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  if (state)
    state++;

  btech_text_builder_append(&text, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_TOP; counter++) {
    btech_text_builder_append_character(&text,
                                        heat_fill_character(fillchar, state));
    if (heat) {
      --heat;
      if (!heat)
        state = 0;
    }
  }
  btech_text_builder_append(&text, "[fg=white bold]|[reset]");
  return heatstr;
}

void print_heat_bar(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char subbuff[256];
  char buff[sizeof(subbuff) + sizeof("Temp:")];
  char heatstr[9] = ".:::::::";

  make_heat_scale_info(mech, heatstr, subbuff, 256);
  (void)snprintf(buff, sizeof(buff), "Temp:%s", subbuff);
  mecha_notify(evaluation, player, buff);
}

void print_info_status(EvaluationContext *evaluation, DbRef player, Mech *mech,
                       int own [[maybe_unused]]) {
  char message_buffer[LBUF_SIZE];
  char buff[256];
  Mech *temp_mech;
  int f;

  switch (mech_class(mech)) {
  case CLASS_MECH:
    (void)snprintf(
        buff, 256,
        "X, Y, Z:%3d,%3d,%3d  Excess Heat:  %3d deg C.  Heat Production:  "
        "%3d deg C.",
        mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
        displayed_heat(mech_excess_heat(mech)),
        displayed_heat(mech_heat_production(mech)));
    mecha_notify(evaluation, player, buff);
    (void)snprintf(buff, 256,
                   "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
                   "[fg=green bold]%3d[reset] "
                   "deg     Heat Sinks:       %3d",
                   displayed_speed(mech_current_speed(mech)),
                   mech_heading_degrees(mech),
                   displayed_speed(mech_active_heat_sinks(mech)));
    mecha_notify(evaluation, player, buff);
    (void)snprintf(
        buff, sizeof(buff),
        "Des. Speed: %3d KPH  Des. Heading: %3d deg     Heat Dissipation: "
        "%3d deg C.",
        displayed_speed(mech_desired_speed(mech)),
        mech_desired_heading_degrees(mech),
        displayed_heat(mech_heat_dissipation(mech)));
    mecha_notify(evaluation, player, buff);

    if (mech_lateral_movement(mech))
      notify_printf(evaluation, player, "You are moving laterally %s",
                    mech_lateral_description(mech));
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    if (mech_is_aerospace_unit(mech)) {
      (void)snprintf(message_buffer, sizeof(message_buffer),
                     "%s angle: [fg=green bold]%d[reset]",
                     mech_desired_angle(mech) >= 0 ? "Climbing" : "Diving",
                     abs(mech_desired_angle(mech)));
    } else {
      message_buffer[0] = '\0';
    }
    (void)snprintf(
        buff, 256, "X, Y, Z:%3d,%3d,%3d  Heat Sinks:          %3d       %s",
        mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
        displayed_speed(mech_active_heat_sinks(mech)), message_buffer);
    mecha_notify(evaluation, player, buff);
    if (mech_is_flying_type(mech) || mech_movement_type(mech) == MOVE_SUB) {
      (void)snprintf(
          buff, sizeof(buff),
          "Speed:      [fg=green bold]%3d[reset] KPH  Vertical Speed:      "
          "[fg=green bold]%3d[reset] KPH   Des. Speed %3d KPH",
          displayed_speed(mech_current_speed(mech)),
          displayed_speed(mech_vertical_speed(mech)),
          displayed_speed(mech_desired_speed(mech)));
      mecha_notify(evaluation, player, buff);
      f = max(0, mech_fuel(mech));
      if (mech_movement_type(mech) == MOVE_SUB) {
        (void)snprintf(
            buff, sizeof(buff), "Heading: %3d KPH  Des. Heading: %3d deg",
            mech_heading_degrees(mech), mech_desired_heading_degrees(mech));
      } else if (mech_aero_has_free_fuel(mech)) {
        (void)snprintf(
            buff, sizeof(buff),
            "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
            "    %3d "
            "deg   Fuel: Unlimited",
            mech_heading_degrees(mech), mech_desired_heading_degrees(mech));
      } else {
        const int ORIGINAL_FUEL = mech_original_fuel(mech);

        (void)snprintf(
            buff, sizeof(buff),
            "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
            "    %3d "
            "deg   Fuel: %d (%.2f %%)",
            mech_heading_degrees(mech), mech_desired_heading_degrees(mech), f,
            (double)(100.0F * (float)f / (float)ORIGINAL_FUEL));
      }

      mecha_notify(evaluation, player, buff);
    } else if (mech_movement_type(mech) != MOVE_NONE) {
      (void)snprintf(buff, sizeof(buff),
                     "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
                     "[fg=green bold]%3d[reset] deg",
                     displayed_speed(mech_current_speed(mech)),
                     mech_heading_degrees(mech));
      mecha_notify(evaluation, player, buff);
      (void)snprintf(buff, sizeof(buff),
                     "Des. Speed: %3d KPH  Des. Heading: %3d deg",
                     displayed_speed(mech_desired_speed(mech)),
                     mech_desired_heading_degrees(mech));
      mecha_notify(evaluation, player, buff);
    }
    mech_scan_show_turret_facing(evaluation, player, mech);
    if (mech_uses_heat(mech)) {
      notify_printf(evaluation, player,
                    "Excess Heat:%3d deg  Heat Production:     %3d deg   Heat "
                    "Dissipation: %3d deg",
                    displayed_heat(mech_excess_heat(mech)),
                    displayed_heat(mech_heat_production(mech)),
                    displayed_heat(mech_heat_dissipation(mech)));
    }
    break;
  case CLASS_MW:
  case CLASS_BSUIT:
    (void)snprintf(
        buff, sizeof(buff),
        "X, Y, Z:%3d,%3d,%3d  Speed:      [fg=green bold]%3d[reset] KPH  "
        "Heading:   "
        "   [fg=green bold]%3d[reset] deg",
        mech_position_x(mech), mech_position_y(mech), mech_position_z(mech),
        displayed_speed(mech_current_speed(mech)), mech_heading_degrees(mech));
    mecha_notify(evaluation, player, buff);
    (void)snprintf(
        buff, sizeof(buff),
        "                     Des. Speed: %3d KPH  Des. Heading: %3d deg",
        displayed_speed(mech_desired_speed(mech)),
        mech_desired_heading_degrees(mech));
    mecha_notify(evaluation, player, buff);
    break;
  }

  if (mech_uses_heat(mech)) {
    print_heat_bar(evaluation, player, mech);
  }
  mecha_notify(evaluation, player, "  ");
  // Show our locked target info (hex or unit).
  display_target(evaluation, player, mech);

  if (mech_carried_dbref(mech) > 0) {
    temp_mech =
        btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
    if (temp_mech)
      notify_printf(evaluation, player, "Towing %s.",
                    mech_to_mech_display_id(mech, temp_mech).text);
  }
}
