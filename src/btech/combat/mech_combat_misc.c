/* Implements BattleTech combat mechanics for unit combat misc. */

#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_fire_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

void mech_ammunition_expenditure_check(
    const AmmunitionExpenditureCheck *check) {
  Mech *mech = check->mech;
  const int WEAPINDX = check->weapon_index;
  const int NS = check->rounds_remaining;
  int targ = ammunition_equipment_index(WEAPINDX);
  int cnt = 0;
  float slots = 0.0F;
  int t, t2;
  int i, j, cl;
  int sev = 0;

  mech_weight_cache_invalidate(mech);

  if (!mech_ammunition_warning_enabled(mech))
    return;

  for (i = 0; i < NUM_SECTIONS; i++) {
    cl = mech_section_critical_count(mech, i);
    for (j = 0; j < cl; j++)
      if (mech_critical_part_type(mech, i, j) == targ) {
        cnt += mech_critical_data(mech, i, j);
        slots += mech_ammunition_slot_multiplier(mech, i, j);
      }
  }
  const int AMMUNITION_PER_TON = weapon_catalogue_ammunition_per_ton(WEAPINDX);
  t = bounded(3, (int)(slots * (float)AMMUNITION_PER_TON / 8.0F), 30);
  t2 = 2 * t;
  if ((cnt == (t + NS)) || (NS && cnt >= t && cnt < (t + NS)))
    sev = 1;
  else if ((cnt == (t2 + NS)) || (NS && cnt >= t2 && cnt < (t2 + NS)))
    sev = 0;
  else
    return;
  /* Okay, we have case of warning here */
  if (mech_is_started(mech))
    if ((sev * 65536 + WEAPINDX) % 65536)
      mech_printf(mech, MECHALL,
                  "%sWARNING: Ammo for %s is running low.[reset]",
                  sev ? "[fg=red bold]" : "[fg=yellow bold]",
                  get_parts_long_name(mech_context(mech),
                                      weapon_equipment_index(WEAPINDX), 0));
}

void mech_heat_effect_apply(Mech *mech, Mech *temp_mech, int heatdam,
                            bool from_inferno) {
  if (mech_class(temp_mech) != CLASS_MECH &&
      mech_class(temp_mech) != CLASS_MW &&
      mech_class(temp_mech) != CLASS_BSUIT && !mech_is_dropship(temp_mech) &&
      mech_movement_type(temp_mech) != MOVE_NONE) {

    if ((mech_class(temp_mech) == CLASS_VEH_GROUND ||
         mech_class(temp_mech) == CLASS_VTOL) &&
        btech_context_uses_advanced_vehicle_fire(mech_context(temp_mech))) {
      if (from_inferno)
        vehicle_fire_start(temp_mech, mech);
      else
        vehicle_fire_check(temp_mech, 0);
    } else {
      if (btech_random_roll(mech_context(temp_mech)) > 8) {
        mech_los_broadcast(temp_mech, "explodes!");
        mech_notify(temp_mech, MECHALL,
                    "The heat's too much for your vehicle! It blows up!");
        mech_mark_destroyed(temp_mech);
        channel_emit_kill(temp_mech, mech, KILL_TYPE_HEAT);
        mech_explosion_apply(temp_mech, mech ? mech : temp_mech);
      }
    }
  } else {

    if (heatdam)
      mech_inferno_burn(temp_mech, heatdam * 6);
  }
}

/* Burn.. burn in hell! ;> */
void mech_inferno_hit(Mech *mech, Mech *hit_mech, int missiles, bool los) {
  int hmod = (missiles + 1) / 2;

  if (mech_is_jellied(hit_mech) ||
      mech_event_count(hit_mech, EVENT_VEHICLEBURN)) {
    mech_los_broadcast(hit_mech, "burns a bit more brightly.");
    mech_notify(hit_mech, MECHALL,
                "[fg=red bold]More burning jelly joins the flames![reset]");
  } else {
    mech_los_broadcast(hit_mech, "suddenly bursts into flames!");
    mech_notify(hit_mech, MECHALL,
                "[fg=red bold]You are sprayed with burning jelly![reset]");
  }
  mech_heat_effect_apply(mech, hit_mech, hmod * 30,
                         1); /* 3min for _each_ missile */
  mech_inferno_extinguish_in_water(
      hit_mech); /* They could be in -2 standing or -1 prone.. Shooter just
                   wastes his missiles! */
}

