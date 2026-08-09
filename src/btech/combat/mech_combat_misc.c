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

void mech_ammunition_decrement(Mech *mech, int weapindx, int section,
                               int critical, int ammoLoc, int ammoCrit,
                               int ammoLoc1, int ammoCrit1,
                               int wGattlingShots) {
  int wGatSec = 0, wGatCrit = 0;
  int wShotsLeft = 0;
  int wCurShots = 0;
  int i = 0;
  int weapSize = 0;
  int firstCrit = 0;

  /* If we're an energy weapon or a PC weapon, return */
  if (weapon_catalogue_is_energy(weapindx) ||
      weapon_catalogue_is_hand_to_hand(weapindx))
    return;

  /* If we're a rocket launcher, fire our load and return */
  if (weapon_catalogue_is_only_rocket(weapindx)) {
    weapSize = GetWeaponCrits(mech, weapindx);
    firstCrit = FindFirstWeaponCrit(
        mech, section, critical, 0,
        mech_critical_part_type(mech, section, critical), weapSize);

    for (i = firstCrit; i < (firstCrit + weapSize); i++) {
      mech_critical_fire_mode_add(mech, section, i, ROCKET_FIRED);
    }

    return;
  }

  /* If we're a one-shot, set us used and return */
  if (mech_critical_fire_mode(mech, section, critical) & OS_MODE) {
    mech_critical_fire_mode_add(mech, section, critical, OS_USED);
    return;
  }
  /* Check the state of our weapon bins */
  mech_ammunition_expenditure_check(
      mech, weapindx,
      MAX(wGattlingShots,
          ((mech_critical_fire_mode(mech, section, critical) & ULTRA_MODE) ||
           (mech_critical_fire_mode(mech, section, critical) & RFAC_MODE))));

  if ((mech_critical_fire_mode(mech, section, critical) & GATTLING_MODE) ||
      weapon_catalogue_is_rotary_autocannon(weapindx)) {
    if (mech_critical_fire_mode(mech, section, critical) & GATTLING_MODE)
      wShotsLeft = wGattlingShots * 3;
    else {
      if (mech_critical_fire_mode(mech, section, critical) & RAC_TWOSHOT_MODE)
        wShotsLeft = 2;
      else if (mech_critical_fire_mode(mech, section, critical) &
               RAC_FOURSHOT_MODE)
        wShotsLeft = 4;
      else if (mech_critical_fire_mode(mech, section, critical) &
               RAC_SIXSHOT_MODE)
        wShotsLeft = 6;
      else
        wShotsLeft = 1;
    }

    while (wShotsLeft > 0) {
      FindAmmoForWeapon_sub(mech, section, critical, weapindx, section,
                            &wGatSec, &wGatCrit, AMMO_MODES, 0);
      wCurShots = mech_critical_data(mech, wGatSec, wGatCrit);

      if (wCurShots) {
        if (wCurShots >= wShotsLeft) {
          mech_critical_data_set(mech, wGatSec, wGatCrit,
                                 wCurShots - wShotsLeft);
          wShotsLeft = 0;
        } else {
          mech_critical_data_set(mech, wGatSec, wGatCrit, 0);
          wShotsLeft -= wCurShots;
        }
      }

      if (CountAmmoForWeapon(mech, weapindx) <= 0)
        break;
    }
  } else { /* Non-RAC/Gattling */
    /* Decrement our ammo one shot */
    if (mech_critical_data(mech, ammoLoc, ammoCrit))
      mech_critical_data_set(mech, ammoLoc, ammoCrit,
                             mech_critical_data(mech, ammoLoc, ammoCrit) - 1);

    /* If we're ultra or rfac, decrement it again */
    if ((mech_critical_fire_mode(mech, section, critical) & ULTRA_MODE) ||
        (mech_critical_fire_mode(mech, section, critical) & RFAC_MODE))
      if (mech_critical_data(mech, ammoLoc1, ammoCrit1))
        mech_critical_data_set(mech, ammoLoc1, ammoCrit1,
                               mech_critical_data(mech, ammoLoc1, ammoCrit1) -
                                   1);
  }
}

void mech_ammunition_expenditure_check(Mech *mech, int weapindx, int ns) {
  int targ = ammunition_equipment_index(weapindx);
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
  const int ammunition_per_ton = weapon_catalogue_ammunition_per_ton(weapindx);
  t = BOUNDED(3, (int)(slots * (float)ammunition_per_ton / 8.0F), 30);
  t2 = 2 * t;
  if ((cnt == (t + ns)) || (ns && cnt >= t && cnt < (t + ns)))
    sev = 1;
  else if ((cnt == (t2 + ns)) || (ns && cnt >= t2 && cnt < (t2 + ns)))
    sev = 0;
  else
    return;
  /* Okay, we have case of warning here */
  if (mech_is_started(mech))
    if ((sev * 65536 + weapindx) % 65536)
      mech_printf(mech, MECHALL,
                  "%sWARNING: Ammo for %s is running low.[reset]",
                  sev ? "[fg=red bold]" : "[fg=yellow bold]",
                  get_parts_long_name(mech_context(mech),
                                      weapon_equipment_index(weapindx), 0));
}

