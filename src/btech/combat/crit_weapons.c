/* Resolves weapon critical hits. */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_api_types.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static int *critical_slot(int *criticals, size_t count, size_t index) {
  return checked_storage_at(criticals, count, sizeof(*criticals), index);
}

static unsigned char *weapon_slot(unsigned char *weapons, size_t count,
                                  size_t index) {
  return checked_storage_at(weapons, count, sizeof(*weapons), index);
}

static const char *weapon_display_name(int weapon_index) {
  return checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
}

int mech_weapon_critical_handle(const WeaponCriticalRequest *request) {
  Mech *attacker = request->attacker;
  Mech *wounded = request->wounded;
  const int hitloc = request->slot.section;
  const int critHit = request->slot.critical;
  const int critType = request->part_type;
  int wMaxCrits, wFirstCrit, wWeapDestroyed = 0;
  int damage;
  char locname[30];
  char msgbuf[MBUF_SIZE] = {0};
  BtechContext *context = mech_context(wounded);

  ArmorStringFromIndex(hitloc, locname, mech_class(wounded),
                       mech_movement_type(wounded));

  /* Get the max number of crits for this weapon */
  wMaxCrits = GetWeaponCrits(wounded, weapon_from_equipment_index(critType));

  /* Find the first crit */
  wFirstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = wounded,
      .weapon = {.section = hitloc, .critical = critHit},
      .start_critical = 0,
      .part_type = critType,
      .maximum_criticals = wMaxCrits,
  });

  /* See if the weapon is already destroyed */
  if (wFirstCrit != -1) {
    wWeapDestroyed =
        (mech_critical_is_nonfunctional(wounded, hitloc, wFirstCrit) ||
         (mech_critical_temporary_failure(wounded, hitloc, wFirstCrit) ==
          FAIL_DESTROYED));
  }

  /* Gauss rifle-ish weapons explode when critted */
  const int weapon_index = weapon_from_equipment_index(critType);
  if (weapon_catalogue_is_gauss(weapon_index) && !wWeapDestroyed) {
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                weapon_display_name(weapon_index));

    mech_printf(wounded, MECHALL, "It explodes for %d points damage.",
                weapon_catalogue_explosion_damage(weapon_index));

    if (!mech_is_destroyed(wounded)) {
      (void)snprintf(msgbuf, MBUF_SIZE,
                     "'s %s is covered in a large electrical discharge!",
                     locname);
      mech_los_broadcast(wounded, msgbuf);
    }

    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = wounded,
        .first = {.section = hitloc, .critical = wFirstCrit},
        .part_type = critType,
        .criticals_to_destroy = wMaxCrits,
        .total_criticals = wMaxCrits});

    if (attacker) {
      mech_damage_apply(&(MechDamageRequest){
          .target = wounded,
          .attacker = attacker,
          .line_of_sight = 0,
          .attack_pilot = -1,
          .hit_location = hitloc,
          .rear = 0,
          .critical = 0,
          .armor_damage = 0,
          .internal_damage = weapon_catalogue_explosion_damage(weapon_index),
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = -1,
          .base_to_hit = 7,
          .weapon_index = -1,
          .ammunition_mode = 0,
          .ignore_swarmers = 1});
    }
    /* Rule Reference: BMR Revised, Page 16-17 (Ammo Explosion=2 Bruise) */
    /* Rule Reference: Total Warfare, Page 41 (Ammo Explosion=2 Bruise) */

    if (mech_class(wounded) != CLASS_BSUIT) {
      mech_notify(wounded, MECHPILOT,
                  "You take personal injury from the weapon's explosion!");

      /* Rule Reference: MaxTech Revised, Page 46 (Reduce by 1 because of pain
       * resistance) */

      if (HasBoolAdvantage(context, mech_pilot_dbref(wounded),
                           "pain_resistance"))
        headhitmwdamage(wounded, wounded, 1);
      else
        headhitmwdamage(wounded, wounded, 2);
    }
    return 1;
  } else if (weapon_catalogue_is_anti_missile(
                 weapon_from_equipment_index(critType))) { /* Have to shut down
                                           AMS when its critted */
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                weapon_display_name(weapon_index));

    mech_ams_enabled_set(wounded, false);
    mech_technology_flags_remove(wounded,
                                 IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH);
  } else if (weapon_catalogue_is_hot_loaded(
                 weapon_from_equipment_index(critType),
                 mech_critical_fire_mode(wounded, hitloc, wFirstCrit)) &&
             !wWeapDestroyed) { /* And crit hotloaded LRMs */
    CriticalSlotLookupResult ammunition =
        ammunition_find(&(AmmunitionLookupRequest){
            .mech = wounded,
            .weapon_index = weapon_from_equipment_index(critType),
            .forbidden_modes = AMMO_MODES});
    if (ammunition.found) {
      damage = weapon_catalogue_damage(weapon_index);

      if (weapon_catalogue_is_missile(weapon_from_equipment_index(critType)) ||
          weapon_catalogue_is_artillery(
              weapon_from_equipment_index(critType))) {
        int missile_count = btech_context_missile_hit_count(&(MissileHitLookup){
            .context = context,
            .weapon = weapon_from_equipment_index(critType),
            .roll = 10,
        });
        if (missile_count > 0) {
          damage *= missile_count;
        }
      }

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  weapon_display_name(weapon_index));

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]Your hotloaded launcher explodes for %d points of "
          "damage![reset]",
          damage);

      if (!mech_is_destroyed(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE,
                       " loses a launcher in a brilliant explosion!");
        mech_los_broadcast(wounded, msgbuf);
      }

      mech_weapon_destroy(&(WeaponDestructionRequest){
          .mech = wounded,
          .first = {.section = hitloc, .critical = wFirstCrit},
          .part_type = critType,
          .criticals_to_destroy = wMaxCrits,
          .total_criticals = wMaxCrits});

      if (attacker) {
        mech_damage_apply(&(MechDamageRequest){.target = wounded,
                                               .attacker = attacker,
                                               .line_of_sight = 0,
                                               .attack_pilot = -1,
                                               .hit_location = hitloc,
                                               .rear = 0,
                                               .critical = 0,
                                               .armor_damage = 0,
                                               .internal_damage = damage,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = -1,
                                               .base_to_hit = 7,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = 1});
      }

      return 1;
    }
  } else if ((mech_critical_ammo_mode(wounded, hitloc, wFirstCrit) &
              AC_INCENDIARY_MODE) &&
             !wWeapDestroyed &&
             mech_weapon_is_recycling_at(
                 wounded, hitloc,
                 wFirstCrit)) { /* Incendiary ACs blow up too */

    CriticalSlotLookupResult ammunition =
        ammunition_find(&(AmmunitionLookupRequest){
            .mech = wounded,
            .weapon_index = weapon_from_equipment_index(critType),
            .required_modes = AC_INCENDIARY_MODE});
    if (ammunition.found) {

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  weapon_display_name(weapon_index));

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]The incendiary ammunition in your launcher ignites "
          "for %d points of damage![reset]",
          weapon_catalogue_damage(weapon_index));

      if (!mech_is_destroyed(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE,
                       "'s %s is engulfed in a brilliant blue flame!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }

      mech_weapon_destroy(&(WeaponDestructionRequest){
          .mech = wounded,
          .first = {.section = hitloc, .critical = wFirstCrit},
          .part_type = critType,
          .criticals_to_destroy = wMaxCrits,
          .total_criticals = wMaxCrits});

      if (attacker) {
        mech_damage_apply(&(MechDamageRequest){
            .target = wounded,
            .attacker = attacker,
            .line_of_sight = 0,
            .attack_pilot = -1,
            .hit_location = hitloc,
            .rear = 0,
            .critical = 0,
            .armor_damage = 0,
            .internal_damage = weapon_catalogue_damage(weapon_index),
            .transfer = MECH_DAMAGE_NORMAL,
            .cause = -1,
            .base_to_hit = 7,
            .weapon_index = -1,
            .ammunition_mode = 0,
            .ignore_swarmers = 1});

        return 1;
      }
    }
  }

  return 0;
}

