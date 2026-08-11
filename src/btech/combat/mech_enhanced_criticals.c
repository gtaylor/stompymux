/* Implements BattleTech combat mechanics for unit enhanced criticals. */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "mech_api_types.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static const char *weapon_display_name(int weapon_index) {
  return checked_string_suffix(weapon_catalogue_name(weapon_index), 3);
}

static int weapon_status_critical(int *criticals, size_t index) {
  return *(const int *)checked_storage_at_const(criticals, MAX_WEAPS_SECTION,
                                                sizeof(*criticals), index);
}

static unsigned char weapon_status_index(unsigned char *weapons, size_t index) {
  return *(const unsigned char *)checked_storage_at_const(
      weapons, MAX_WEAPS_SECTION, sizeof(*weapons), index);
}

static void mech_weapon_damage_info_show(DbRef player, Mech *mech, int section,
                                         int critical);

static void mech_weapon_critical_data(Mech *mech, int section, int critical,
                                      int *weapon_index, int *weapon_size,
                                      int *first_critical) {
  int w_crit_type = 0;

  /* Get the crit type */
  w_crit_type = mech_critical_part_type(mech, section, critical);

  /* Get the weapon index */
  *weapon_index = weapon_from_equipment_index(w_crit_type);

  /* Get the max number of crits for this weapon */
  *weapon_size = get_weapon_crits(mech, *weapon_index);

  /* Find the first crit */
  *first_critical = mech_weapon_first_critical(&(WeaponCriticalSearch){
      .mech = mech,
      .weapon = {.section = section, .critical = critical},
      .start_critical = 0,
      .part_type = w_crit_type,
      .maximum_criticals = *weapon_size,
  });
}

int mech_weapon_critical_to_hit_modifier(
    const WeaponCriticalToHitRequest *request) {
  Mech *mech = request->mech;
  const int SECTION = request->slot.section;
  const int CRITICAL = request->slot.critical;
  const WeaponRangeBracket RANGE_BRACKET = request->range_bracket;
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int w_ret_mod = 0;
  int count = 0;
  int nloc, ncrit;

  if (mech_class(mech) != CLASS_MECH)
    return 0;

  mech_weapon_critical_data(mech, SECTION, CRITICAL, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, SECTION, i) & WEAP_DAM_MODERATE)
      w_ret_mod++;

    if ((mech_critical_damage_flags(mech, SECTION, i) &
         (WEAP_DAM_EN_FOCUS | WEAP_DAM_MSL_RANGING)) &&
        RANGE_BRACKET != RANGE_SHORT)
      w_ret_mod++;
    count++;
  }

  if (count < w_weap_size) { // got split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){SECTION, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_MODERATE)
          w_ret_mod++;
        if ((mech_critical_damage_flags(mech, nloc, i) &
             (WEAP_DAM_EN_FOCUS | WEAP_DAM_MSL_RANGING)) &&
            RANGE_BRACKET != RANGE_SHORT)
          w_ret_mod++;
      }
    }
  }

  return w_ret_mod;
}

int mech_weapon_critical_heat_modifier(Mech *mech, int section, int critical) {
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int w_ret_mod = 0;
  int count = 0, nloc, ncrit;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  if (!weapon_catalogue_is_energy(w_weap_index))
    return 0;

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_EN_CRYSTAL)
      w_ret_mod++;
    count++;
  }

  if (count < w_weap_size && mech_class(mech) == CLASS_MECH) { // split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){section, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_EN_CRYSTAL)
          w_ret_mod++;
      }
    }
  }

  return w_ret_mod;
}

int mech_weapon_critical_damage_penalty(Mech *mech, int section, int critical) {
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int w_ret_mod = 0;
  int count = 0, nloc, ncrit;

  if (mech_class(mech) != CLASS_MECH)
    return 0;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  if (!weapon_catalogue_is_energy(w_weap_index))
    return 0;

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_EN_FOCUS)
      w_ret_mod++;
    count++;
  }

  if (count < w_weap_size) { // got split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){section, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_EN_FOCUS)
          w_ret_mod++;
      }
    }
  }

  return w_ret_mod;
}

