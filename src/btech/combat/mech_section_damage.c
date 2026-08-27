#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/* Implements BattleTech combat mechanics for unit section damage. */

#include <stdint.h>
#include <stdio.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btech_text_result.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "environment_damage_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

// NOLINTNEXTLINE(misc-no-recursion): fixed critical-slot count bounds cascade.
void mech_weapon_destroy(const WeaponDestructionRequest *request) {
  Mech *wounded = request->mech;
  int hitloc = request->first.section;
  int start_crit = request->first.critical;
  int type = request->part_type;
  int numcrits = request->criticals_to_destroy;
  int totalcrits = request->total_criticals;
  int i;
  int sum = totalcrits;
  int destroyed = numcrits;
  int disable = 0; // Hax for later destroying all crits or disabling

  for (i = start_crit; i < NUM_CRITICALS; i++) {
    if (mech_critical_part_type(wounded, hitloc, i) == type) {
      if (mech_critical_is_damaged(wounded, hitloc, i)) {
        if (disable)
          mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
        else
          mech_critical_destroy(wounded, hitloc, i);
      } else if (destroyed) {
        if (disable)
          mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
        else
          mech_critical_destroy(wounded, hitloc, i);
        destroyed--;
      } else {
        mech_critical_fire_mode_add(wounded, hitloc, i, BROKEN_MODE);
      }
      sum--;
      //                      if(sum == totalcrits)
      if (!sum)
        return;
    }
  }
  // if we've gotten here, then sum != total crits, but we've run outta crits in
  // this location, so it must be a split crit.
  if (mech_class(wounded) != CLASS_MECH)
    return; // sanity check
  SplitCriticalLookup split =
      split_critical_find(wounded, (CriticalSlotReference){hitloc, start_crit});
  if (split.found) {
    mech_weapon_destroy(
        &(WeaponDestructionRequest){.mech = wounded,
                                    .first = split.slot,
                                    .part_type = split.part_type,
                                    .criticals_to_destroy = destroyed,
                                    .total_criticals = sum});
  }
}

int mech_weapon_count_in_section(Mech *mech, int loc) {
  int i;
  int j;
  int sec;
  int count = 0;

  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = 1});
  j = lookup.value;
  sec = lookup.slot.section;
  for (i = 2; j != -1; i++) {
    if (sec == loc)
      count++;
    lookup = weapon_number_find(
        &(WeaponNumberLookupRequest){.mech = mech, .number = i});
    j = lookup.value;
    sec = lookup.slot.section;
  }
  return count;
}

int mech_weapon_index_in_section(const WeaponSectionLookup *section_lookup) {
  Mech *mech = section_lookup->mech;
  const int LOC = section_lookup->section;
  const int NUM = section_lookup->ordinal;
  int i;
  int j;
  int sec;
  int count = 0;

  WeaponNumberLookupResult lookup = weapon_number_find(
      &(WeaponNumberLookupRequest){.mech = mech, .number = 1});
  j = lookup.value;
  sec = lookup.slot.section;
  for (i = 2; j != -1; i++) {
    if (sec == LOC) {
      count++;
      if (count == NUM)
        return j;
    }
    lookup = weapon_number_find(
        &(WeaponNumberLookupRequest){.mech = mech, .number = i});
    j = lookup.value;
    sec = lookup.slot.section;
  }
  return -1;
}

void mech_weapon_destroy_random(Mech *mech, int hitloc) {
  /* Look for hit locations.. */
  int i = mech_weapon_count_in_section(mech, hitloc);
  int a;
  int b;
  int first_crit;

  if (!i)
    return;
  a = btech_random_range_int(mech_context(mech), 1, i);
  b = mech_weapon_index_in_section(
      &(WeaponSectionLookup){.mech = mech, .section = hitloc, .ordinal = a});
  if (b < 0)
    return;

  first_crit = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = mech,
      .weapon = {.section = hitloc, .critical = -1},
      .start_critical = 0,
      .part_type = weapon_equipment_index(b),
      .maximum_criticals = get_weapon_crits(mech, b),
  });

  mech_weapon_destroy(&(WeaponDestructionRequest){
      .mech = mech,
      .first = {.section = hitloc, .critical = first_crit},
      .part_type = weapon_equipment_index(b),
      .criticals_to_destroy = 1,
      .total_criticals = get_weapon_crits(mech, b)});
  mech_printf(mech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
              checked_string_suffix(weapon_catalogue_name(b), 3));
}

