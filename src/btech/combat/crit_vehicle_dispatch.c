/* Dispatches critical hits for vehicles. */

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
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static int critical_at(int *criticals, size_t index) {
  return *(const int *)checked_storage_at_const(criticals, MAX_WEAPS_SECTION,
                                                sizeof(*criticals), index);
}

static unsigned char weapon_at(unsigned char *weapons, size_t index) {
  return *(const unsigned char *)checked_storage_at_const(
      weapons, MAX_WEAPS_SECTION, sizeof(*weapons), index);
}

void mech_main_weapon_destroy(Mech *mech) {
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
  unsigned char maxtype = 0;
  int firstCrit = 0;
  BtechContext *context = mech_context(mech);

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (mech_section_is_destroyed(mech, loop))
      continue;
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        const int current_critical = critical_at(critical, (size_t)ii);
        const unsigned char current_weapon = weapon_at(weaparray, (size_t)ii);
        if (!mech_critical_is_broken(mech, loop, current_critical)) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_context_random_i31(context);
          if (tempcrit > maxcrit) {
            critfound = 1;
            maxcrit = tempcrit;
            maxloc = loop;
            maxtype = current_weapon;
          }
        }
      }
    }
  }
  if (critfound) {
    firstCrit = FindFirstWeaponCrit(mech, maxloc, -1, 0,
                                    weapon_equipment_index(maxtype), 1);
    mech_weapon_destroy(mech, maxloc, weapon_equipment_index(maxtype), 1,
                        firstCrit, GetWeaponCrits(mech, maxtype));
    mech_printf(mech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
                checked_string_suffix(weapon_catalogue_name(maxtype), 3));
  }
}

void mech_fasa_vehicle_critical_handle(Mech *wounded, Mech *attacker, int LOS,
                                       int hitloc, int num) {
  BtechContext *context = mech_context(wounded);

  if (mech_movement_type(wounded) == MOVE_NONE)
    return;

  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
  switch (btech_random_range(context, 0, 5)) {
  case 0:
    /* Crew stunned for one turn...treat like a head hit */
    headhitmwdamage(wounded, attacker, 1);
    break;
  case 1:
    /* Weapon jams, set them recylcling maybe */
    /* hmm. nothing for now, tanks are so weak */
    mech_main_weapon_jam(wounded);
    break;
  case 2:
    /* Engine Hit */
    mech_notify(wounded, MECHALL,
                "Your engine takes a direct hit!  You can't move anymore.");
    mech_max_speed_set(wounded, 0.0);
    break;
  case 3:
    /* Crew Killed */
    mech_notify(wounded, MECHALL,
                "Your armor is pierced and you are killed instantly!");
    mech_destroy(wounded, attacker, 0, KILL_TYPE_PILOT);
    mech_contents_kill_if_in_character(wounded);
    break;
  case 4:
    /* Fuel Tank Explodes */
    mech_notify(wounded, MECHALL, "Your fuel tank explodes in a ball of fire!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s fule tank explodes in a ball of fire!");
    mech_destroy(wounded, attacker, 1, KILL_TYPE_FUELTANK);
    mech_explosion_apply(wounded, attacker);
    break;
  case 5:
    /* Ammo/Power Plant Explodes */
    mech_notify(wounded, MECHALL, "Your power plant explodes!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s power plant suddenly explodes!");
    mech_destroy(wounded, attacker, 1, KILL_TYPE_POWERPLANT);
    if (!mech_section_configuration_has(wounded, BSIDE, CASE_TECH))
      mech_explosion_apply(wounded, attacker);
    else
      mech_section_destroy(wounded, attacker, LOS, BSIDE);
    break;
  }
}