bool mech_weapon_critical_can_explode(const WeaponCriticalRoll *request) {
  Mech *mech = request->mech;
  const int SECTION = request->slot.section;
  const int CRITICAL = request->slot.critical;
  const int ROLL = request->roll;
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int w_explosion_check = 0;
  int count = 0, nloc, ncrit;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, SECTION, CRITICAL, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, SECTION, i) &
        (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
      w_explosion_check++;
    count++;
  }

  if (count < w_weap_size) { // got split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){SECTION, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) &
            (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
          w_explosion_check++;
      }
    }
  }

  if (w_explosion_check > 0)
    w_explosion_check += 1;

  return w_explosion_check >= ROLL;
}

bool mech_weapon_critical_can_jam(const WeaponCriticalRoll *request) {
  Mech *mech = request->mech;
  const int SECTION = request->slot.section;
  const int CRITICAL = request->slot.critical;
  const int ROLL = request->roll;
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int w_jam_check = 0;
  int count = 0, nloc, ncrit;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, SECTION, CRITICAL, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, SECTION, i) & WEAP_DAM_BALL_BARREL)
      w_jam_check++;
    count++;
  }

  if (count < w_weap_size) { // got split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){SECTION, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_BALL_BARREL)
          w_jam_check++;
      }
    }
  }

  if (w_jam_check > 0)
    w_jam_check += 1;

  return w_jam_check >= ROLL;
}

bool mech_weapon_ammo_feed_is_locked(Mech *mech, int section, int critical) {
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int i;
  int count = 0, nloc, ncrit;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_damage_flags(mech, section, i) &
        (WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
      return true;
    count++;
  }

  if (count < w_weap_size) { // got split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){section, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) &
            (WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
          return true;
      }
    }
  }

  return false;
}

int mech_weapon_damaged_slot_count_at(Mech *mech, int section, int critical) {
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  return mech_weapon_damaged_slot_count(mech, section, w_first_crit,
                                        w_weap_size);
}

int mech_weapon_damaged_slot_count(Mech *mech, int section, int w_first_crit,
                                   int w_weap_size) {
  int w_crits_damaged = 0;
  int i;
  int count = 0, nloc, ncrit;

  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_is_damaged(mech, section, i))
      w_crits_damaged++;
    count++;
  }

  if (count < w_weap_size && mech_class(mech) == CLASS_MECH) { // split crits
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){section, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_is_damaged(mech, nloc, i))
          w_crits_damaged++;
      }
    }
  }

  return w_crits_damaged;
}

bool mech_weapon_critical_should_destroy(Mech *mech, int section, int critical,
                                         bool increment_count) {
  int w_crits_damaged = 0;
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;

  if (mech_class(mech) != CLASS_MECH)
    return true;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  if (increment_count)
    w_crits_damaged++;

  w_crits_damaged +=
      mech_weapon_damaged_slot_count(mech, section, w_first_crit, w_weap_size);

  return (w_crits_damaged * 2) > w_weap_size;
}

