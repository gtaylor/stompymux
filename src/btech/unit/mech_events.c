#include "btech/context.h"
#include "equipment_types.h"
#include "mech_crew_api.h"
#include "mech_targeting_api.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* Implements BattleTech unit mechanics for unit events. */

#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <time.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "checked_conversion.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

#undef WEAPON_RECYCLE_DEBUG

void mech_staggercheck_heartbeat(Mech *mech) {
  time_t now = mech->xcode.context->clock->now;
  int cur_stagger_damage = 0;
  int prev_stagger_damage = 0;
  int stagger_level = 0;

  // if we've not checked stagger since last time... ruhroh!
  if (now - (mech)->rd.last_stagger_check >=
      btech_context_stagger_interval(mech_context(mech))) {
    (mech)->rd.last_stagger_check = now;

    // curStagger is stuff we haven't rolled against
    // prevStagger is stuf we have
    // stuff we have adds to the difficulty, but doesn't get rolled against
    cur_stagger_damage = mech_stagger_damage_current(mech, now);
    prev_stagger_damage = mech_stagger_damage_current_counted(mech, now);
    if (cur_stagger_damage < 20)
      return;
    stagger_level = cur_stagger_damage / 20;

    // Dont need to remove stagger anymore, it clears on fall,
    // unless we're using
    // Stagger mode 2 removes damage after it is checked.
    if (btech_context_stagger_mode(mech_context(mech)) == 2) {
      mech_stagger_damage_remove(mech, stagger_level);
    } else {
      mech_stagger_damage_mark(mech, stagger_level);
      stagger_level = (cur_stagger_damage + prev_stagger_damage) / 20;
    }
    switch (stagger_level) {
    case 1:
      mech_notify(mech, MECHALL,
                  "[fg=yellow bold]The damage causes you to stagger a "
                  "little.[reset]");
      mech_los_broadcast(mech, "stumbles slightly!");
      break;

    case 2:
      mech_notify(mech, MECHALL,
                  "[fg=red]The damage causes you to stagger even more![reset]");
      mech_los_broadcast(mech, "starts to stagger from the damage!");
      break;

    default:
      mech_notify(
          mech, MECHALL,
          "[fg=red bold]The damage causes you to stagger violently while "
          "attempting to keep your footing![reset]");
      mech_los_broadcast(
          mech, "staggers back and forth attempting to keep its footing!");
      break;
    }

    // do the actual staggering here
    mech_notify(mech, MECHALL, "You stagger from the damage!");

    if (!made_pilot_skill_roll(
            mech, mech_stagger_modifier_at_level(mech, stagger_level))) {
      mech_notify(mech, MECHALL,
                  "You loose the battle with gravity and tumble over!!");
      mech_los_broadcast(mech, "tumbles over, staggered by the damage!");
      mech_fall(mech, 1, 0);
    }
  }
}

int mech_stagger_modifier_at_level(Mech *mech, int stagger_level) {
  int bth_mod = 0;
  int tonnage_mod = 0;

  if (!mech_is_started(mech)) {
    bth_mod = 999;
  } else {
    bth_mod = stagger_level - 1;

    if (((mech)->ud.tons) <= 35)
      tonnage_mod = 1;
    else if (((mech)->ud.tons) <= 55)
      tonnage_mod = 0;
    else if (((mech)->ud.tons) <= 75)
      tonnage_mod = -1;
    else
      tonnage_mod = -2;

    // disable tonnage mods if so configured
    if (btech_context_stagger_uses_tonnage(mech_context(mech)))
      bth_mod += tonnage_mod;
  }

  return bth_mod;
}

static int factoral(int n) {
  int i;
  int j = 0;

  for (i = 1; i <= n; i++)
    j += i;
  return j;
}

void mech_standfail_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_notify(mech, MECHALL,
              "[fg=green]You have finally recovered from your attempt to "
              "stand.[reset]");
}