void mech_vehicle_critical_handle(Mech *wounded, Mech *attacker, int LOS,
                                  int hitloc, int num) {
  BtechContext *context = mech_context(wounded);
  MechConditionSummary condition = mech_condition_summary(wounded);

  if (mech_movement_type(wounded) == MOVE_NONE)
    return;
  if (hitloc == TURRET) {
    if (btech_random_range(context, 1, 3) == 2) {
      if (!condition.turret_locked) {
        mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
        mech_turret_locked_set(wounded, true);
        mech_notify(wounded, MECHALL,
                    "Your turret takes a direct hit and locks up!");
      }
      return;
    }
  } else
    switch (btech_random_range(context, 1, 10)) {
    case 1:
    case 2:
    case 3:
    case 4:
      if (!condition.fallen) {
        mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
        switch (mech_movement_type(wounded)) {
        case MOVE_TRACK:
          mech_notify(wounded, MECHALL, "One of your tracks is damaged!");
          break;
        case MOVE_WHEEL:
          mech_notify(wounded, MECHALL, "One of your wheels is damaged!");
          break;
        case MOVE_HOVER:
          mech_notify(wounded, MECHALL, "Your air skirt is damaged!");
          break;
        case MOVE_HULL:
        case MOVE_SUB:
        case MOVE_FOIL:
          mech_notify(wounded, MECHALL, "Your craft suddenly slows!");
          break;
        case MOVE_BIPED:
        case MOVE_VTOL:
        case MOVE_FLY:
        case MOVE_QUAD:
        case MOVE_NONE:
        default:
          break;
        }
        mech_max_speed_lower(wounded, MP1);
      }
      return;
      break;
    case 5:
      if (!condition.fallen) {
        mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
        switch (mech_movement_type(wounded)) {
        case MOVE_TRACK:
          mech_notify(
              wounded, MECHALL,
              "One of your tracks is destroyed, immobilizing your vehicle!");
          break;
        case MOVE_WHEEL:
          mech_notify(
              wounded, MECHALL,
              "One of your wheels is destroyed, immobilizing your vehicle!");
          break;
        case MOVE_HOVER:
          mech_notify(wounded, MECHALL,
                      "Your lift fan is destroyed, immobilizing your vehicle!");
          break;
        case MOVE_HULL:
        case MOVE_SUB:
        case MOVE_FOIL:
          mech_notify(wounded, MECHALL,
                      "Your engines cut out and you drift to a halt!");
          break;
        case MOVE_BIPED:
        case MOVE_VTOL:
        case MOVE_FLY:
        case MOVE_QUAD:
        case MOVE_NONE:
        default:
          break;
        }
        mech_max_speed_set(wounded, 0.0);

        mech_make_fall(wounded);
      }
      return;
      break;
    }
  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");
  switch (btech_random_range(context, 0, 5)) {
  case 0:
    /* Crew stunned for one turn...treat like a head hit */
    headhitmwdamage(wounded, attacker, 1);
    break;
  case 1:
    /* Weapon jams, set them recylcling maybe */
    /* hmm. nothing for now, tanks are so weak */
    mech_main_weapon_jam(wounded);
    break;
  case 2:
    /* Engine Hit */
    mech_notify(wounded, MECHALL,
                "Your engine takes a direct hit!  You can't move anymore.");
    mech_max_speed_set(wounded, 0.0);
    break;
  case 3:
    /* Crew Killed */
    mech_notify(wounded, MECHALL,
                "Your armor is pierced and you are killed instantly!");
    mech_destroy(wounded, attacker, 0, KILL_TYPE_PILOT);
    mech_contents_kill_if_in_character(wounded);
    break;
  case 4:
    /* Fuel Tank Explodes */
    mech_notify(wounded, MECHALL, "Your fuel tank explodes in a ball of fire!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s fuel tank explodes in a ball of fire!");
    mech_destroy(wounded, attacker, 1, KILL_TYPE_FUELTANK);
    mech_explosion_apply(wounded, attacker);
    break;
  case 5:
    /* Ammo/Power Plant Explodes */
    mech_notify(wounded, MECHALL, "Your power plant explodes!");
    if (wounded != attacker)
      mech_los_broadcast(wounded, "'s power plant suddenly explodes!");
    mech_destroy(wounded, attacker, 1, KILL_TYPE_POWERPLANT);
    mech_explosion_apply(wounded, attacker);
    break;
  }
}