void mech_weapon_critical_apply(const WeaponCriticalApplication *application) {
  Mech *mech = application->mech;
  const int SECTION = application->slot.section;
  const int CRITICAL = application->slot.critical;
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  int w_crit_roll = btech_random_roll(mech_context(mech));
  int t_destroy_weapon = 0;
  int t_no_crit = 0;
  int t_moderate_crit = 0;

  mech_weapon_critical_data(mech, SECTION, CRITICAL, &w_weap_index,
                            &w_weap_size, &w_first_crit);

  /* See if we should just destroy the sucker outright */
  if (mech_weapon_critical_should_destroy(mech, SECTION, CRITICAL, true))
    t_destroy_weapon = 1;
  else {
    /* Add the total number of damaged slots */
    w_crit_roll += mech_weapon_damaged_slot_count(mech, SECTION, w_first_crit,
                                                  w_weap_size);
    w_crit_roll++;
  }

  if (!t_destroy_weapon) {
    /* See what damage we do */
    if (weapon_catalogue_is_energy(w_weap_index)) {
      if (w_crit_roll <= 3) {
        t_no_crit = 1;
      } else if (w_crit_roll <= 5) {
        t_moderate_crit = 1;
      } else if (w_crit_roll <= 7) {
        mech_printf(
            mech, MECHALL,
            "Your %s's focusing mechanism gets knocked out of alignment!!",
            weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_EN_FOCUS);
      } else if (w_crit_roll <= 9) {
        mech_printf(mech, MECHALL,
                    "Your %s's charging crystal takes a direct hit!!",
                    weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_EN_CRYSTAL);
      } else {
        t_destroy_weapon = 1;
      }
    } else if (weapon_catalogue_is_missile(w_weap_index)) {
      if (w_crit_roll <= 3) {
        t_no_crit = 1;
      } else if (w_crit_roll <= 5) {
        t_moderate_crit = 1;
      } else if (w_crit_roll <= 7) {
        mech_printf(mech, MECHALL, "Your %s's ranging system takes a hit!!",
                    weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_MSL_RANGING);
      } else if (w_crit_roll <= 9) {
        mech_printf(mech, MECHALL, "Your %s's ammo feed is damaged!!",
                    weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_MSL_AMMO);
      } else {
        t_destroy_weapon = 1;
      }
    } else if (weapon_catalogue_is_ballistic(w_weap_index) ||
               weapon_catalogue_is_artillery(w_weap_index)) {
      if (w_crit_roll <= 3) {
        t_no_crit = 1;
      } else if (w_crit_roll <= 5) {
        t_moderate_crit = 1;
      } else if (w_crit_roll <= 7) {
        mech_printf(mech, MECHALL, "Your %s's barrel warps from the damage!!",
                    weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_BALL_BARREL);
      } else if (w_crit_roll <= 9) {
        mech_printf(mech, MECHALL, "Your %s's ammo feed is damaged!!",
                    weapon_display_name(w_weap_index));
        mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                       WEAP_DAM_BALL_AMMO);
      } else {
        t_destroy_weapon = 1;
      }
    } else {
      t_destroy_weapon = 1;
    }
  }

  if (t_destroy_weapon) {
    mech_printf(mech, MECHALL, "Your %s has been destroyed!!",
                weapon_display_name(w_weap_index));
    mech_weapon_destroy(&(WeaponDestructionRequest){
        .mech = mech,
        .first = {.section = SECTION, .critical = w_first_crit},
        .part_type = mech_critical_part_type(mech, SECTION, CRITICAL),
        .criticals_to_destroy = 1,
        .total_criticals = w_weap_size});
  } else {
    mech_critical_fire_mode_add(mech, SECTION, CRITICAL, DAMAGED_MODE);

    if (t_no_crit)
      mech_printf(mech, MECHALL,
                  "Your %s takes a hit but suffers no noticeable damage!!",
                  weapon_display_name(w_weap_index));
    else if (t_moderate_crit) {
      mech_printf(mech, MECHALL, "Your %s takes a hit but continues working!!",
                  weapon_display_name(w_weap_index));
      mech_critical_damage_flags_add(mech, SECTION, CRITICAL,
                                     WEAP_DAM_MODERATE);
    }
  }
}

