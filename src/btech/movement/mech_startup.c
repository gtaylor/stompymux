#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/*
 * $Id: mech.startup.c,v 1.2 2005/06/23 18:31:42 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Last modified: Thu Jul  9 06:59:34 1998 fingon
 *
 */

#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "autopilot_resume_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "econ_cmds_api.h"
#include "map_los_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_startup_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"

/* NOTE: Number of boot messages for both types _MUST_ match */

enum { BOOT_MESSAGE_COUNT = 6 };

static const char *const bsuit_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->         Initializing powerpack       <-[reset]",
    "[fg=green]->          Powerpack operational       <-[reset]",
    "[fg=green]->             Suit sealed              <-[reset]",
    "[fg=green]->  Computer system is now operational  <-[reset]",
    "[fg=green]->         Air pressure steady          <-[reset]",
    ("       [fg=green]- [fg=red]-=>[fg=white bold] All systems go![reset] "
     "[fg=red]<= [fg=green]-[reset]")};

static const char *const aero_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->       Main reactor is now online    <-[reset]",
    "[fg=green]->            Thrusters online         <-[reset]",
    "[fg=green]->  Main computer system is now online <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("       [fg=green]- [fg=red]-=>[fg=white bold] All systems go![reset] "
     "[fg=red]<= [fg=green]-[reset]")};

static const char *const bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->       Main reactor is now online    <-[reset]",
    "[fg=green]->         Gyros are now stable        <-[reset]",
    "[fg=green]->  Main computer system is now online <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *const hover_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->  Powerplant initialized and online  <-[reset]",
    "[fg=green]->   Checking plenum chamber status    <-[reset]",
    "[fg=green]->         Verifying fan status        <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *const track_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->  Powerplant initialized and online  <-[reset]",
    "[fg=green]->      Auto-aligning drive wheels     <-[reset]",
    "[fg=green]->       Adjusting track tension       <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *const wheel_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->  Powerplant initialized and online  <-[reset]",
    "[fg=green]->  Performing steering system checks  <-[reset]",
    "[fg=green]->        Checking wheel status        <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *const vtol_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->     Initializing main powerplant    <-[reset]",
    "[fg=green]-> Main turbine online and operational <-[reset]",
    "[fg=green]->      Rotor transmission engaged     <-[reset]",
    "[fg=green]->     Scanners are now operational    <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *const naval_bootmsgs[BOOT_MESSAGE_COUNT] = {
    "[fg=green]->       Main reactor is now online    <-[reset]",
    "[fg=green]->  Main computer system is now online <-[reset]",
    "[fg=green]->   Hull integrity monitoring online  <-[reset]",
    "[fg=green]-> Ballast and propulsion are nominal  <-[reset]",
    "[fg=green]-> Targeting system is now operational <-[reset]",
    ("   [fg=green]- [fg=red]-=>[fg=white bold] All systems "
     "operational![reset] [fg=red]<=- [fg=green]-[reset]")};

static const char *
startup_message(const char *const messages[static BOOT_MESSAGE_COUNT],
                long timer) {
  if (timer < 0)
    abort();
  const size_t index = (size_t)timer;
  const char *const *message = checked_storage_at_const(
      messages, BOOT_MESSAGE_COUNT, sizeof(*messages), index);
  return *message;
}

static int mech_startup_step_delay(const Mech *mech) {
  return mech_class(mech) == CLASS_BSUIT ? 1
                                         : STARTUP_TIME / BOOT_MESSAGE_COUNT;
}

static void mech_startup_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long timer = (long)e->data2;
  BattleMap *mech_map;
  int unit_class = mech_class(mech);
  int movement_type = mech_movement_type(mech);
  BtechContext *context = mech_context(mech);

  /*
   * Each *_bootmsgs[] array is a fixed set of string-literal boot messages
   * indexed by timer; none of them contain printf conversions, just
   * non-literal styled text.
   */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-nonliteral"
#pragma clang diagnostic ignored "-Wformat-security"
#endif
  if (mech_is_aerospace_unit(mech)) {
    mech_printf(mech, MECHALL, startup_message(aero_bootmsgs, timer));
  } else if (unit_class == CLASS_BSUIT) {
    mech_printf(mech, MECHALL, startup_message(bsuit_bootmsgs, timer));
  } else
    switch (movement_type) {
    case MOVE_HOVER:
      mech_printf(mech, MECHALL, startup_message(hover_bootmsgs, timer));
      break;
    case MOVE_TRACK:
      mech_printf(mech, MECHALL, startup_message(track_bootmsgs, timer));
      break;
    case MOVE_WHEEL:
      mech_printf(mech, MECHALL, startup_message(wheel_bootmsgs, timer));
      break;
    case MOVE_VTOL:
      mech_printf(mech, MECHALL, startup_message(vtol_bootmsgs, timer));
      break;
    case MOVE_BIPED:
      mech_printf(mech, MECHALL, startup_message(bootmsgs, timer));
      break;
    case MOVE_HULL:
    case MOVE_FOIL:
    case MOVE_SUB:
      mech_printf(mech, MECHALL, startup_message(naval_bootmsgs, timer));
      break;
    default:
      mech_printf(mech, MECHALL, startup_message(bootmsgs, timer));
      break;
    }