void mech_plasma_hit(Mech *hit_mech) {
  /* For now, lets just worry about IS.PlasmaRifles
   * They are 1D6 Heat to mechs and 2D6 damage to anything else (clustered into
   * 5) We'll handle the cluster damage in HitMech
   */

  float heatadd = 0;

  if (mech_class(hit_mech) == CLASS_MECH) {
    const int HEAT_ROLL = btech_random_range_int(mech_context(hit_mech), 1, 6);
    heatadd = (float)HEAT_ROLL;
    mech_weapon_heat_add(hit_mech, heatadd);
  }
}

// extern int global_kill_cheat;
void mech_contents_kill_if_in_character(Mech *mech) {
  BtechContext *context = mech_context(mech);
  GameDatabase *database = btech_context_database(context);
  DbRef mech_ref = mech_dbref(mech);

  // global_kill_cheat = 1;
  if (!is_in_character(database, mech_ref))
    return;
  if (!btech_context_in_character_enabled(context) ||
      btech_context_experience_loss(context) >= 1000)
    contents_teleport(&(ContentsTeleportRequest){
        .context = context,
        .source = mech_ref,
        .destination = btech_context_afterlife_dbref(context),
        .options = TELE_LOUD,
    });
  else
    contents_teleport(&(ContentsTeleportRequest){
        .context = context,
        .source = mech_ref,
        .destination = btech_context_afterlife_dbref(context),
        .options = TELE_XP | TELE_LOUD,
    });
}

enum { BOOM_LENGTH = 24 };
static const char BOOM[BOOM_LENGTH][80] = {
    "                              ________________",
    "                         ____/ (  (    )   )  \\___",
    "                        /( (  (  )   _    ))  )   )\\",
    "                      ((     (   )(    )  )   (   )  )",
    "                    ((/  ( _(   )   (   _) ) (  () )  )",
    "                   ( (  ( (_)   ((    (   )  .((_ ) .  )_",
    "                  ( (  )    (      (  )    )   ) . ) (   )",
    "                 (  (   (  (   ) (  _  ( _) ).  ) . ) ) ( )",
    "                 ( (  (   ) (  )   (  ))     ) _)(   )  )  )",
    "                ( (  ( \\ ) (    (_  ( ) ( )  )   ) )  )) ( )",
    "                 (  (   (  (   (_ ( ) ( _    )  ) (  )  )   )",
    "                ( (  ( (  (  )     (_  )  ) )  _)   ) _( ( )",
    "                 ((  (   )(    (     _    )   _) _(_ (  (_ )",
    "                  (_((__(_(__(( ( ( |  ) ) ) )_))__))_)___)",
    "                  ((__)        \\\\||lll|l||///          \\_))",
    "                           (   /(/ (  )  ) )\\   )",
    "                         (    ( ( ( | | ) ) )\\   )",
    "                          (   /(| / ( )) ) ) )) )",
    "                        (     ( ((((_(|)_)))))     )",
    "                         (      ||\\(|(|)|/||     )",
    "                       (        |(||(||)||||        )",
    "                         (     //|/l|||)|\\\\ \\     )",
    "                       (/ / //  /|//||||\\\\  \\ \\  \\ _)",
    // clang-format off
    "----------------------------------------------------------------------------"};
// clang-format on