void mech_weapon_status(DbRef player, Mech *mech, char *buffer) {
  int sec_iter = 0;
  int weap_iter = 0;
  int w_weaps_in_sec = 0;
  int wc_weaps = 0;
  int w_damaged_slots = 0;
  unsigned char weaparray[MAX_WEAPS_SECTION] = {0};
  unsigned char weapdata[MAX_WEAPS_SECTION] = {0};
  int critical[MAX_WEAPS_SECTION] = {0};
  char tempbuff[LBUF_SIZE] = {0};
  char str_location[80] = {0};
  char weapbuff[LBUF_SIZE] = {0};

  if (!common_checks(player, mech, MECH_USUALSP))
    return;

  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "=========================WEAPON SYSTEMS "
               "STATUS=========================");
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "[##] -------- Weapon Name -------- || Location -------- || "
               "Status -----");

  for (sec_iter = 0; sec_iter < NUM_SECTIONS; sec_iter++) {
    w_weaps_in_sec =
        find_weapons_advanced(mech, sec_iter, weaparray, weapdata, critical, 1);

    if (w_weaps_in_sec <= 0)
      continue;

    armor_string_from_index(sec_iter, tempbuff, mech_class(mech),
                            mech_movement_type(mech));
    (void)snprintf(str_location, sizeof(str_location), "%-18.18s", tempbuff);

    for (weap_iter = 0; weap_iter < w_weaps_in_sec; weap_iter++) {
      const unsigned char WEAPON_INDEX =
          weapon_status_index(weaparray, (size_t)weap_iter);
      const int WEAPON_CRITICAL =
          weapon_status_critical(critical, (size_t)weap_iter);
      (void)snprintf(weapbuff, sizeof(weapbuff), "[%2d] %-29.29s || ",
                     wc_weaps++, weapon_display_name(WEAPON_INDEX));

      strlcat(weapbuff, str_location, sizeof(weapbuff));
      w_damaged_slots = 0;

      if (mech_critical_is_broken(mech, sec_iter, WEAPON_CRITICAL) ||
          mech_critical_temporary_failure(mech, sec_iter, WEAPON_CRITICAL) ==
              FAIL_DESTROYED)
        strlcat(weapbuff, "|| [fg=red bold]DESTROYED[reset]", sizeof(weapbuff));
      else {

        if (mech_class(mech) == CLASS_MECH)
          w_damaged_slots = mech_weapon_damaged_slot_count_at(mech, sec_iter,
                                                              WEAPON_CRITICAL);

        if (mech_critical_is_disabled(mech, sec_iter, WEAPON_CRITICAL))
          strlcat(weapbuff, "|| [fg=red bold]DISABLED[reset]",
                  sizeof(weapbuff));
        else if (mech_critical_temporary_failure(mech, sec_iter,
                                                 WEAPON_CRITICAL)) {
          switch (mech_critical_temporary_failure(mech, sec_iter,
                                                  WEAPON_CRITICAL)) {
          case FAIL_JAMMED:
            strlcat(weapbuff, "|| [fg=yellow]JAMMED[reset]", sizeof(weapbuff));
            break;
          case FAIL_SHORTED:
            strlcat(weapbuff, "|| [fg=blue]SHORTED[reset]", sizeof(weapbuff));
            break;
          case FAIL_EMPTY:
            strlcat(weapbuff, "|| [fg=cyan]EMPTY[reset]", sizeof(weapbuff));
            break;
          case FAIL_DUD:
            strlcat(weapbuff, "|| [fg=yellow]DUD[reset]", sizeof(weapbuff));
            break;
          case FAIL_AMMOJAMMED:
            strlcat(weapbuff, "|| [fg=yellow]AMMOJAM[reset]", sizeof(weapbuff));
            break;
          }
        } else if (w_damaged_slots > 0)
          strlcat(weapbuff, "|| [fg=yellow bold]DAMAGED[reset]",
                  sizeof(weapbuff));
        else
          strlcat(weapbuff, "|| [fg=green bold]OPERATIONAL[reset]",
                  sizeof(weapbuff));
      }

      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   weapbuff);

      mech_weapon_damage_info_show(player, mech, sec_iter, WEAPON_CRITICAL);
    }
  }
}