void mech_main_weapon_jam(Mech *mech) {
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int count;
  int loop;
  int ii;
  int tempcrit;
  int maxcrit = 0;
  int maxloc = 0;
  int critfound = 0;
  int critnum = 0;
  unsigned char maxtype = 0;
  BtechContext *context = mech_context(mech);

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (mech_section_is_destroyed(mech, loop))
      continue;
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        const int current_critical =
            *critical_slot(critical, MAX_WEAPS_SECTION, (size_t)ii);
        const unsigned char current_weapon =
            *weapon_slot(weaparray, MAX_WEAPS_SECTION, (size_t)ii);
        if (!mech_critical_is_broken(mech, loop, current_critical)) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_context_random_i31(context);
          if (tempcrit > maxcrit) {
            critfound = 1;
            maxcrit = tempcrit;
            maxloc = loop;
            maxtype = current_weapon;
            critnum = current_critical;
          }
        }
      }
    }
  }

  if (critfound) {
    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = mech,
        .slot = {.section = maxloc, .critical = critnum},
        .failure = FAIL_DESTROYED});
    mech_printf(mech, MECHALL, "[fg=red bold]Your %s is jammed![reset]",
                weapon_display_name(maxtype));
  }
}

void mech_random_weapon_select(Mech *objMech, int wLoc, int *critNum,
                               int wIgnoreJams) {
  int awCrits[MAX_WEAPS_SECTION];
  int wcWeaps = 0;
  int wIter;

  /*
   * Find our weapons
   */

  for (wIter = 0; wIter < MAX_WEAPS_SECTION; wIter++) {
    if (equipment_is_weapon(mech_critical_part_type(objMech, wLoc, wIter))) {
      if (!mech_critical_is_broken(objMech, wLoc, wIter)) {
        if (!wIgnoreJams || (wIgnoreJams && !mech_critical_temporary_failure(
                                                objMech, wLoc, wIter))) {
          *critical_slot(awCrits, MAX_WEAPS_SECTION, (size_t)wcWeaps) = wIter;

          wcWeaps++;
        }
      }
    }
  }

  if (wcWeaps <= 0) {
    *critNum = -1;
    return;
  }

  /*
   * Now randomly pick one
   */

  *critNum = *critical_slot(
      awCrits, MAX_WEAPS_SECTION,
      (size_t)btech_random_range_int(mech_context(objMech), 0, wcWeaps - 1));
}

