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

bool mech_weapon_critical_handle(const WeaponCriticalRequest *request) {
  Mech *attacker = request->attacker;
  Mech *wounded = request->wounded;
  const int HITLOC = request->slot.section;
  const int CRIT_HIT = request->slot.critical;
  const int CRIT_TYPE = request->part_type;
  int w_max_crits;
  int w_first_crit;
  int w_weap_destroyed = 0;
  int damage;
  char locname[30];
  char msgbuf[MBUF_SIZE] = {0};
  BtechContext *context = mech_context(wounded);

  armor_string_from_index(HITLOC, locname, mech_class(wounded),
                          mech_movement_type(wounded));

  /* Get the max number of crits for this weapon */
  w_max_crits =
      get_weapon_crits(wounded, weapon_from_equipment_index(CRIT_TYPE));

  /* Find the first crit */
  w_first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = wounded,
      .weapon = {.section = HITLOC, .critical = CRIT_HIT},
      .start_critical = 0,
      .part_type = CRIT_TYPE,
      .maximum_criticals = w_max_crits,
  });

  /* See if the weapon is already destroyed */
  if (w_first_crit != -1) {
    w_weap_destroyed =
        (mech_critical_is_nonfunctional(wounded, HITLOC, w_first_crit) ||
         (mech_critical_temporary_failure(wounded, HITLOC, w_first_crit) ==
          FAIL_DESTROYED));
  }

  /* Gauss rifle-ish weapons explode when critted */
  const int WEAPON_INDEX = weapon_from_equipment_index(CRIT_TYPE);
  if (weapon_catalogue_is_gauss(WEAPON_INDEX) && !w_weap_destroyed) {
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                weapon_display_name(WEAPON_INDEX));

    mech_printf(wounded, MECHALL, "It explodes for %d points damage.",
                weapon_catalogue_explosion_damage(WEAPON_INDEX));

    if (!mech_is_destroyed(wounded)) {
      (void)snprintf(msgbuf, MBUF_SIZE,
                     "'s %s is covered in a large electrical discharge!",
                     locname);
      mech_los_broadcast(wounded, msgbuf);
    }

    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = wounded,
        .first = {.section = HITLOC, .critical = w_first_crit},
        .part_type = CRIT_TYPE,
        .criticals_to_destroy = w_max_crits,
        .total_criticals = w_max_crits});

    if (attacker) {
      mech_damage_apply(&(MechDamageRequest){
          .target = wounded,
          .attacker = attacker,
          .line_of_sight = false,
          .attack_pilot = -1,
          .hit_location = HITLOC,
          .rear = false,
          .critical = false,
          .armor_damage = 0,
          .internal_damage = weapon_catalogue_explosion_damage(WEAPON_INDEX),
          .transfer = MECH_DAMAGE_NORMAL,
          .cause = -1,
          .base_to_hit = 7,
          .weapon_index = -1,
          .ammunition_mode = 0,
          .ignore_swarmers = true});
    }
    /* Rule Reference: BMR Revised, Page 16-17 (Ammo Explosion=2 Bruise) */
    /* Rule Reference: Total Warfare, Page 41 (Ammo Explosion=2 Bruise) */

    if (mech_class(wounded) != CLASS_BSUIT) {
      mech_notify(wounded, MECHPILOT,
                  "You take personal injury from the weapon's explosion!");

      /* Rule Reference: MaxTech Revised, Page 46 (Reduce by 1 because of pain
       * resistance) */

      if (has_bool_advantage(context, mech_pilot_dbref(wounded),
                             "pain_resistance"))
        headhitmwdamage(wounded, wounded, 1);
      else
        headhitmwdamage(wounded, wounded, 2);
    }
    return true;
  }
  if (weapon_catalogue_is_anti_missile(
          weapon_from_equipment_index(CRIT_TYPE))) { /* Have to shut down
                                    AMS when its critted */
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                weapon_display_name(WEAPON_INDEX));

    mech_ams_enabled_set(wounded, false);
    mech_technology_flags_remove(wounded,
                                 IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH);
  } else if (weapon_catalogue_is_hot_loaded(
                 weapon_from_equipment_index(CRIT_TYPE),
                 mech_critical_fire_mode(wounded, HITLOC, w_first_crit)) &&
             !w_weap_destroyed) { /* And crit hotloaded LRMs */
    CriticalSlotLookupResult ammunition =
        ammunition_find(&(AmmunitionLookupRequest){
            .mech = wounded,
            .weapon_index = weapon_from_equipment_index(CRIT_TYPE),
            .forbidden_modes = AMMO_MODES});
    if (ammunition.found) {
      damage = weapon_catalogue_damage(WEAPON_INDEX);

      if (weapon_catalogue_is_missile(weapon_from_equipment_index(CRIT_TYPE)) ||
          weapon_catalogue_is_artillery(
              weapon_from_equipment_index(CRIT_TYPE))) {
        int missile_count = btech_context_missile_hit_count(&(MissileHitLookup){
            .context = context,
            .weapon = weapon_from_equipment_index(CRIT_TYPE),
            .roll = 10,
        });
        if (missile_count > 0) {
          damage *= missile_count;
        }
      }

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  weapon_display_name(WEAPON_INDEX));

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
          .first = {.section = HITLOC, .critical = w_first_crit},
          .part_type = CRIT_TYPE,
          .criticals_to_destroy = w_max_crits,
          .total_criticals = w_max_crits});

      if (attacker) {
        mech_damage_apply(&(MechDamageRequest){.target = wounded,
                                               .attacker = attacker,
                                               .line_of_sight = false,
                                               .attack_pilot = -1,
                                               .hit_location = HITLOC,
                                               .rear = false,
                                               .critical = false,
                                               .armor_damage = 0,
                                               .internal_damage = damage,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = -1,
                                               .base_to_hit = 7,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = true});
      }

      return true;
    }
  } else if ((mech_critical_ammo_mode(wounded, HITLOC, w_first_crit) &
              AC_INCENDIARY_MODE) &&
             !w_weap_destroyed &&
             mech_weapon_is_recycling_at(
                 wounded, HITLOC,
                 w_first_crit)) { /* Incendiary ACs blow up too */

    CriticalSlotLookupResult ammunition =
        ammunition_find(&(AmmunitionLookupRequest){
            .mech = wounded,
            .weapon_index = weapon_from_equipment_index(CRIT_TYPE),
            .required_modes = AC_INCENDIARY_MODE});
    if (ammunition.found) {

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  weapon_display_name(WEAPON_INDEX));

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]The incendiary ammunition in your launcher ignites "
          "for %d points of damage![reset]",
          weapon_catalogue_damage(WEAPON_INDEX));

      if (!mech_is_destroyed(wounded)) {
        (void)snprintf(msgbuf, MBUF_SIZE,
                       "'s %s is engulfed in a brilliant blue flame!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }

      mech_weapon_destroy(&(WeaponDestructionRequest){
          .mech = wounded,
          .first = {.section = HITLOC, .critical = w_first_crit},
          .part_type = CRIT_TYPE,
          .criticals_to_destroy = w_max_crits,
          .total_criticals = w_max_crits});

      if (attacker) {
        mech_damage_apply(&(MechDamageRequest){
            .target = wounded,
            .attacker = attacker,
            .line_of_sight = false,
            .attack_pilot = -1,
            .hit_location = HITLOC,
            .rear = false,
            .critical = false,
            .armor_damage = 0,
            .internal_damage = weapon_catalogue_damage(WEAPON_INDEX),
            .transfer = MECH_DAMAGE_NORMAL,
            .cause = -1,
            .base_to_hit = 7,
            .weapon_index = -1,
            .ammunition_mode = 0,
            .ignore_swarmers = true});

        return true;
      }
    }
  }

  return false;
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
    count = find_weapons_advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        const int CURRENT_CRITICAL =
            *critical_slot(critical, MAX_WEAPS_SECTION, (size_t)ii);
        const unsigned char CURRENT_WEAPON =
            *weapon_slot(weaparray, MAX_WEAPS_SECTION, (size_t)ii);
        if (!mech_critical_is_broken(mech, loop, CURRENT_CRITICAL)) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_context_random_i31(context);
          if (tempcrit > maxcrit) {
            critfound = 1;
            maxcrit = tempcrit;
            maxloc = loop;
            maxtype = CURRENT_WEAPON;
            critnum = CURRENT_CRITICAL;
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

void mech_random_weapon_select(Mech *obj_mech, int w_loc, int *crit_num,
                               int w_ignore_jams) {
  int aw_crits[MAX_WEAPS_SECTION];
  int wc_weaps = 0;
  int w_iter;

  /*
   * Find our weapons
   */

  for (w_iter = 0; w_iter < MAX_WEAPS_SECTION; w_iter++) {
    if (equipment_is_weapon(mech_critical_part_type(obj_mech, w_loc, w_iter))) {
      if (!mech_critical_is_broken(obj_mech, w_loc, w_iter)) {
        if (!w_ignore_jams ||
            (w_ignore_jams &&
             !mech_critical_temporary_failure(obj_mech, w_loc, w_iter))) {
          *critical_slot(aw_crits, MAX_WEAPS_SECTION, (size_t)wc_weaps) =
              w_iter;

          wc_weaps++;
        }
      }
    }
  }

  if (wc_weaps <= 0) {
    *crit_num = -1;
    return;
  }

  /*
   * Now randomly pick one
   */

  *crit_num = *critical_slot(
      aw_crits, MAX_WEAPS_SECTION,
      (size_t)btech_random_range_int(mech_context(obj_mech), 0, wc_weaps - 1));
}

/*
 * Make sure we're not set to go over our walking/cruise speed
 */
void mech_speed_limit_to_cruise(Mech *obj_mech) {
  float maximum_speed;

  maximum_speed =
      mech_cargo_maximum_speed(obj_mech, mech_maximum_speed(obj_mech));

  if (mech_movement_type(obj_mech) == MOVE_VTOL)
    maximum_speed =
        sqrtf((maximum_speed * maximum_speed) -
              (mech_vertical_speed(obj_mech) * mech_vertical_speed(obj_mech)));

  const float WALKING_SPEED = 2.0F * maximum_speed / 3.0F;
  if (WALKING_SPEED < mech_desired_speed(obj_mech))
    mech_desired_speed_set(obj_mech, WALKING_SPEED - 0.1F);
}

void mech_vehicle_stabilizer_critical_apply(Mech *obj_mech, int w_loc) {
  /*
   * Double attacker movement for all weapons fired from
   * this location. If no weapons in this location, crit has no
   * effect. Only first stablizer hit matters, subsequent ones
   * should be ignored.
   */

  char str_loc_name[30];

  armor_string_from_index(w_loc, str_loc_name, mech_class(obj_mech),
                          mech_movement_type(obj_mech));

  if (mech_section_configuration_has(obj_mech, w_loc, STABILIZERS_DESTROYED)) {
    mech_printf(obj_mech, MECHALL,
                "The destroyed weapon stabilizers in your %s take another hit!",
                str_loc_name);
  } else {
    mech_printf(obj_mech, MECHALL,
                "The weapon stabilizers in your %s have been destroyed!",
                str_loc_name);
    mech_section_configuration_add(obj_mech, w_loc, STABILIZERS_DESTROYED);
  }
}

void mech_turret_lock_critical_apply(Mech *obj_mech) {
  /*
   * Turret locks in the current direction.
   */

  MechConditionSummary condition = mech_condition_summary(obj_mech);
  if (condition.turret_locked) {
    mech_notify(
        obj_mech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed)
    mech_turret_jammed_set(obj_mech, false);

  mech_turret_locked_set(obj_mech, true);
  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]The shot destroys your turret rotation mechanism![reset]");
}

void mech_turret_jam_critical_apply(Mech *obj_mech) {
  /*
   * Turret rotation temporarily jams. Vehicle crew must spend
   * attack phase unjamming (read for mux: no weapons fire/ramming/etc...
   * while unjamming turret. Second jam crit == turret locked.
   */

  MechConditionSummary condition = mech_condition_summary(obj_mech);
  if (condition.turret_locked) {
    mech_notify(
        obj_mech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed) {
    mech_turret_lock_critical_apply(obj_mech);
    return;
  }

  mech_turret_jammed_set(obj_mech, true);
  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]Your turret gets jammed on its current facing![reset]");
}

void mech_weapon_jam_critical_apply(Mech *obj_mech, int w_loc) {
  /*
   * A weapon in this location is stuck. The vehicle crew must spend
   * the attack phase unjamming this weapon.
   *
   * Can this really apply to a non-ammo weapon? Maybe we should just do a
   * 'shorted/jammed' failure on the weapon?
   *
   * ALTERATION: Currently it's coded to 'auto-unjam' after 60 to 120 seconds.
   */

  int w_weap_idx = 0;
  int w_crit_type = 0;
  int w_crit_num = 0;

  if (mech_section_is_destroyed(obj_mech, w_loc))
    return;

  mech_random_weapon_select(obj_mech, w_loc, &w_crit_num, 1);

  if (w_crit_num < 0) {
    mech_notify(
        obj_mech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  w_crit_type = mech_critical_part_type(obj_mech, w_loc, w_crit_num);
  w_weap_idx = weapon_from_equipment_index(w_crit_type);

  if (w_weap_idx >= 0) {
    switch (weapon_catalogue_type(w_weap_idx)) {
    case TBEAM:
    case TMISSILE:
    case TARTILLERY:
      w_crit_type = FAIL_SHORTED;
      mech_printf(obj_mech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  weapon_display_name(w_weap_idx));
      break;
    case TAMMO:
      w_crit_type = FAIL_JAMMED;
      mech_printf(obj_mech, MECHALL,
                  "[fg=red bold]The shot temporarily jams your %s![reset]",
                  weapon_display_name(w_weap_idx));
      break;
    default:
      w_crit_type = FAIL_SHORTED;
      mech_printf(obj_mech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  weapon_display_name(w_weap_idx));
      break;
    }

    mech_critical_temporary_failure_set(&(CriticalSlotFailureSet){
        .mech = obj_mech,
        .slot = {.section = w_loc, .critical = w_crit_num},
        .failure = w_crit_type});
    mech_set_recycle_part(
        obj_mech, w_loc, w_crit_num,
        btech_random_range_int(mech_context(obj_mech), 60, 120));
  }
}

void mech_weapon_destroyed_critical_apply(
    const RandomWeaponDestructionRequest *request) {
  Mech *obj_attacker = request->attacker;
  Mech *obj_mech = request->mech;
  const int W_LOC = request->section;
  /*
   * A weapon in this location is destroyed.
   */
  int w_weap_idx = 0;
  int w_crit_num = 0;
  int w_crit_type = 0;
  int first_crit = 0;

  if (mech_section_is_destroyed(obj_mech, W_LOC))
    return;

  mech_random_weapon_select(obj_mech, W_LOC, &w_crit_num, 0);

  if (w_crit_num < 0) {
    mech_notify(
        obj_mech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  w_crit_type = mech_critical_part_type(obj_mech, W_LOC, w_crit_num);
  w_weap_idx = weapon_from_equipment_index(w_crit_type);

  if (mech_weapon_critical_handle(&(WeaponCriticalRequest){
          .attacker = obj_attacker,
          .wounded = obj_mech,
          .slot = {.section = W_LOC, .critical = w_crit_num},
          .part_type = w_crit_type})) {
    return;
  }

  if (w_weap_idx >= 0) {
    first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
        .mech = obj_mech,
        .weapon = {.section = W_LOC, .critical = -1},
        .start_critical = 0,
        .part_type = w_crit_type,
        .maximum_criticals = 1,
    });

    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = obj_mech,
        .first = {.section = W_LOC, .critical = first_crit},
        .part_type = w_crit_type,
        .criticals_to_destroy = 1,
        .total_criticals = 1});
    mech_printf(obj_mech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
                weapon_display_name(w_weap_idx));
  }
}

void mech_turret_blown_off_critical_apply(Mech *obj_mech, Mech *obj_attacker,
                                          int los) {
  /*
   * The turret is blown off, destroying everything in there
   */

  if (mech_section_is_destroyed(obj_mech, TURRET))
    return;

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]The shot pops your turret clear off its housing![reset]");
  mech_los_broadcast(obj_mech, "'s turret flies off!");
  mech_section_destroy(&(SectionDestructionRequest){.wounded = obj_mech,
                                                    .attacker = obj_attacker,
                                                    .line_of_sight = los,
                                                    .section = TURRET});
}