#ifdef __clang__
#pragma clang diagnostic pop
#endif
  timer++;

  /* Check if the unit is in water and if it should die */
  /* Make sure it checks pretty early in the startup */
  if (timer >= 2) {

    if (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
        mech_position_z(mech) < 0 &&
        (unit_class == CLASS_VEH_GROUND || unit_class == CLASS_VTOL ||
         unit_class == CLASS_BSUIT || unit_class == CLASS_AERO ||
         unit_class == CLASS_DS) &&
        !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {

      mech_notify(mech, MECHALL,
                  "Water floods your engine and your unit "
                  "becomes inoperable.");
      if (unit_class == CLASS_BSUIT)
        mech_los_broadcast(mech, "emits some bubbles and flails their arms "
                                 "around as they sink to the bottom.");
      else
        mech_los_broadcast(mech,
                           "emits some bubbles as its engines are flooded.");
      mech_destroy(mech, mech, 0, KILL_TYPE_FLOOD);
      return;
    }
  }

  if (timer < BOOT_MESSAGE_COUNT) {
    mech_event_schedule(mech, EVENT_STARTUP, mech_startup_event,
                        mech_startup_step_delay(mech), timer);
    return;
  }
  if ((mech_map = btech_context_get_map(context, mech_map_dbref(mech))))
    battle_map_los_observer_clear(mech_map, mech_map_slot(mech));
  initialize_pc(mech_pilot_dbref(mech), mech);
  mech_power_up(mech);
  MarkForLOSUpdate(mech);
  mech_cargo_weight_recalculate(mech);
  mech_player_killer_set(mech, false);
  mech_los_broadcast(mech, "powers up!");
  mech_vertical_speed_set(mech, 0.0F);
  mech_sixth_sense_set(
      mech,
      mech_pilot_dbref(mech) > 0 &&
              is_player(btech_context_database(context), mech_pilot_dbref(mech))
          ? char_getvalue(context, mech_pilot_dbref(mech), "Sixth_Sense")
          : 0);
  if (mech_is_flying_type(mech)) {
    char terrain = mech_real_terrain_get(mech);
    int elevation = mech_position_elevation_magnitude(mech);
    int surface_elevation =
        terrain == BATTLE_TERRAIN_WATER || terrain == BATTLE_TERRAIN_ICE
            ? -elevation
            : elevation;
    if (mech_position_z(mech) <= surface_elevation)
      mech_landed_set(mech, true);
  }
  mech_communication_skill_set(mech, DEFAULT_COMM);
  if (is_player(btech_context_database(context), mech_pilot_dbref(mech))) {
    mech_communication_skill_set(
        mech, char_getskilltarget(context, mech_pilot_dbref(mech),
                                  "Comm-Conventional", 0));
    mech_perception_target_set(
        mech,
        char_getskilltarget(context, mech_pilot_dbref(mech), "Perception", 0));
  } else {
    mech_communication_skill_set(mech, 6);
    mech_perception_target_set(mech, 6);
  }
  mech_communication_last_tick_set(mech, 0);
  mech_last_startup_set(mech, (int)btech_context_now(context));
  if (mech_is_aerospace_unit(mech) && !mech_is_landed(mech)) {
    mech_desired_angle_set(mech, -90);
    mech_motion_vector_reset(mech);
    mech_desired_speed_set(mech, mech_maximum_speed(mech));
    mech_maybe_move(mech);
  }
  autopilot_resume_for_mech(mech);
}