void mech_destroy(Mech *target, Mech *mech, bool showboom, const char *reason) {
  BtechContext *context = mech_context(target);
  GameDatabase *database = btech_context_database(context);
  int loop;
  BattleMap *mech_map;
  Mech *ttarget;
  Mech *ctarget;

  DbRef a, b;

  if (mech_is_destroyed(target)) {
    if (strncmp(reason, KILL_TYPE_HEAD_TARGET, strlen(KILL_TYPE_HEAD_TARGET)) ==
        0)
      /* Need some logic in here to detect for beheadings 'after the fact' */
      /* I.e. Mechas that got engine flooded, or XL death */
      /* For now, just exit */
      return;
    return;
  }
  // global_kill_cheat = 1;

  // Destroy Contents Right Away
  if (btech_context_transported_unit_death_enabled(context)) {
    SAFE_DOLIST(database, a, b,
                game_object_contents(database, mech_dbref(target)))
    if (btech_context_is_mech(context, a) && is_in_character(database, a)) {
      ctarget = btech_context_get_mech(context, a);
      mech_notify(
          ctarget, MECHALL,
          "Due to your transport's destruction, your unit has been destroyed!");
      mech_udisembark(a, ctarget, "");
      mech_destroy(ctarget, mech, 1, KILL_TYPE_TRANSPORT);
    }
  }

  if (mech && target)
    channel_emit_kill(target, mech, reason);
  else
    channel_emit_kill(target, target, reason);
  if (mech) {

    if (mech != target) {
      mech_notify(mech, MECHALL, "You destroyed the target!");
      mech_los_broadcast_unit(target, mech, "has been destroyed by %s!");
    } else
      mech_los_broadcast(target, "has been destroyed!");
    if (showboom) {
      for (loop = 0; loop < BOOM_LENGTH; loop++) {
        const char *const *message =
            (const char *const *)checked_storage_at_const(
                BOOM, BOOM_LENGTH, sizeof(*BOOM), (size_t)loop);
        mech_notify(target, MECHALL, *message);
      }
    }
    switch (mech_class(target)) {
    case CLASS_MW:
    case CLASS_BSUIT:
      mech_notify(target, MECHALL, "You have been killed!");
      break;
    case CLASS_MECH:
    case CLASS_VEH_GROUND:
    case CLASS_VTOL:
    case CLASS_VEH_NAVAL:
    case CLASS_SPHEROID_DS:
    case CLASS_AERO:
    case CLASS_DS:
    default:
      mech_notify(target, MECHALL, "You have been destroyed!");
      break;
    }
    if (mech_map_dbref(target) != -1) {
      mech_map = btech_context_get_map(context, mech_map_dbref(target));
      if (btech_context_vtol_ice_fire_enabled(context) &&
          (mech_technology_flags(target) & ICE_TECH) &&
          mech_class(target) == CLASS_VTOL) {
        mech_los_broadcast(target, "explodes in a ball of flames!");
        add_decoration(&(MapDecorationRequest){
            .map = mech_map,
            .position = {.x = mech_position_x(target),
                         .y = mech_position_y(target)},
            .type = TYPE_FIRE,
            .terrain_marker = FIRE,
            .duration = btech_random_range_int(context, 60, 180),
        });
      }
    }
    if (mech_carried_dbref(target) > 0) {
      ttarget = btech_context_get_mech(context, mech_carried_dbref(target));
      if (ttarget) {
        mech_notify(ttarget, MECHALL, "Your tow lines go suddenly slack!");
        mech_dropoff(GOD, target, "");
      }
    }
  }

  /* shut it down */
  if (mech) {
    mech_destroy_and_place(target);
  } else {
    mech_mark_destroyed(target);
  }
  if (mech_class(target) == CLASS_MW) {
    if (is_in_character(database, mech_dbref(target))) {
      if (btech_context_mechwarrior_experience_loss_enabled(context)) {
        mech_contents_kill_if_in_character(target);
      } else {
        contents_teleport(&(ContentsTeleportRequest){
            .context = context,
            .source = mech_dbref(target),
            .destination = btech_context_afterlife_dbref(context),
            .options = TELE_LOUD,
        });
      }
      discard_mw(target);
    }
  }
}

const char *mech_hex_target_short_name(const Mech *mech) {

  if (mech_targets_hex_for_ignition(mech))
    return "ign";
  if (mech_targets_hex_for_clearing(mech))
    return "clr";
  if (mech_targets_hex(mech))
    return "hex";
  if (mech_targets_building(mech))
    return "bld";
  return "reg";
}
