#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "equipment_types.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_template_api.h"
#include "mech_utils_api.h"
#include "mech_utils_internal.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

void ArmorStringFromIndex(int index, char *buffer, UnitClass type,
                          MechMovementType movement_type) {
  size_t location_count = mech_section_name_count(type, movement_type);
  if (index >= 0 && (size_t)index < location_count) {
    // NOLINTNEXTLINE(clang-analyzer-security.insecureAPI.strcpy)
    strcpy(buffer, mech_section_name(type, movement_type, (size_t)index));
    return;
  }
  // NOLINTNEXTLINE(clang-analyzer-security.insecureAPI.strcpy)
  strcpy(buffer, "Invalid!!");
}

int IsInWeaponArc(Mech *mech, float x, float y, int section, int critical) {
  int weaponarc, isrear;
  int wantarc = NOARC;

  if (((mech)->ud.type) == CLASS_MECH &&
      (section == LLEG || section == RLEG ||
       (mech_is_quad(mech) && (section == LARM || section == RARM)))) {
    int ts = ((mech)->rd.status) & (TORSO_LEFT | TORSO_RIGHT);
    ((mech)->rd.status) &= ~(ts);
    weaponarc = InWeaponArc(mech, x, y);
    ((mech)->rd.status) |= ts;
  } else
    weaponarc = InWeaponArc(mech, x, y);

  switch (((mech)->ud.type)) {
  case CLASS_MECH:
  case CLASS_BSUIT:
  case CLASS_MW:
    if (mech_critical_fire_mode(mech, section, critical) & REAR_MOUNT)
      wantarc = REARARC;
    else if (section == LARM && (((mech)->rd.status) & FLIPPED_ARMS))
      wantarc = REARARC | LSIDEARC;
    else if (section == LARM)
      wantarc = FORWARDARC | LSIDEARC;
    else if (section == RARM && (((mech)->rd.status) & FLIPPED_ARMS))
      wantarc = REARARC | RSIDEARC;
    else if (section == RARM)
      wantarc = FORWARDARC | RSIDEARC;
    else
      wantarc = FORWARDARC;
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
    switch (section) {
    case TURRET:
      wantarc = TURRETARC;
      break;
    case FSIDE:
      wantarc = FORWARDARC;
      break;
    case LSIDE:
      wantarc = LSIDEARC;
      break;
    case RSIDE:
      wantarc = RSIDEARC;
      break;
    case BSIDE:
      wantarc = REARARC;
      break;
    }
    break;
  case CLASS_DS:
    switch (section) {
    case DS_NOSE:
      wantarc = FORWARDARC;
      break;
    case DS_LWING:
    case DS_LRWING:
      wantarc = LSIDEARC;
      break;
    case DS_RWING:
    case DS_RRWING:
      wantarc = RSIDEARC;
      break;
    case DS_AFT:
      wantarc = REARARC;
      break;
    }
    break;
  case CLASS_SPHEROID_DS:
    switch (section) {
    case DS_NOSE:
      wantarc = FORWARDARC;
      break;
    case DS_LWING:
      wantarc = FORWARDARC | LSIDEARC;
      break;
    case DS_LRWING:
      wantarc = REARARC | LSIDEARC;
      break;
    case DS_RWING:
      wantarc = FORWARDARC | RSIDEARC;
      break;
    case DS_RRWING:
      wantarc = REARARC | RSIDEARC;
      break;
    case DS_AFT:
      wantarc = REARARC;
      break;
    }
    break;

  case CLASS_AERO:
    isrear = (mech_critical_fire_mode(mech, section, critical) & REAR_MOUNT);
    switch (section) {
    case AERO_NOSE:
      wantarc = FORWARDARC | LSIDEARC | RSIDEARC;
      break;
    case AERO_LWING:
      wantarc = LSIDEARC | (isrear ? REARARC : FORWARDARC);
      break;
    case AERO_RWING:
      wantarc = RSIDEARC | (isrear ? REARARC : FORWARDARC);
      break;
    case AERO_AFT:
      wantarc = REARARC;
      break;
    }
    break;
  }
  return wantarc ? (wantarc & weaponarc) : 0;
}