/*
 * Make sure we're not set to go over our walking/cruise speed
 */
void mech_speed_limit_to_cruise(Mech *objMech) {
  float maximum_speed;

  maximum_speed =
      mech_cargo_maximum_speed(objMech, mech_maximum_speed(objMech));

  if (mech_movement_type(objMech) == MOVE_VTOL)
    maximum_speed =
        sqrtf(maximum_speed * maximum_speed -
              mech_vertical_speed(objMech) * mech_vertical_speed(objMech));

  const float walking_speed = 2.0F * maximum_speed / 3.0F;
  if (walking_speed < mech_desired_speed(objMech))
    mech_desired_speed_set(objMech, walking_speed - 0.1F);
}

void mech_vehicle_stabilizer_critical_apply(Mech *objMech, int wLoc) {
  /*
   * Double attacker movement for all weapons fired from
   * this location. If no weapons in this location, crit has no
   * effect. Only first stablizer hit matters, subsequent ones
   * should be ignored.
   */

  char strLocName[30];

  ArmorStringFromIndex(wLoc, strLocName, mech_class(objMech),
                       mech_movement_type(objMech));

  if (mech_section_configuration_has(objMech, wLoc, STABILIZERS_DESTROYED))
    mech_printf(objMech, MECHALL,
                "The destroyed weapon stabilizers in your %s take another hit!",
                strLocName);
  else {
    mech_printf(objMech, MECHALL,
                "The weapon stabilizers in your %s have been destroyed!",
                strLocName);
    mech_section_configuration_add(objMech, wLoc, STABILIZERS_DESTROYED);
  }
}