void mech_fall_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long fallspeed = (long)e->data2;
  int fallen_elev;

  if (mech_is_started(mech) && fallspeed >= 0)
    return;
  if (fallspeed <= 0 &&
      (!mech_is_started(mech) || !(mech_is_flying_type(mech)) ||
       ((((mech)->ud.fuel) <= 0) && !mech_aero_has_free_fuel(mech)) ||
       ((((mech)->ud.type) == CLASS_VTOL) &&
        (mech_section_is_destroyed(mech, ROTOR)))))
    fallspeed -= FALL_ACCEL;
  else
    fallspeed += FALL_ACCEL;
  mark_for_los_update(mech);
  if (mech_height_above_surface(mech) > labs(fallspeed)) {
    ((mech)->pd.z) -= labs(fallspeed);
    ((mech)->pd.fz) = ((mech)->pd.z) * ZSCALE;
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK,
                        fallspeed);
    return;
  }
  /* Time to hit da ground */
  long fall_distance = labs(fallspeed);
  fallen_elev = factoral(clamp_intptr_to_int((intptr_t)fall_distance));
  mech_notify(mech, MECHALL, "You hit the ground!");
  mech_los_broadcast(mech, "hits the ground!");
  mech_fall(mech, fallen_elev, 0);
  ((mech)->rd.status) &= ~JUMPING;
}

/* This is just a 'toy' event */
void mech_lock_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *target;

  if (mech_target_dbref(mech) >= 0) {
    target =
        btech_context_find_object(mech->xcode.context, mech_target_dbref(mech));
    if (!target)
      return;
    if (!mech_los_check(mech, target, ((target)->pd.x), ((target)->pd.y),
                        mech_range_to(mech, target)))
      return;
    mech_printf(mech, MECHALL, "The sensors acquire a stable lock on %s.",
                mech_to_mech_display_id(mech, target).text);
  } else if (mech_target_hex_x(mech) >= 0 && mech_target_hex_y(mech) >= 0) {
    mech_printf(mech, MECHALL, "The sensors acquire a stable lock on (%d,%d).",
                mech_target_hex_x(mech), mech_target_hex_y(mech));
  }
}

/* Various events that don't fit too well to other categories */

/* Basically the update events + some movenement events */
void mech_stabilizing_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_notify(mech, MECHSTARTED,
              "[fg=green]You have finally stabilized after your jump.[reset]");
}

void mech_jump_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_event_schedule(mech, EVENT_JUMP, mech_jump_event, JUMP_TICK, 0);
  mech_movement_update(mech);
  if (!mech_is_jumping(mech))
    mech_event_cancel(mech, EVENT_JUMP);
}

void mech_recovery_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_is_destroyed(mech) || !mech_pilot_is_unconscious(mech))
    return;
  if (handlemwconc(mech, 0)) {
    ((mech)->rd.status) &= ~UNCONSCIOUS;
    mech_notify(mech, MECHALL, "The pilot regains consciousness!");
    return;
  }
}

void mech_unconsciousness_extend(Mech *mech, int len) {
  int l;

  if (mech_is_destroyed(mech))
    return;
  if (!mech_event_count(mech, EVENT_RECOVERY)) {
    ((mech)->rd.status) |= UNCONSCIOUS;
    mech_event_schedule(mech, EVENT_RECOVERY, mech_recovery_event, len, 0);
    return;
  }
  l = mux_event_last_type_data(mech->xcode.context->events, EVENT_RECOVERY,
                               (void *)mech) +
      len;
  mux_event_remove_type_data(mech->xcode.context->events, EVENT_RECOVERY,
                             (void *)mech);
  mech_event_schedule(mech, EVENT_RECOVERY, mech_recovery_event, l, 0);
}