int GetWeaponCrits(Mech *mech, int weapindx) {
  return (((mech)->ud.type) == CLASS_MECH)
             ? weapon_catalogue_critical_slots(weapindx)
             : 1;
}

int listmatch(const char *const *values, size_t value_count,
              const char *match) {
  for (size_t i = 0; i < value_count; i++) {
    const char *const *value =
        checked_storage_at_const(values, value_count, sizeof(*values), i);
    if (!strcasecmp(*value, match))
      return clamp_size_to_int(i);
  }
  return -1;
}

/* Takes care of :
   JumpSpeed
   Numsinks

   TODO: More support(?)
 */

void do_sub_magic(Mech *mech, int loud) {
  int jjs = 0;
  int hses = 0;
  int wanths, wanths_f;
  int shs_size = mech_heat_sink_critical_size(mech);
  int hs_eff = mech_has_double_heat_sinks(mech) ? 2 : 1;
  int i, j;
  int inthses = mech_engine_rating(mech) / 25;
  int dest_hses = 0;
  int maxjjs =
      (int)((float)((mech)->ud.maxspeed) * MP_PER_KPH *
            ((!(((mech)->rd.specials2) & IMPROVED_JJ_TECH)) ? (2 / 3) : 1));

  if (((mech)->rd.specials) & ICE_TECH)
    inthses = 0;
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++)
      switch (
          special_from_equipment_index(mech_critical_part_type(mech, i, j))) {
      case HEAT_SINK:
        hses++;
        if (mech_critical_is_nonfunctional(mech, i, j))
          dest_hses++;
        break;
      case JUMP_JET:
        jjs++;
        break;
      }
  if (((mech)->ud.hsengoverride))
    inthses = ((mech)->ud.hsengoverride);
  hses += MIN(((mech)->ud.numsinks) * shs_size / hs_eff, inthses * shs_size);

  /* Improved are 2 crits per Jump MP */
  if ((((mech)->rd.specials2) & IMPROVED_JJ_TECH))
    jjs = jjs / 2;

  if (jjs > maxjjs) {
    if (loud)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Error in #%ld (%s): %d JJs, yet %d maximum available "
                  "(due to walk MPs)?",
                  mech->mynum, ((mech)->ud.mech_type), jjs, maxjjs));

    jjs = maxjjs;
  }
  ((mech)->rd.jumpspeed) = MP1 * (float)jjs;
  wanths_f = (hses / shs_size) * hs_eff;
  wanths = wanths_f - (dest_hses * hs_eff / shs_size);
  if (loud)
    ((mech)->rd.onumsinks) =
        wanths - MIN(((mech)->ud.numsinks), inthses * hs_eff);
  if (wanths != ((mech)->ud.numsinks) && loud) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in #%ld (%s): Set HS: %d. Existing HS: %d. "
                "Difference: %d. Please %s.",
                mech->mynum, ((mech)->ud.mech_type), ((mech)->ud.numsinks),
                wanths, ((mech)->ud.numsinks) - wanths,
                wanths < ((mech)->ud.numsinks) ? "add the extra HS critical(s)"
                                               : "fix the template"));
  } else
    ((mech)->ud.numsinks) = clamp_int_to_char(wanths);
  ((mech)->rd.onumsinks) = wanths_f;

  if ((((mech)->rd.onumsinks) * shs_size / hs_eff -
       (((mech)->rd.specials) & ICE_TECH ? 0 : 10) * shs_size) < 0)
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in #%ld (%s): HS less then max possible in engine!",
                mech->mynum, ((mech)->ud.mech_type)));
}

/* Values to take care of:
   - JumpSpeed
   - MaxSpeed
   - Numsinks
   - EngineHeat
   - PilotSkillBase
   - LRS/Tac/ScanRange
   - BTH

   Status:
   - Destroyed

   Critstatus:
   - kokonaan paitsi

   section(s) / basetohit
 */