void mech_turret_lock_critical_apply(Mech *objMech) {
  /*
   * Turret locks in the current direction.
   */

  MechConditionSummary condition = mech_condition_summary(objMech);
  if (condition.turret_locked) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed)
    mech_turret_jammed_set(objMech, false);

  mech_turret_locked_set(objMech, true);
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot destroys your turret rotation mechanism![reset]");
}

void mech_turret_jam_critical_apply(Mech *objMech) {
  /*
   * Turret rotation temporarily jams. Vehicle crew must spend
   * attack phase unjamming (read for mux: no weapons fire/ramming/etc...
   * while unjamming turret. Second jam crit == turret locked.
   */

  MechConditionSummary condition = mech_condition_summary(objMech);
  if (condition.turret_locked) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed) {
    mech_turret_lock_critical_apply(objMech);
    return;
  }

  mech_turret_jammed_set(objMech, true);
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your turret gets jammed on its current facing![reset]");
}

void mech_weapon_jam_critical_apply(Mech *objMech, int wLoc) {
  /*
   * A weapon in this location is stuck. The vehicle crew must spend
   * the attack phase unjamming this weapon.
   *
   * Can this really apply to a non-ammo weapon? Maybe we should just do a
   * 'shorted/jammed' failure on the weapon?
   *
   * ALTERATION: Currently it's coded to 'auto-unjam' after 60 to 120 seconds.
   */

  int wWeapIdx = 0;
  int wCritType = 0;
  int wCritNum = 0;

  if (mech_section_is_destroyed(objMech, wLoc))
    return;

  mech_random_weapon_select(objMech, wLoc, &wCritNum, 1);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = mech_critical_part_type(objMech, wLoc, wCritNum);
  wWeapIdx = weapon_from_equipment_index(wCritType);

  if (wWeapIdx >= 0) {
    switch (weapon_catalogue_type(wWeapIdx)) {
    case TBEAM:
    case TMISSILE:
    case TARTILLERY:
      wCritType = FAIL_SHORTED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  weapon_display_name(wWeapIdx));
      break;
    case TAMMO:
      wCritType = FAIL_JAMMED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot temporarily jams your %s![reset]",
                  weapon_display_name(wWeapIdx));
      break;
    default:
      wCritType = FAIL_SHORTED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  weapon_display_name(wWeapIdx));
      break;
    }

    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = objMech,
        .slot = {.section = wLoc, .critical = wCritNum},
        .failure = wCritType});
    mech_set_recycle_part(
        objMech, wLoc, wCritNum,
        btech_random_range_int(mech_context(objMech), 60, 120));
  }
}