#ifdef BT_MOVEMENT_MODES
static void mech_sideslip_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int roll;

  if (!mech || !mech_is_started(mech))
    return;
  mech_notify(mech, MECHALL, "You make a skill roll while sideslipping!");
  if (!made_pilot_skill_roll(mech, has_bool_advantage(mech->xcode.context,
                                                      mech_pilot_dbref(mech),
                                                      "maneuvering_ace")
                                       ? -1
                                       : 0)) {
    mech_notify(mech, MECHALL, "You fail and spin out!");
    mech_los_broadcast(mech, "spins out while sideslipping!");
    ((mech)->rd.speed) = 0.0;
    roll = clamp_intptr_to_int(btech_random_range(mech->xcode.context, 0, 5));
    mech_fall_heading_apply(mech, roll * 60);
    ((mech)->rd.desired_speed) = 0.0;
    mech_lateral_movement_set(mech, 0);
    return;
  }
  mech_event_schedule(mech, EVENT_SIDESLIP, mech_sideslip_event, TURN, 0);
}
#endif

void mech_lateral_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  intptr_t latmode = (intptr_t)e->data2;
  const char *description;
  int offset;

  if (!mech || !mech_is_started(mech))
    return;
  if (!mech_lateral_mode_details(clamp_intptr_to_int(latmode), &description,
                                 &offset))
    return;
  mech_printf(mech, MECHALL,
              "Lateral movement mode change to %s (%d offset) completed.",
              description, offset);
  mech_lateral_movement_set(mech, offset);
#ifdef BT_MOVEMENT_MODES
  if (((mech)->ud.move) != MOVE_QUAD) {
    if (((mech)->rd.lateral) == 0)
      mech_event_cancel(mech, EVENT_SIDESLIP);
    else if (!(mech_event_count(mech, EVENT_SIDESLIP)))
      mech_event_schedule(mech, EVENT_SIDESLIP, mech_sideslip_event, 1, 0);
  }
#endif
}

void mech_move_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (((mech)->ud.type) == CLASS_VTOL)
    if (mech_is_landed(mech) || aero_fuel_check(mech))
      return;
  mech_heading_update(mech);
  if ((is_mech_leg_less(mech)) || mech_is_jumping(mech) ||
      mech_cocoon_integrity(mech)) {
    if (((mech)->rd.desiredfacing) != mech_heading_degrees(mech))
      mech_event_schedule(mech, EVENT_MOVE, mech_move_event, MOVE_TICK, 0);
    return;
  }
  mech_speed_update(mech);
  mech_movement_update(mech);

  if (mech->mapindex < 0)
    return;

  if (((mech)->ud.type) == CLASS_VEH_NAVAL &&
      mech_real_terrain_get(mech) != BRIDGE &&
      mech_real_terrain_get(mech) != ICE &&
      mech_real_terrain_get(mech) != WATER)
    return;

  if (mech->rd.speed != 0.0F || mech->rd.desired_speed != 0.0F ||
      ((mech)->rd.desiredfacing) != mech_heading_degrees(mech) ||
      ((((mech)->ud.type) == CLASS_VTOL || ((mech)->ud.move) == MOVE_SUB) &&
       mech->rd.verticalspeed != 0.0F))
    mech_event_schedule(mech, EVENT_MOVE, mech_move_event, MOVE_TICK, 0);
}

void mech_stand_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_los_broadcast(mech, "stands up!");
  mech_notify(mech, MECHALL, "You have finally finished standing up.");
  mech_make_stand(mech);
}

void mech_plos_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *target;
  BattleMap *map;
  int mapvis;
  int maplight;
  float range;

  if (!mech_is_started(mech))
    return;
  map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  if (!map)
    return;
  mech_event_schedule(mech, EVENT_PLOS, mech_plos_event, PLOS_TICK, 0);
  if (!((mech)->rd.can_see) && !(((mech)->rd.specials) & AA_TECH))
    return;
  mapvis = (unsigned char)map->mapvis;
  maplight = (unsigned char)map->maplight;
  ((mech)->rd.can_see) = 0;
  for (int i = 0; i < battle_map_unit_count(map); i++) {
    DbRef target_dbref = battle_map_unit_dbref(map, i);
    unsigned short los_flags =
        battle_map_los_flags(map, mech_map_slot(mech), i);
    if (target_dbref > 0 && target_dbref != mech_dbref(mech)) {
      if (!(los_flags & BATTLE_MAP_LOS_SEEN)) {
        target = btech_context_find_object(mech->xcode.context, target_dbref);
        if (!target)
          continue;
        range = mech_range_to(mech, target);
        ((mech)->rd.can_see)++;
        MechSensorVisibilityRequest request = {
            .observer = mech,
            .los_flags = los_flags,
            .range = range,
            .x = -1,
            .y = -1,
            .target = target,
            .map_visibility = mapvis,
            .map_light = maplight,
            .cloud_base = battle_map_cloud_base(map),
            .notification_level = 1,
        };
        los_flags = mech_sensor_visibility_update(&request);
        battle_map_los_flags_set(map, mech_map_slot(mech), i, los_flags);
      }
    }
  }
}