void do_fixextra(Mech *mech) {

  int i, j;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_is_flooded(mech, i))
      mech_section_flooded_set(mech, i, false);
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (!equipment_is_ammunition(mech_critical_part_type(mech, i, j))) {
        if (!mech_critical_is_broken(mech, i, j) &&
            !mech_critical_is_destroyed(mech, i, j))
          mech_RepairPart(mech, i, j);
        else {
          mech_critical_fire_mode_clear(mech, i, j, DISABLED_MODE);
          mech_RepairPart(mech, i, j);
        }
      } else {
        mech_critical_fire_mode_clear(mech, i, j, DISABLED_MODE);
        mech_FillPartAmmo(mech, i, j);
      }
    }
  }
}

void do_magic(Mech *mech) {
  Mech opp;
  int i, j, t;
  int mask = 0;
  int tankCritMask = 0;

  if (((mech)->ud.type) != CLASS_MECH)
    tankCritMask =
        (TURRET_LOCKED | TURRET_JAMMED | TAIL_ROTOR_DESTROYED | CREW_STUNNED);

  /* stop the burning */
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  mech_performing_action_set(mech, false);

  memcpy(&opp, mech, sizeof(Mech));
  mech_template_load(GOD, &opp, ((mech)->ud.mech_type));
  ((mech)->rd.erat) =
      mech_calculated_engine_rating(&opp); /* From intact template */
  opp.mynum = -1;
  /* Ok.. It's at perfect condition. Start inflicting some serious crits.. */
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      mech_critical_part_type_set(&opp, i, j,
                                  mech_critical_part_type(mech, i, j));
      mech_critical_brand_set(&opp, i, j, mech_critical_brand(mech, i, j));
      mech_critical_data_set(&opp, i, j, 0);
      mech_critical_fire_mode_set(&opp, i, j, 0);
      mech_critical_ammo_mode_set(&opp, i, j, 0);
    }
  if (((mech)->ud.type) == CLASS_MECH)
    do_sub_magic(&opp, 0);
  ((mech)->rd.onumsinks) = ((&opp)->rd.onumsinks);
  for (i = 0; i < NUM_SECTIONS; i++) {

    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (mech_critical_is_destroyed(mech, i, j)) {
        if (!mech_critical_is_destroyed(&opp, i, j)) {
          if (!equipment_is_ammunition(
                  (t = mech_critical_part_type(mech, i, j)))) {
            if (!equipment_is_weapon(t))
              if (((mech)->ud.type) == CLASS_MECH)
                mech_critical_effect_apply(&opp, nullptr, 0, i, j, t,
                                           mech_critical_data(mech, i, j));
          }
        }
      } else {
        t = mech_critical_part_type(mech, i, j);
        if (weapon_catalogue_is_anti_missile(weapon_from_equipment_index(t))) {
          if (weapon_catalogue_has_special(weapon_from_equipment_index(t),
                                           CLAT))
            ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
          else
            ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
        }
        mech_critical_fire_mode_clear(
            mech, i, j, OS_USED | ROCKET_FIRED | IS_JETTISONED_MODE);
      }
    }

    mech_section_configuration_remove(mech, i, STABILIZERS_DESTROYED);

    if (mech_section_is_destroyed(mech, i))
      mech_section_destroy(&opp, nullptr, 0, i);
    if (((mech)->pd.stall) > 0)
      mech_section_breached_set(
          mech, i, false); /* Just in case ; this leads to 'unbreachable'
                                     legs once you've 'done your time' once */
  }
  mech_jump_speed_set(mech, mech_jump_speed(&opp));
  mech_maximum_speed_set(mech, mech_maximum_speed(&opp));
  mech_heat_sink_count_set(mech, mech_heat_sink_count(&opp));
  mech_engine_heat_set(mech, mech_engine_heat(&opp));
  mech_pilot_skill_modifier_set(mech, mech_pilot_skill_modifier(&opp));
  mech_long_range_sensor_range_set(mech, mech_long_range_sensor_range(&opp));
  mech_tactical_range_set(mech, mech_tactical_range(&opp));
  mech_scanner_range_set(mech, mech_scanner_range(&opp));
  mech_base_to_hit_modifier_set(mech, mech_base_to_hit_modifier(&opp));
  ((mech)->rd.critstatus) &= mask;
  ((mech)->rd.critstatus) |= ((&opp)->rd.critstatus) & (~mask);

  ((mech)->rd.tankcritstatus) &= tankCritMask;
  ((mech)->rd.tankcritstatus) |= ((&opp)->rd.tankcritstatus) & (~tankCritMask);

  for (i = 0; i < NUM_SECTIONS; i++) {
    mech_section_base_to_hit_set(mech, i, mech_section_base_to_hit(&opp, i));
    mech_section_specials_set(mech, i, mech_section_specials(&opp, i));
    mech_section_special_remove(mech, i,
                                INARC_HOMING_ATTACHED | INARC_HAYWIRE_ATTACHED |
                                    INARC_ECM_ATTACHED |
                                    INARC_NEMESIS_ATTACHED);
  }

  /* Case of undestroying */
  if (!mech_is_destroyed(&opp) && mech_is_destroyed(mech))
    ((mech)->rd.status) &= ~DESTROYED;
  else if (mech_is_destroyed(&opp) && !mech_is_destroyed(mech))
    ((mech)->rd.status) |= DESTROYED;
  if (!mech_is_destroyed(mech) && ((mech)->ud.type) != CLASS_MECH)
    mech_fallen_set(mech, mech_is_fallen(&opp));
  update_specials(mech);
}