void mech_weapon_destroyed_critical_apply(
    const RandomWeaponDestructionRequest *request) {
  Mech *objAttacker = request->attacker;
  Mech *objMech = request->mech;
  const int wLoc = request->section;
  /*
   * A weapon in this location is destroyed.
   */
  int wWeapIdx = 0;
  int wCritNum = 0;
  int wCritType = 0;
  int firstCrit = 0;

  if (mech_section_is_destroyed(objMech, wLoc))
    return;

  mech_random_weapon_select(objMech, wLoc, &wCritNum, 0);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = mech_critical_part_type(objMech, wLoc, wCritNum);
  wWeapIdx = weapon_from_equipment_index(wCritType);

  if (mech_weapon_critical_handle(&(WeaponCriticalRequest){
          .attacker = objAttacker,
          .wounded = objMech,
          .slot = {.section = wLoc, .critical = wCritNum},
          .part_type = wCritType})) {
    return;
  }

  if (wWeapIdx >= 0) {
    firstCrit = mech_weapon_first_critical(&(WeaponCriticalSearch){
        .mech = objMech,
        .weapon = {.section = wLoc, .critical = -1},
        .start_critical = 0,
        .part_type = wCritType,
        .maximum_criticals = 1,
    });

    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = objMech,
        .first = {.section = wLoc, .critical = firstCrit},
        .part_type = wCritType,
        .criticals_to_destroy = 1,
        .total_criticals = 1});
    mech_printf(objMech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
                weapon_display_name(wWeapIdx));
  }
}

void mech_turret_blown_off_critical_apply(Mech *objMech, Mech *objAttacker,
                                          int LOS) {
  /*
   * The turret is blown off, destroying everything in there
   */

  if (mech_section_is_destroyed(objMech, TURRET))
    return;

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot pops your turret clear off its housing![reset]");
  mech_los_broadcast(objMech, "'s turret flies off!");
  mech_section_destroy(&(SectionDestructionRequest){.wounded = objMech,
                                                    .attacker = objAttacker,
                                                    .line_of_sight = LOS,
                                                    .section = TURRET});
}

void mech_ammunition_critical_apply(const AmmunitionCriticalRequest *request) {
  Mech *objMech = request->mech;
  Mech *objAttacker = request->attacker;
  const int wLoc = request->section;
  /*
   * Count total ammo carried on the tank. Apply damage directly to
   * the internal structure of the vehicle.
   *
   * If the vehicle has CASE, apply the damage to the rear ARMOR and also
   * cause a Driver Hit, Commander Hit and Crew Stunned crit.
   *
   * if the vehicle has no ammunition, treat this as a weapon destroyed crit.
   */

  int wTotalAmmoDamage = 0;
  int wTempDamage = 0;
  int wSecIter, wSlotIter;
  int wPartType = 0;
  int wWeapIdx;
  BtechContext *context = mech_context(objMech);

  for (wSecIter = 0; wSecIter <= 7; wSecIter++) {
    if (mech_section_is_destroyed(objMech, wSecIter))
      continue;

    for (wSlotIter = mech_section_critical_count(objMech, wSecIter) - 1;
         wSlotIter >= 0; wSlotIter--) {
      wPartType = mech_critical_part_type(objMech, wSecIter, wSlotIter);
      wWeapIdx = ammunition_to_weapon_index(wPartType);

      if (equipment_is_ammunition(wPartType) &&
          mech_critical_data(objMech, wSecIter, wSlotIter) &&
          !weapon_catalogue_is_gauss(wWeapIdx)) {
        wTempDamage = weapon_maximum_ammunition_damage(
                          context, ammunition_to_weapon_index(wPartType)) *
                      mech_critical_data(objMech, wSecIter, wSlotIter);
        wTotalAmmoDamage += wTempDamage;

        mech_critical_data_set(objMech, wSecIter, wSlotIter, 0);
      }
    }
  }

  if (wTotalAmmoDamage == 0) {
    mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
        .attacker = objAttacker, .mech = objMech, .section = wLoc});
    return;
  }

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]One of your ammo bins is struck causing a cascading "
      "explosion![reset]");
  mech_los_broadcast(objMech, "has an internal ammo explosion!");

  mech_damage_apply(&(MechDamageRequest){.target = objMech,
                                         .attacker = objAttacker,
                                         .line_of_sight = 0,
                                         .attack_pilot = -1,
                                         .hit_location = wLoc,
                                         .rear = 0,
                                         .critical = 0,
                                         .armor_damage = 0,
                                         .internal_damage = wTotalAmmoDamage,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = 0,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = 1});
}