void aero_move_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!mech_is_landed(mech)) {
    /* Returns 1 only if we
       1) Ran out of fuel, and
       2) Were VTOL, and
       3) Crashed
     */
    if (aero_fuel_check(mech))
      return;
    /* Genuine CHEAT :-) */
    if (mech_is_started(mech)) {
      aero_heading_update(mech);
      aero_speed_update(mech);
    }
    if (mech_is_fallen(mech))
      ((mech)->rd.startfz) = ((mech)->rd.startfz) - 1;
    mech_movement_update(mech);
    if (mech_is_dropship(mech) &&
        ((mech)->pd.z) <= (mech_position_surface_elevation(mech) + 5) &&
        ((mech->xcode.context->events->tick / WEAPON_TICK) % 10) == 0) {
      dropship_exhaust_blast(&(DropshipExhaustBlastRequest){
          .dropship = mech,
          .direct_message = "You are hit by the DropShip's plasma exhaust!",
          .direct_observer_message = "is hit directly by DropShip's exhaust!",
          .nearby_message = "You are hit by the DropShip's plasma exhaust!",
          .nearby_observer_message = "is hit by DropShip's exhaust!",
          .tree_message = "light up and burn.",
          .damage = 8,
      });
    }
    mech_event_schedule(mech, EVENT_MOVE, aero_move_event, MOVE_TICK, 0);
  } else if (mech_is_landed(mech) && !mech_is_fallen(mech) &&
             mech_is_rolling_aerospace_unit(mech)) {
    mech_heading_update(mech);
    mech_speed_update(mech);
    mech_movement_update(mech);
    if (fabsf(mech->rd.speed) > 0.0F || fabsf(mech->rd.desired_speed) > 0.0F ||
        ((mech)->rd.desiredfacing) != mech_heading_degrees(mech))
      if (!aero_fuel_check(mech))
        mech_event_schedule(mech, EVENT_MOVE, aero_move_event, MOVE_TICK, 0);
  }
}

void mech_event_failure_marker(MuxEvent *event) {}

/*
 * Exile Stun Code Event
 */
void mech_crewstun_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!mech)
    return;
  if (!mech_is_started(mech) || mech_is_destroyed(mech)) {
    if (((mech)->rd.critstatus) & MECH_STUNNED)
      ((mech)->rd.critstatus) &= ~MECH_STUNNED;
    return;
  }
  if (((mech)->ud.type) != CLASS_MECH)
    mech_notify(
        mech, MECHALL,
        "[fg=green bold]The crew recovers from their bewilderment![reset]");
  else
    mech_notify(
        mech, MECHALL,
        "[fg=green bold]You recover from your stunning experience![reset]");

  if (((mech)->rd.critstatus) & MECH_STUNNED)
    ((mech)->rd.critstatus) &= ~MECH_STUNNED;
}

void unstun_crew_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_event_count(mech, EVENT_UNSTUN_CREW) >
      1) /* If we've been stunned again! */
    return;

  mech_notify(
      mech, MECHALL,
      "Your head clears and you're able to control your vehicle again.");
  ((mech)->rd.tankcritstatus) &= ~CREW_STUNNED;
}