void mech_heat_sink_destroy(Mech *mech, int hitloc) {
  /* This can be done easily, or this can be done painfully. */
  /* Let's try the painful way, it's more fun that way. */
  int num;
  int i = special_equipment_index(HEAT_SINK);

  if (find_obj(mech, hitloc, i)) {
    num = mech_heat_sink_critical_size(mech);
    mech_weapon_destroy(
        &(WeaponDestructionRequest){.mech = mech,
                                    .first = {.section = hitloc, .critical = 0},
                                    .part_type = i,
                                    .criticals_to_destroy = 1,
                                    .total_criticals = num});
    mech_heat_sink_count_remove(mech, max(num, 2));
    mech_notify(mech, MECHALL,
                "The computer shows a heatsink died due to the impact.");
  }
}

// NOLINTNEXTLINE(misc-no-recursion): fixed section count bounds cascade.
void mech_section_destroy(const SectionDestructionRequest *request) {
  Mech *wounded = request->wounded;
  Mech *attacker = request->attacker;
  const int LOS = request->line_of_sight;
  const int HITLOC = request->section;
  char locname[30] = {0};
  char msgbuf[MBUF_SIZE] = {0};
  int i;
  int t_kill_mech;
  int t_is_leg =
      ((HITLOC == RLEG || HITLOC == LLEG) ||
       ((HITLOC == RARM || HITLOC == LARM) && mech_is_quad(wounded)));
  DbRef wounded_pilot = mech_pilot_dbref(wounded);
  Mech *ttarget;

  /* Prevent the rare occurance of a section getting destroyed twice */
  if (mech_section_is_destroyed(wounded, HITLOC)) {
    (void)fprintf(stderr, "Double-desting section %d on mech #%ld\n", HITLOC,
                  mech_dbref(wounded));
    if (mech_is_dropship(wounded))
      return;
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(wounded, i) &&
          mech_section_internal(wounded, i))
        return;
    if (btech_context_event_data_count(mech_context(wounded), EVENT_NUKEMECH,
                                       (intptr_t)wounded)) {
      (void)fprintf(stderr, "And nuke event already existed.\n");
      return;
    }
    discard_mw(wounded);
    return;
  }
  /* Ouch. They got toasted */
  mech_section_armor_set(wounded, HITLOC, 0);
  mech_section_internal_set(wounded, HITLOC, 0);
  mech_section_rear_armor_set(wounded, HITLOC, 0);
  mech_section_specials_clear(wounded, HITLOC);

  /* uncycle the section <in the case of an arm/leg that was kicking getting
   * blown */
  mech_set_recycle_limb(wounded, HITLOC, 0);

  /* drop off what we were carrying, since we really can't pick it up with one
   * arm */
  if ((HITLOC == RARM || HITLOC == LARM)) {
    if (mech_carried_dbref(wounded) > 0) {
      ttarget = btech_context_get_mech(mech_context(wounded),
                                       mech_carried_dbref(wounded));
      if (ttarget) {
        mech_notify(ttarget, MECHALL, "Your tow lines go suddenly slack!");
        mech_dropoff(GOD, wounded, "");
      }
    }
  }

  /* Tell the attacker about it... */
  if (attacker) {
    armor_string_from_index(HITLOC, locname, mech_class(wounded),
                            mech_movement_type(wounded));
    if (LOS >= 0)
      mech_printf(wounded, MECHALL, "Your %s has been destroyed!", locname);
    (void)snprintf(msgbuf, sizeof(msgbuf), "'s %s has been destroyed!",
                   locname);
    mech_los_broadcast(wounded, msgbuf);
  }

  /* Destroy everything in the loc */
  mech_parts_destroy(attacker, wounded, HITLOC, false, false);
  mech_ecm_check(wounded);
  /* Stop lateral if we're a quad */
  if (mech_class(wounded) == CLASS_MECH && mech_is_quad(wounded))
    if (mech_lateral_movement(wounded) && t_is_leg)
      mech_lateral_movement_set(wounded, 0);
  /* Check to see if we should destroy the unit */
  if (mech_class(wounded) == CLASS_BSUIT) {
    if (bsuit_member_count(wounded) > 0)
      goto skip_nuke;
    else if (!mech_is_destroyed(wounded))
      mech_destroy(wounded, attacker, true, KILL_TYPE_NORMAL);
  } else {
    for (i = 0; i < NUM_SECTIONS; i++)
      if (mech_section_original_internal(wounded, i) &&
          mech_section_internal(wounded, i))
        goto skip_nuke;
  }

  /* Ensure the template's timely demise */
  if (is_in_character(btech_context_database(mech_context(wounded)),
                      mech_dbref(wounded))) {
    mech_communications_clear(wounded);

    /* There's a 25% chance of bsuit pilots living through it */
    if ((mech_class(wounded) == CLASS_BSUIT) &&
        (btech_random_range(mech_context(wounded), 1, 100) <= 25) &&
        wounded_pilot)
      autoeject(wounded_pilot, wounded, 1);
    else
      mech_contents_kill_if_in_character(wounded);
    /* Schedule removal of the template */
    if (!mech_is_dropship(wounded))
      discard_mw(wounded);
  }

  /* We've done everything we should do... */
  return;