void mech_RepairPart(Mech *mech, int loc, int pos) {
  int t = mech_critical_part_type(mech, loc, pos);

  mech_critical_restore(mech, loc, pos);
  if (equipment_is_weapon(t) || equipment_is_ammunition(t)) {
    mech_critical_data_set(mech, loc, pos, 0);
    mech_critical_fire_mode_clear(mech, loc, pos,
                                  OS_USED | IS_JETTISONED_MODE | ROCKET_FIRED);
  } else if (equipment_is_special(t)) {
    switch (special_from_equipment_index(t)) {
    case TARGETING_COMPUTER:
    case HEAT_SINK:
    case LIFE_SUPPORT:
    case COCKPIT:
    case SENSORS:
    case JUMP_JET:
    case ENGINE:
    case GYRO:
    case SHOULDER_OR_HIP:
    case LOWER_ACTUATOR:
    case UPPER_ACTUATOR:
    case HAND_OR_FOOT_ACTUATOR:
    case C3_MASTER:
    case C3_SLAVE:
    case C3I:
    case ECM:
    case ANGELECM:
    case NULL_SIGNATURE_SYSTEM:
    case BEAGLE_PROBE:
    case LIGHT_BAP:
    case ARTEMIS_IV:
    case TAG:
    case BLOODHOUND_PROBE:
      /* Magic stuff here :P */
      if (((mech)->ud.type) == CLASS_MECH)
        do_magic(mech);
      break;
    }
  }
}

int no_locations_destroyed(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_original_internal(mech, i) &&
        mech_section_is_destroyed(mech, i))
      return 0;
  return 1;
}

void mech_ReAttach(Mech *mech, int loc) {
  if (!mech_section_is_destroyed(mech, loc))
    return;
  mech_section_flooded_set(mech, loc, false);
  mech_section_internal_set(mech, loc,
                            mech_section_original_internal(mech, loc));
  if (mech_is_aerospace_unit(mech))
    mech_section_internal_set(mech, loc, 1);
  if (((mech)->ud.type) != CLASS_MECH) {
    if (no_locations_destroyed(mech) && mech_is_dropship(mech))
      ((mech)->rd.status) &= ~DESTROYED;
    return;
  }
}

void mech_ReplaceSuit(Mech *mech, int loc) {
  if (!mech_section_is_destroyed(mech, loc))
    return;

  mech_section_internal_set(mech, loc,
                            mech_section_original_internal(mech, loc));
}

/*
 * Added for new flood code by Kipsta
 * 8/4/99
 */