void mech_startup(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  int n;
  int unit_class = mech_class(mech);
  BtechContext *context = mech_context(mech);
  GameDatabase *database = btech_context_database(context);

  if (!common_checks(player, mech, MECH_CONSISTENT | MECH_MAP | MECH_PILOT_CON))
    return;
  if (buffer != nullptr)
    buffer =
        checked_storage_at_const(buffer, strlen(buffer) + 1, sizeof(*buffer),
                                 strspn(buffer, " \t\r\n\f\v"));
  if (!buffer)
    buffer = "";
  if (!(is_good_obj(database, player) &&
        (is_alive(database, player) || is_xcode(database, player)))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That is not a valid player!");
    return;
  }
  if (unit_class == CLASS_MW && mech_is_started(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're up and about already!");
    return;
  }
  if (mech_is_towed(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You're being towed! Wait for drop-off before starting again!");
    return;
  }
  if (mech_map_dbref(mech) < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are not on any map!");
    return;
  }
  if (mech_is_destroyed(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'Mech is destroyed!");
    return;
  }
  if (mech_is_started(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'Mech is already started!");
    return;
  }
  if (mech_event_count(mech, EVENT_STARTUP)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'Mech is already starting!");
    return;
  }
  if (mech_event_count(mech, EVENT_VEHICLE_EXTINGUISH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're way too busy putting out fires!");
    return;
  }
  n = figure_latest_tech_event(mech);
  if (n) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "This 'Mech is still under repairs (see checkstatus for more info)");
    return;
  }
  if (mech_excess_heat(mech) > 30.0F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This 'Mech is too hot to start back up!");
    return;
  }
  if (is_in_character(database, mech_dbref(mech)) &&
      !is_wizard(database, player) &&
      (char_lookupplayer(
           context, GOD, GOD, 0,
           btech_attribute_read(database, mech_dbref(mech), A_PILOTNUM,
                                (char[LBUF_SIZE]){0})) != player)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This isn't your mech!");
    return;
  }
  n = 0;
  if (*buffer && !strncasecmp(buffer, "override", strlen(buffer))) {
    if (!is_wizard(database, player)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Insufficient access!");
      return;
    }
    n = BOOT_MESSAGE_COUNT - 1;
  }
  mech_pilot_dbref_set(mech, player);

  /* Initialize the PilotDamage from the new pilot */
  fix_pilotdamage(mech, player);
  mech_notify(mech, MECHALL, "Startup Cycle commencing...");
  mech_set_recycle_limb(mech, RLEG, 0);
  mech_set_recycle_limb(mech, LLEG, 0);
  mech_set_recycle_limb(mech, RARM, 0);
  mech_set_recycle_limb(mech, LARM, 0);
  mech_set_recycle_limb(mech, RTORSO, 0);
  mech_set_recycle_limb(mech, LTORSO, 0);
  mech_event_schedule(
      mech, EVENT_STARTUP, mech_startup_event,
      (n || unit_class == CLASS_MW) ? 1 : mech_startup_step_delay(mech),
      (long)(unit_class == CLASS_MW ? BOOT_MESSAGE_COUNT - 1 : n));
}

void mech_shutdown(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  int unit_class = mech_class(mech);
  int movement_type = mech_movement_type(mech);
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if ((!mech_is_started(mech) && !mech_event_count(mech, EVENT_STARTUP))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The 'mech hasn't been started yet!");
    return;
  }
  if (unit_class == CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You snore for a while.. and then _start_ yourself back up.");
    return;
  }
  if (mech_is_dropship(mech) && !mech_is_landed(mech) &&
      !is_wizard(btech_context_database(context), player)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "No shutdowns in mid-air! Are you suicidal?");
    return;
  }
  if (mech_pilot_dbref(mech) == -1)
    return;
  if (mech_event_count(mech, EVENT_STARTUP)) {
    mech_notify(mech, MECHALL, "The startup sequence has been aborted.");
    mech_event_cancel(mech, EVENT_STARTUP);
    mech_pilot_dbref_set(mech, -1);
    return;
  }
  mech_printf(mech, MECHALL, "%s has been shutdown!",
              mech_is_dropship(mech)         ? "Dropship"
              : mech_is_aerospace_unit(mech) ? "Fighter"
              : unit_class == CLASS_BSUIT    ? "Suit"
              : ((movement_type == MOVE_HOVER) ||
                 (movement_type == MOVE_TRACK) || (movement_type == MOVE_WHEEL))
                  ? "Vehicle"
              : movement_type == MOVE_VTOL ? "VTOL"
                                           : "Mech");

  /*
   * Fixed by Kipsta so searchlights shutoff when the mech shuts down
   */

  if (mech_searchlight_active(mech)) {
    mech_notify(mech, MECHALL, "Your searchlight shuts off.");
    mech_searchlight_active_set(mech, false);
    mech_illumination_set(mech, false);
  }

  condition = mech_condition_summary(mech);
  if (condition.torso_right) {
    mech_notify(mech, MECHSTARTED, "Torso rotated back to center for shutdown");
  }
  if (condition.torso_left) {
    mech_notify(mech, MECHSTARTED, "Torso rotated back to center for shutdown");
  }
  mech_torso_twist_set(mech, MECH_TORSO_CENTER);
  if (movement_type != MOVE_NONE && unit_class != CLASS_VEH_NAVAL &&
      ((unit_class == CLASS_MECH && mech_is_jumping(mech)) ||
       (unit_class != CLASS_MECH &&
        mech_position_z(mech) > mech_upper_surface_elevation(mech) &&
        mech_position_z(mech) < ORBIT_Z))) {
    mech_notify(mech, MECHALL, "You start free-fall.. Enjoy the ride!");
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
  } else if (mech_current_speed(mech) > MP1) {
    mech_notify(mech, MECHALL, "Your systems stop in mid-motion!");
    if (unit_class == CLASS_MECH)
      mech_los_broadcast(mech, "stops in mid-motion, and falls!");
    else {
      mech_notify(mech, MECHALL,
                  "You tumble end over end and come to a crashing halt!");
      mech_los_broadcast(mech,
                         "tumbles end over end and comes to a crashing halt!");
    }
    mech_fall(mech, 1, 0);
    mech_domino_resolve(mech, MECH_DOMINO_FALL);
  }
  mech_power_down(mech);
}