skip_nuke:

  /* Add 4 MW damage if it's a MW loosing a location */
  if (mech_class(wounded) == CLASS_MW) {
    mwlethaldam(wounded, attacker, 4);
  }

  /* If it's a MW or a mech, let's see if there's additional stuff we need to do
   */
  if (mech_class(wounded) == CLASS_MW || mech_class(wounded) == CLASS_MECH) {
    if (HITLOC == LTORSO) {
      mech_section_destroy(&(SectionDestructionRequest){
          .wounded = wounded,
          .attacker = attacker,
          .line_of_sight = LOS,
          .section = LARM,
      });
    } else if (HITLOC == RTORSO) {
      mech_section_destroy(&(SectionDestructionRequest){
          .wounded = wounded,
          .attacker = attacker,
          .line_of_sight = LOS,
          .section = RARM,
      });
    } else if (HITLOC == CTORSO || HITLOC == HEAD) {
      if (!mech_is_destroyed(wounded)) {
        if (HITLOC == HEAD) {
          if (attacker && mech_aim_section(attacker) == HEAD) {
            mech_destroy(wounded, attacker, true, KILL_TYPE_HEAD_TARGET);
          } else {
            mech_destroy(wounded, attacker, true, KILL_TYPE_BEHEADED);
          }
        } else {
          mech_destroy(wounded, attacker, true, KILL_TYPE_NORMAL);
        }
      }
      /* If it's the head or a MW's CT, kill the contents if IC */
      if (HITLOC == HEAD ||
          ((mech_class(wounded) == CLASS_MW) && (HITLOC == CTORSO))) {
        if (is_in_character(btech_context_database(mech_context(wounded)),
                            mech_dbref(wounded))) {
          mech_communications_clear(wounded);
          mech_contents_kill_if_in_character(wounded);
        }
      }

      if (mech_class(wounded) == CLASS_MW)
        discard_mw(wounded);
    }

    return;
  }

  /* If we're an aero... */
  if (mech_is_aerospace_unit(wounded)) {
    /* FIXME: Could this be the invincible aero bug? */
    /* Aero handling is trivial ; No destruction whatsoever, for now. */
    /* With one exception.. */
    if (HITLOC == COCKPIT && mech_class(wounded) == CLASS_AERO) {
      if (!mech_is_destroyed(wounded))
        mech_destroy(wounded, attacker, false, KILL_TYPE_COCKPIT);
      mech_communications_clear(wounded);
      mech_contents_kill_if_in_character(wounded);
    }
    return;
  }

  /* Last check to see if we destroy the unit... vehicle stuff */
  if (mech_hit_location_transfer(wounded, 0) < 0)
    t_kill_mech = 1;
  else
    t_kill_mech = 0;
  switch (mech_class(wounded)) {
  case CLASS_BSUIT:
    t_kill_mech = 0;
    break;
  case CLASS_VEH_GROUND:
    if (HITLOC == TURRET) {
      t_kill_mech = 0;
      mech_turret_auto_turn_set(wounded, false);
    } else {
      t_kill_mech = 1;
    }
    break;
  case CLASS_VTOL:
    if (HITLOC == ROTOR) {
      t_kill_mech = 0;
      mech_vtol_crash_start(wounded);
    } else {
      t_kill_mech = 1;
    }
    break;
  case CLASS_MECH:
  case CLASS_VEH_NAVAL:
  case CLASS_SPHEROID_DS:
  case CLASS_AERO:
  case CLASS_MW:
  case CLASS_DS:
    break;
  default:
    break;
  }

  if (t_kill_mech) {
    if (!mech_is_destroyed(wounded))
      mech_destroy(wounded, attacker, true, KILL_TYPE_NORMAL);
  }
}