void mech_ReSeal(Mech *mech, int loc) {
  int i;

  if (mech_section_is_destroyed(mech, loc))
    return;
  if (!mech_section_is_flooded(mech, loc))
    return;

  mech_section_flooded_set(mech, loc, false);

  for (i = 0; i < CritsInLoc(mech, loc); i++) {
    if (mech_critical_is_disabled(mech, loc, i)) {
      if (!mech_critical_is_broken(mech, loc, i) &&
          !mech_critical_is_damaged(mech, loc, i))
        mech_RepairPart(mech, loc, i);
      else
        mech_critical_fire_mode_clear(mech, loc, i, DISABLED_MODE);
    }
  }
}

void mech_Detach(Mech *mech, int loc) {
  if (mech_section_is_destroyed(mech, loc))
    return;
  mech_section_destroy(mech, nullptr, 0, loc);
}

/* Figures out how much ammo there is when we're 'fully loaded', and
   fills it */
void mech_FillPartAmmo(Mech *mech, int loc, int pos) {
  int t, to;

  t = mech_critical_part_type(mech, loc, pos);

  if (!equipment_is_ammunition(t))
    return;
  if (!(to =
            weapon_catalogue_ammunition_per_ton(ammunition_to_weapon_index(t))))
    return;
  mech_critical_data_set(mech, loc, pos, FullAmmo(mech, loc, pos));
}

int AcceptableDegree(int d) {
  /*
   * Silly billies, integer modulo (division) is still faster than loops.
   * And probably slightly faster than branches, too, but let's not worry
   * about that.
   */
  if (d < 0) {
    return (d % 360) + 360;
  } else if (d >= 360) {
    return (d % 360);
  } else {
    return d;
  }
}

void MarkForLOSUpdate(Mech *mech) {
  BattleMap *mech_map;

  if (!(mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
    return;
  mech_map->moves++;
  battle_map_unit_moved_set(mech_map, mech_map_slot(mech));
}

void multi_weap_sel(Mech *mech, DbRef player, char *buffer, int bitbybit,
                    MultiWeaponSelectionCallback callback, void *context) {
  /* Insight: buffer contains stuff in form:
     <num>
     <num>-<num>
     <num>,..
     <num>-<num>,..
   */
  /* Ugly recursive piece of code :> */
  char *c;
  char empty_buffer[] = "";
  int i1, i2, i3;
  int section, critical;

  if (buffer)
    buffer =
        checked_mutable_string_suffix(buffer, strspn(buffer, " \t\n\v\f\r"));
  if (!buffer)
    buffer = empty_buffer;
  if ((c = strstr(buffer, ","))) {
    *c = 0;
    c = checked_mutable_string_suffix(c, 1);
  }
  char *range_separator = strchr(buffer, '-');
  if (range_separator != nullptr) {
    *range_separator = '\0';
    if (!parse_int_checked(buffer, &i1) ||
        !parse_int_checked(checked_string_suffix(range_separator, 1), &i2)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Invalid value: %s", buffer));
      return;
    }
    if (i1 < 0 || i1 >= MAX_WEAPONS_PER_MECH) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Invalid first number in range (%d)", i1));
      return;
    }
    if (i2 < 0 || i2 >= MAX_WEAPONS_PER_MECH) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Invalid second number in range (%d)", i2));
      return;
    }
    if (i1 > i2) {
      i3 = i1;
      i1 = i2;
      i2 = i3;
    }
  } else {
    if (!parse_int_checked(buffer, &i1)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Invalid value: %s", buffer));
      return;
    }
    if (i1 < 0 || i1 >= MAX_WEAPONS_PER_MECH) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Invalid weapon number: %d", i1));
      return;
    }
    i2 = i1;
  }
  if (bitbybit / 2) {
    if (i2 >= NUM_TICS) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("There are only %d tics!", i2));
      return;
    }
  } else {
    if (!(FindWeaponNumberOnMech(mech, i2, &section, &critical) != -1)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   tprintf("Error: the mech doesn't HAVE %d weapons!", i2 + 1));
      return;
    }
  }
  if (bitbybit % 2) {
    for (i3 = i1; i3 <= i2; i3++)
      if (callback(mech, player, i3, i3, context))
        return;
  } else if (callback(mech, player, i1, i2, context))
    return;
  if (c)
    multi_weap_sel(mech, player, c, bitbybit, callback, context);
}