void mech_unjam_ammo_event(MuxEvent *obj_event) {
  Mech *obj_mech = (Mech *)obj_event->data; /* get the mech */
  int w_weap_num =
      clamp_intptr_to_int((intptr_t)obj_event->data2); /* weapon number */
  int w_sect;
  int w_slot;
  int w_weap_status;
  int w_weap_idx;
  int w_roll = 0;
  int w_roll_needed = 0;

  if (mech_pilot_is_unconscious(obj_mech) || !mech_is_started(obj_mech))
    return;

  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = obj_mech, .number = w_weap_num});
  w_weap_status = lookup.value;
  w_sect = lookup.slot.section;
  w_slot = lookup.slot.critical;

  if (w_weap_status ==
      TIC_NUM_DESTROYED) /* return if the weapon has been destroyed */
    return;

  w_weap_idx = find_weapon_index(obj_mech, w_weap_num);

  AmmunitionCheckResult ammunition = ammunition_check(&(AmmunitionCheckRequest){
      .mech = obj_mech,
      .weapon_index = w_weap_idx,
      .weapon = {.section = w_sect, .critical = w_slot}});
  if (!ammunition.available) {
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = obj_mech,
        .slot = {.section = w_sect, .critical = w_slot},
        .failure = 0});

    mech_printf(obj_mech, MECHALL,
                "You finish bouncing around and realize you no longer have "
                "ammo for your %s!",
                get_parts_long_name(obj_mech->xcode.context,
                                    weapon_equipment_index(w_weap_idx), 0));
    return;
  }

  if (weapon_catalogue_has_special(w_weap_status, RAC)) {
    w_roll = btech_random_roll(obj_mech->xcode.context);
    w_roll_needed = find_pilot_gunnery(obj_mech, w_weap_status) + 3;

    mech_notify(obj_mech, MECHPILOT, "You make a roll to unjam the weapon!");
    mech_printf(obj_mech, MECHPILOT, "Modified Gunnery Skill: BTH %d\tRoll: %d",
                w_roll_needed, w_roll);

    if (w_roll < w_roll_needed) {
      mech_notify(obj_mech, MECHALL,
                  "Your attempt to remove the jammed slug fails. You'll need "
                  "to try again to clear it.");
      return;
    }
  } else {
    if (!made_pilot_skill_roll(obj_mech, 0)) {
      mech_notify(obj_mech, MECHALL,
                  "Your attempt to remove the jammed slug fails. You'll need "
                  "to try again to clear it.");
      return;
    }
  }

  mech_critical_temporary_failure_set(
      &(CriticalSlotFailureSet){.mech = obj_mech,
                                .slot = {.section = w_sect, .critical = w_slot},
                                .failure = 0});
  mech_printf(obj_mech, MECHALL, "You manage to clear the jam on your %s!",
              get_parts_long_name(obj_mech->xcode.context,
                                  weapon_equipment_index(w_weap_idx), 0));
  mech_los_broadcast(obj_mech, "ejects a mangled shell!");

  mech_ammunition_decrement(&(AmmunitionDecrementRequest){
      .mech = obj_mech,
      .weapon_index = w_weap_num,
      .weapon = {.section = w_sect, .critical = w_slot},
      .primary_ammunition = ammunition.primary,
      .secondary_ammunition = ammunition.secondary,
      .gatling_shots = 0,
  });
}

void check_stagger_event(MuxEvent *event) {
  Mech *mech = (Mech *)event->data; /* get the mech */

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Triggered stagger check for %ld.", mech->mynum));

  if ((mech_stagger_level(mech) < 1) || mech_is_fallen(mech) ||
      (((mech)->ud.type) != CLASS_MECH)) {
    mech_stop_stagger_check(mech);
    return;
  }

  if (mech_is_jumping(mech)) {
    return;
  }

  mech_notify(mech, MECHALL, "You stagger from the damage!");
  if (!made_pilot_skill_roll(mech, mech_stagger_modifier(mech))) {
    mech_notify(mech, MECHALL,
                "You loose the battle with gravity and tumble over!!");
    mech_los_broadcast(mech, "tumbles over, staggered by the damage!");
    mech_fall(mech, 1, 0);
  }

  mech_stop_stagger_check(mech);
  /* Since stagger rolls happen much more often now, this adds 10 damage
   * points of 'buffer' to mech that was just forced to make a stager roll.
   * Mechs whose damage accumulation times out without making a roll (<20
   * damage) don't get this help. This 10 points of damage assistance slowly
   * times out in mech_damage_stagger_check, or can be erased by weapons fire */
  mech->rd.stagger_damage = -10;
}