BtechTextResult
mech_armor_status_set_value(const ArmorStatusSetRequest *request) {
  Mech *mech = request->mech;
  const char *sectstr = request->section;
  const char *typestr = request->armor_type;
  const char *valuestr = request->value;
  int index;
  int type;
  int value;

  if (!sectstr || !*sectstr)
    return (BtechTextResult){.text = "#-1 INVALID SECTION", .success = false};
  index = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    sectstr);
  if (index == -1 || !mech_section_original_internal(mech, index))
    return (BtechTextResult){.text = "#-1 INVALID SECTION", .success = false};
  if (!parse_int_checked(valuestr, &value) || value < 0 || value > 255)
    return (BtechTextResult){.text = "#-1 INVALID ARMORVALUE",
                             .success = false};
  if (!parse_int_checked(typestr, &type))
    return (BtechTextResult){.text = "#-1 INVALID TYPE", .success = false};
  switch (type) {
  case 0:
    mech_section_armor_set(mech, index, value);
    break;
  case 1:
    mech_section_internal_set(mech, index, value);
    break;
  case 2:
    mech_section_rear_armor_set(mech, index, value);
    break;
  default:
    return (BtechTextResult){.text = "#-1 INVALID ARMORTYPE", .success = false};
  }
  return (BtechTextResult){.text = "1", .success = true};
}

bool mech_damage_apply_clusters(const DamageClusterRequest *request) {
  Mech *mech = request->mech;
  int totaldam = request->total_damage;
  const int CLUSTERSIZE = request->cluster_size;
  const int DIRECTION = request->direction;
  const bool ISCRITICAL = request->critical;
  const char *mechmsg = request->mech_message;
  const char *mechbroadcast = request->broadcast_message;

  int hitloc = 1;
  int this_time;
  bool isrear = false;
  bool dummy = false;
  bool *dummy1 = &dummy;
  bool *dummy2 = &dummy;

  if (DIRECTION < 8) {
    hitloc = DIRECTION;
  } else if (DIRECTION < 16) {
    hitloc = DIRECTION - 8;
    isrear = true;
  } else if (DIRECTION > 21) {
    return false;
  }

  if (mechmsg && *mechmsg)
    mech_notify(mech, MECHALL, mechmsg);
  if (mechbroadcast && *mechbroadcast)
    mech_los_broadcast(mech, mechbroadcast);
  while (totaldam) {
    if (DIRECTION > 18)
      isrear = true;
    if (DIRECTION > 15)
      hitloc =
          mech_hit_location(mech, ((DIRECTION - 1) & 3) + 1, dummy1, dummy2);
    this_time = min(CLUSTERSIZE, totaldam);
    mech_damage_apply(&(MechDamageRequest){.target = mech,
                                           .attacker = mech,
                                           .line_of_sight = false,
                                           .attack_pilot = -1,
                                           .hit_location = hitloc,
                                           .rear = isrear,
                                           .critical = ISCRITICAL,
                                           .armor_damage = this_time,
                                           .internal_damage = 0,
                                           .transfer = MECH_DAMAGE_NORMAL,
                                           .cause = 0,
                                           .base_to_hit = 0,
                                           .weapon_index = -1,
                                           .ammunition_mode = 0,
                                           .ignore_swarmers = true});
    totaldam -= this_time;
  }
  return true;
}