void mech_ammunition_critical_apply(const AmmunitionCriticalRequest *request) {
  Mech *obj_mech = request->mech;
  Mech *obj_attacker = request->attacker;
  const int W_LOC = request->section;
  /*
   * Count total ammo carried on the tank. Apply damage directly to
   * the internal structure of the vehicle.
   *
   * If the vehicle has CASE, apply the damage to the rear ARMOR and also
   * cause a Driver Hit, Commander Hit and Crew Stunned crit.
   *
   * if the vehicle has no ammunition, treat this as a weapon destroyed crit.
   */

  int w_total_ammo_damage = 0;
  int w_temp_damage = 0;
  int w_sec_iter;
  int w_slot_iter;
  int w_part_type = 0;
  int w_weap_idx;
  BtechContext *context = mech_context(obj_mech);

  for (w_sec_iter = 0; w_sec_iter <= 7; w_sec_iter++) {
    if (mech_section_is_destroyed(obj_mech, w_sec_iter))
      continue;

    for (w_slot_iter = mech_section_critical_count(obj_mech, w_sec_iter) - 1;
         w_slot_iter >= 0; w_slot_iter--) {
      w_part_type = mech_critical_part_type(obj_mech, w_sec_iter, w_slot_iter);
      w_weap_idx = ammunition_to_weapon_index(w_part_type);

      if (equipment_is_ammunition(w_part_type) &&
          mech_critical_data(obj_mech, w_sec_iter, w_slot_iter) &&
          !weapon_catalogue_is_gauss(w_weap_idx)) {
        w_temp_damage = weapon_maximum_ammunition_damage(
                            context, ammunition_to_weapon_index(w_part_type)) *
                        mech_critical_data(obj_mech, w_sec_iter, w_slot_iter);
        w_total_ammo_damage += w_temp_damage;

        mech_critical_data_set(obj_mech, w_sec_iter, w_slot_iter, 0);
      }
    }
  }

  if (w_total_ammo_damage == 0) {
    mech_weapon_destroyed_critical_apply(&(RandomWeaponDestructionRequest){
        .attacker = obj_attacker, .mech = obj_mech, .section = W_LOC});
    return;
  }

  mech_notify(
      obj_mech, MECHALL,
      "[fg=red bold]One of your ammo bins is struck causing a cascading "
      "explosion![reset]");
  mech_los_broadcast(obj_mech, "has an internal ammo explosion!");

  mech_damage_apply(&(MechDamageRequest){.target = obj_mech,
                                         .attacker = obj_attacker,
                                         .line_of_sight = false,
                                         .attack_pilot = -1,
                                         .hit_location = W_LOC,
                                         .rear = false,
                                         .critical = false,
                                         .armor_damage = 0,
                                         .internal_damage = w_total_ammo_damage,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = 0,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = true});
}