#ifdef BT_MOVEMENT_MODES
void mech_movemode_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long i = (long)e->data2;
  int dir = (i & MODE_ON) != 0;

  if (!mech)
    return;

  if (!mech_is_started(mech) || mech_is_destroyed(mech)) {
    ((mech)->rd.status2) &= ~(MOVE_MODES);
    return;
  }
  if (dir) {
    if (i & MODE_EVADE) {
      ((mech)->rd.status2) |= EVADING;
      mech_notify(mech, MECHALL,
                  "You bounce chaotically as you maximize your movement mode "
                  "to evade!");
      mech_los_broadcast(
          mech,
          "suddenly begins to move erratically performing evasive maneuvers!");
    } else if (i & MODE_SPRINT) {
      ((mech)->rd.status2) |= SPRINTING;
      mech_notify(mech, MECHALL,
                  "You shimmy side to side as you get more speed from your "
                  "movement mode.");
      if ((((mech)->ud.type) == CLASS_MECH) ||
          (((mech)->ud.type) == CLASS_BSUIT))
        mech_los_broadcast(mech, "breaks out into a full blown stride as it "
                                 "sprints over the terrain!");
      else
        mech_los_broadcast(mech,
                           "shifts into high gear as it gains more speed!");
      if (((mech)->rd.speed) < 0) {
        mech_notify(mech, MECHALL,
                    "You stop your backward momemtum while sprinting and come "
                    "to a stop!");
        ((mech)->rd.desired_speed) = 0;
      }
    } else if (i & MODE_DODGE) {
      if (mech_recycling_state(mech, CHECK_PHYS) > 0) {
        mech_notify(mech, MECHALL,
                    "You cannot enter DODGE mode due to physical useage.");
        return;
      }
      ((mech)->rd.status2) |= DODGING;
      mech_notify(mech, MECHALL,
                  "You brace yourself for any oncoming physicals.");
    }
  } else {
    if (i & MODE_EVADE) {
      ((mech)->rd.status2) &= ~EVADING;
      mech_notify(
          mech, MECHALL,
          "Cockpit movement normalizes as you cease your evasive maneuvers.");
      mech_los_broadcast(mech, "ceases its evasive behavior and calms down.");
    } else if (i & MODE_SPRINT) {
      ((mech)->rd.status2) &= ~SPRINTING;
      mech_notify(mech, MECHALL,
                  "You feel less seasick as you leave your sprint mode and "
                  "resume normal movement.");
      mech_los_broadcast(mech, "slows down and enters a normal movement mode.");
    } else if (i & MODE_DODGE) {
      ((mech)->rd.status2) &= ~DODGING;
      if (i & MODE_DG_USED)
        mech_notify(mech, MECHALL,
                    "Your dodge maneuver has been used and you are no longer "
                    "braced for physicals.");
      else
        mech_notify(mech, MECHALL,
                    "You loosen up your stance and no longer dodge physicals.");
    }
  }
  if (((mech)->rd.speed) > mech_effective_maximum_speed(mech) ||
      ((mech)->rd.desired_speed) > mech_effective_maximum_speed(mech))
    ((mech)->rd.desired_speed) = mech_effective_maximum_speed(mech);
}
#endif

int mech_stagger_modifier(Mech *mech) {
  int bth_mod = 0;
  int tonnage_mod = 0;

  if (!mech_is_started(mech)) {
    bth_mod = 999;
  } else {
    bth_mod = mech_stagger_level(mech);

    if (((mech)->ud.tons) <= 35)
      tonnage_mod = 1;
    else if (((mech)->ud.tons) <= 55)
      tonnage_mod = 0;
    else if (((mech)->ud.tons) <= 75)
      tonnage_mod = -1;
    else
      tonnage_mod = -2;

    bth_mod += tonnage_mod;
  }

  return bth_mod;
}