void mech_damage(DbRef player, Mech *mech, char *buffer) {
  char *args[5];
  int damage;
  int clustersize;
  int isrear;
  int iscritical;

  if (mech_parseattributes(buffer, args, 5) != 4) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[0], &damage)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage!");
    return;
  }
  if (!parse_int_checked(args[1], &clustersize)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid cluster size!");
    return;
  }
  if (!parse_int_checked(args[2], &isrear)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid isrear flag!");
    return;
  }
  if (!parse_int_checked(args[3], &iscritical)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid iscritical flag!");
    return;
  }
  if (damage <= 0 || damage > 1000) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage!");
    return;
  }
  if (clustersize <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid cluster size!");
    return;
  }
  if (clustersize > damage) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Invalid cluster size! (must be smaller than damage amount, but > 0)");
    return;
  }
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "No MW killings!");
    return;
  }
  MissileHitsRequest request = {
      .attacker = mech,
      .target = mech,
      .target_hex = {.x = -1, .y = -1},
      .rear = isrear != 0,
      .critical = iscritical != 0,
      .weapon = {.weapon_index = 0},
      .fire_mode = -1,
      .ammunition_mode = -1,
      .missile_count = clustersize,
      .damage_per_missile = damage / clustersize,
      .salvo_size = 1,
  };
  mech_missile_apply_hits(&request);
}

void mech_damage_section(DbRef player, Mech *mech, char *buffer) {
  char *args[5];
  int damage;
  int isrear;
  int iscritical;
  int section;

  /* ARGS: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL> */

  if (mech_parseattributes(buffer, args, 5) != 4) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid Arguments: <SECTION> <DAMAGE> <ISREAR> <ISCRITICAL>");
    return;
  }

  section = armor_section_from_string(mech_class(mech),
                                      mech_movement_type(mech), args[0]);

  if (section == -1) {
    invalid_section(player, mech);
    return;
  }

  if (!parse_int_checked(args[1], &damage)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage (Arg 2) amount! (Must be a number!)");
    return;
  }
  if (!parse_int_checked(args[2], &isrear)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Isrear value (Arg 3) Invalid! (1 or 0)");
    return;
  }
  if (!parse_int_checked(args[3], &iscritical)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Iscritical value (Arg 4) Invalid! (1 or 0)");
    return;
  }
  if (damage <= 0 || damage > 1000) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid damage (Arg 2 amount! (Must be >0 or <1000)");
    return;
  }
  mech_damage_apply(&(MechDamageRequest){.target = mech,
                                         .attacker = mech,
                                         .line_of_sight = false,
                                         .attack_pilot = -1,
                                         .hit_location = section,
                                         .rear = isrear != 0,
                                         .critical = iscritical != 0,
                                         .armor_damage = damage,
                                         .internal_damage = 0,
                                         .transfer = MECH_DAMAGE_NORMAL,
                                         .cause = 0,
                                         .base_to_hit = 0,
                                         .weapon_index = -1,
                                         .ammunition_mode = 0,
                                         .ignore_swarmers = true});
}