static void mech_weapon_damage_info_show(DbRef player, Mech *mech, int section,
                                         int critical) {
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  int w_weap_size = 0;
  int w_first_crit = 0;
  int w_weap_index = 0;
  struct {
    int general;
    int primary;
    int secondary;
  } damage = {0};
  int i;
  int t_has_damaged_part = 0;
  int t_print_space = 0;
  int w_ammo_loc = mech_critical_desired_ammo_section(mech, section, critical);
  char str_location[80];
  struct {
    int damaged;
    int destroyed;
    int disabled;
  } non_operational = {0};
  int damflag;
  int count = 0, nloc, ncrit;

  mech_weapon_critical_data(mech, section, critical, &w_weap_index,
                            &w_weap_size, &w_first_crit);
  const WeaponRangeProfile RANGES = weapon_catalogue_ranges(w_weap_index);

  for (i = w_first_crit; i < min(NUM_CRITICALS, w_first_crit + w_weap_size);
       i++) {
    if (mech_critical_is_damaged(mech, section, i)) {
      t_has_damaged_part = 1;
      damflag = mech_critical_damage_flags(mech, section, i);
      damage.general += damflag & WEAP_DAM_MODERATE;
      damage.primary += (damflag & (WEAP_DAM_EN_FOCUS | WEAP_DAM_BALL_BARREL |
                                    WEAP_DAM_MSL_RANGING));
      damage.secondary += (damflag & (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO |
                                      WEAP_DAM_MSL_AMMO));
      non_operational.damaged++;
    } else if (mech_critical_is_destroyed(mech, section, i)) {
      non_operational.destroyed++;
    } else if (mech_critical_is_disabled(mech, section, i)) {
      non_operational.disabled++;
    }
  }

  if (count < w_weap_size && mech_class(mech) == CLASS_MECH) {
    SplitCriticalLookup split_lookup = split_critical_find(
        mech, (CriticalSlotReference){section, w_first_crit});
    if (split_lookup.found) {
      nloc = split_lookup.slot.section;
      ncrit = split_lookup.slot.critical;
      for (i = ncrit; i < (w_weap_size - count); i++) {
        if (mech_critical_is_damaged(mech, nloc, i)) {
          t_has_damaged_part = 1;
          damflag = mech_critical_damage_flags(mech, nloc, i);
          damage.general += damflag & WEAP_DAM_MODERATE;
          damage.primary +=
              (damflag & (WEAP_DAM_EN_FOCUS | WEAP_DAM_BALL_BARREL |
                          WEAP_DAM_MSL_RANGING));
          damage.secondary +=
              (damflag &
               (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO));
          non_operational.damaged++;
        } else if (mech_critical_is_destroyed(mech, nloc, i)) {
          non_operational.destroyed++;
        } else if (mech_critical_is_disabled(mech, nloc, i)) {
          non_operational.disabled++;
        }
      }
    }
  }

  if (t_has_damaged_part) {
    if (damage.general > 0) {
      notify_printf(
          evaluation, player, "      General damage (%d hit%s): +%d to hit.",
          damage.general, damage.general > 1 ? "s" : "", damage.general);
    }

    if (weapon_catalogue_is_energy(w_weap_index)) {
      if (damage.primary > 0) {
        notify_printf(evaluation, player,
                      "      Focus misalignment (%d hit%s): -%d damage. +%d to "
                      "hit at >%d hexes.",
                      damage.primary, damage.primary > 1 ? "s" : "",
                      damage.primary, damage.primary, RANGES.short_range);
      }

      if (damage.secondary > 0) {
        notify_printf(evaluation, player,
                      "      Charging crystal damage (%d hit%s): +%d heat. "
                      "Explodes on %d or less.[reset]",
                      damage.secondary, damage.secondary > 1 ? "s" : "",
                      damage.secondary, damage.secondary + 1);
      }
    } else if (weapon_catalogue_is_missile(w_weap_index)) {
      if (damage.primary > 0) {
        notify_printf(
            evaluation, player,
            "      Ranging system damage (%d hit%s): +%d to hit at >%d hexes.",
            damage.primary, damage.primary > 1 ? "s" : "", damage.primary,
            RANGES.short_range);
      }

      if (damage.secondary > 0) {
        notify_printf(evaluation, player,
                      "      Ammo feed damage (%d hit%s): Can't switch ammo. "
                      "Explodes on %d or less.",
                      damage.secondary, damage.secondary > 1 ? "s" : "",
                      damage.secondary + 1);
      }
    } else if (weapon_catalogue_is_ballistic(w_weap_index) ||
               weapon_catalogue_is_artillery(w_weap_index)) {
      if (damage.primary > 0) {
        notify_printf(evaluation, player,
                      "      [fg=red bold]Barrel damage (%d hit%s): Jams on a "
                      "%d or less.[reset]",
                      damage.primary, damage.primary > 1 ? "s" : "",
                      damage.primary + 1);
      }

      if (damage.secondary > 0) {
        notify_printf(evaluation, player,
                      "      Ammo feed damage (%d hit%s): Can't switch ammo. "
                      "Explodes on %d or less.",
                      damage.secondary, damage.secondary > 1 ? "s" : "",
                      damage.secondary + 1);
      }
    }

    if (damage.general == 0 && damage.primary == 0 && damage.secondary == 0)
      notify_printf(evaluation, player,
                    "      Damaged, but fully operational.");
    t_print_space = 1;
  }

  if (w_ammo_loc >= 0) {
    armor_string_from_index(w_ammo_loc, str_location, mech_class(mech),
                            mech_movement_type(mech));
    notify_printf(evaluation, player, "      Prefered ammo source: %s",
                  str_location);
    t_print_space = 1;
  }

  if (non_operational.damaged > 0 || non_operational.destroyed > 0 ||
      non_operational.disabled > 0) {
    notify_printf(evaluation, player,
                  "      Slot status: Damaged: %d. Destroyed: %d. Disabled: %d",
                  non_operational.damaged, non_operational.destroyed,
                  non_operational.disabled);
    t_print_space = 1;
  }

  if (t_print_space)
    mecha_notify(evaluation, player, " ");
}