void mech_heat_effect_apply(Mech *mech, Mech *tempMech, int heatdam,
                            bool fromInferno) {
  if (mech_class(tempMech) != CLASS_MECH && mech_class(tempMech) != CLASS_MW &&
      mech_class(tempMech) != CLASS_BSUIT && !mech_is_dropship(tempMech) &&
      mech_movement_type(tempMech) != MOVE_NONE) {

    if ((mech_class(tempMech) == CLASS_VEH_GROUND ||
         mech_class(tempMech) == CLASS_VTOL) &&
        btech_context_uses_advanced_vehicle_fire(mech_context(tempMech))) {
      if (fromInferno)
        vehicle_fire_start(tempMech, mech);
      else
        vehicle_fire_check(tempMech, 0);
    } else {
      if (btech_random_roll(mech_context(tempMech)) > 8) {
        mech_los_broadcast(tempMech, "explodes!");
        mech_notify(tempMech, MECHALL,
                    "The heat's too much for your vehicle! It blows up!");
        mech_mark_destroyed(tempMech);
        ChannelEmitKill(tempMech, mech, KILL_TYPE_HEAT);
        mech_explosion_apply(tempMech, mech ? mech : tempMech);
      }
    }
  } else {

    if (heatdam)
      mech_inferno_burn(tempMech, heatdam * 6);
  }
}

/* Burn.. burn in hell! ;> */
void mech_inferno_hit(Mech *mech, Mech *hitMech, int missiles, bool LOS) {
  int hmod = (missiles + 1) / 2;

  if (mech_is_jellied(hitMech) ||
      mech_event_count(hitMech, EVENT_VEHICLEBURN)) {
    mech_los_broadcast(hitMech, "burns a bit more brightly.");
    mech_notify(hitMech, MECHALL,
                "[fg=red bold]More burning jelly joins the flames![reset]");
  } else {
    mech_los_broadcast(hitMech, "suddenly bursts into flames!");
    mech_notify(hitMech, MECHALL,
                "[fg=red bold]You are sprayed with burning jelly![reset]");
  }
  mech_heat_effect_apply(mech, hitMech, hmod * 30,
                         1); /* 3min for _each_ missile */
  mech_inferno_extinguish_in_water(
      hitMech); /* They could be in -2 standing or -1 prone.. Shooter just
                   wastes his missiles! */
}

void mech_plasma_hit(Mech *mech, Mech *hitMech, bool LOS) {
  /* For now, lets just worry about IS.PlasmaRifles
   * They are 1D6 Heat to mechs and 2D6 damage to anything else (clustered into
   * 5) We'll handle the cluster damage in HitMech
   */

  float heatadd = 0;

  if (mech_class(hitMech) == CLASS_MECH) {
    const int heat_roll = btech_random_range_int(mech_context(hitMech), 1, 6);
    heatadd = (float)heat_roll;
    mech_weapon_heat_add(hitMech, heatadd);
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
    tele_contents(context, mech_ref, btech_context_afterlife_dbref(context),
                  TELE_LOUD);
  else
    tele_contents(context, mech_ref, btech_context_afterlife_dbref(context),
                  TELE_XP | TELE_LOUD);
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
    ChannelEmitKill(target, mech, reason);
  else
    ChannelEmitKill(target, target, reason);
  if (mech) {

    if (mech != target) {
      mech_notify(mech, MECHALL, "You destroyed the target!");
      mech_los_broadcast_unit(target, mech, "has been destroyed by %s!");
    } else
      mech_los_broadcast(target, "has been destroyed!");
    if (showboom) {
      for (loop = 0; loop < BOOM_LENGTH; loop++) {
        const char *const *message = checked_storage_at_const(
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
        add_decoration(mech_map, mech_position_x(target),
                       mech_position_y(target), TYPE_FIRE, FIRE,
                       btech_random_range_int(context, 60, 180));
      }
    }
    if (mech_carried_dbref(target) > 0) {
      if ((ttarget =
               btech_context_get_mech(context, mech_carried_dbref(target)))) {
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
        tele_contents(context, mech_dbref(target),
                      btech_context_afterlife_dbref(context), TELE_LOUD);
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
