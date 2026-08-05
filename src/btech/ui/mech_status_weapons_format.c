#include "mech_status_api.h"
#include "mech_status_render_internal.h"

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "failures.h"
#include "legacy_macros.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_tag_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_settings.h"

static const char *section_recycle_status(Mech *mech, int section) {
  int recycle = mech_section_recycle_ticks(mech, section);
  if (mech_section_is_destroyed(mech, section))
    return "[fg=black bold]*****[reset]";
  if (recycle > 0)
    return tprintf("%-5d", recycle / WEAPON_TICK + recycle % WEAPON_TICK);
  return "[fg=green]Ready[reset]";
}

static const char *physical_recycle_status(Mech *mech, int section,
                                           int physical_type) {
  int recycle = mech_section_recycle_ticks(mech, section);
  if (!canUsePhysical(mech, section, physical_type))
    return "[fg=red bold]XX[reset]";
  if (recycle > 0)
    return tprintf("%-3d", recycle / WEAPON_TICK + recycle % WEAPON_TICK);
  return "[fg=green]Rdy[reset]";
}

void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size) {
  unsigned char weaparray[MAX_WEAPS_SECTION] = {0};
  unsigned char weapdata[MAX_WEAPS_SECTION] = {0};
  int critical[MAX_WEAPS_SECTION] = {0};
  unsigned char ammoweap[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammo[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammomax[8 * MAX_WEAPS_SECTION] = {0};
  unsigned int modearray[8 * MAX_WEAPS_SECTION] = {0};
  char tmpbuf[LBUF_SIZE] = {0};
  int count, ammoweapcount;
  int loop;
  int ii, i = 0;
  char weapname[LBUF_SIZE] = {0};
  char *tmpc;
  char weapbuff[LBUF_SIZE] = {0};
  char tempbuff[LBUF_SIZE] = {0};
  char location[80] = {0};
  char astrAmmoSpacer[MBUF_SIZE] = {0}; /* mem is cheap. over allocate */
  int running_sum = 0;
  short ammo_mode;
  int technology = mech_technology_flags(mech);
  int technology_secondary = mech_technology_flags_secondary(mech);
  int infantry_technology = mech_infantry_technology_flags(mech);
  MechConditionSummary conditions = mech_condition_summary(mech);
  bool has_c3_master = technology & C3_MASTER_TECH;
  bool has_c3_slave = technology & C3_SLAVE_TECH;
  bool has_c3i = technology_secondary & C3I_TECH;
  bool has_tag = (technology_secondary & TAG_TECH) || has_c3_master;

  if ((technology & ECM_TECH) || (technology_secondary & STEALTH_ARMOR_TECH) ||
      (technology_secondary & NULLSIGSYS_TECH) || (technology & SLITE_TECH) ||
      has_c3_master || has_c3_slave || has_c3i || (technology & MASC_TECH) ||
      (technology_secondary & SUPERCHARGER_TECH) ||
      (technology & TRIPLE_MYOMER_TECH) ||
      (technology_secondary & ANGEL_ECM_TECH) || has_tag ||
      (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)) {
    strcpy(tempbuff, "AdvTech: ");

    if (technology & ECM_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "ECM(%s)  ",
               conditions.ecm_destroyed ? "[fg=red bold]XX[reset]"
               : conditions.ecm_enabled
                   ? (conditions.ecm_active ? "[fg=green bold]ECM[reset]"
                                            : "[fg=red bold]ECM[reset]")
               : conditions.eccm_enabled  ? "[fg=green bold]ECCM[reset]"
               : conditions.ecm_countered ? "[fg=red]Off[reset]"
                                          : "[fg=green]Off[reset]");
    }

    if (technology_secondary & ANGEL_ECM_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "AngelECM(%s)  ",
               conditions.angel_ecm_destroyed ? "[fg=red bold]XX[reset]"
               : conditions.angel_ecm_enabled
                   ? (conditions.angel_ecm_active ? "[fg=green bold]ECM[reset]"
                                                  : "[fg=red bold]ECM[reset]")
               : conditions.angel_eccm_enabled ? "[fg=green bold]ECCM[reset]"
               : conditions.ecm_countered      ? "[fg=red]Off[reset]"
                                               : "[fg=green]Off[reset]");
    }

    if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "PersonalECM(%s)  ",
               conditions.personal_ecm_enabled
                   ? (conditions.personal_ecm_active
                          ? "[fg=green bold]ECM[reset]"
                          : "[fg=red bold]ECM[reset]")
               : conditions.personal_eccm_enabled ? "[fg=green bold]ECCM[reset]"
               : conditions.ecm_countered         ? "[fg=red]Off[reset]"
                                                  : "[fg=green]Off[reset]");
    }

    if (technology_secondary & STEALTH_ARMOR_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SthArmor(%s)  ",
               conditions.ecm_destroyed          ? "[fg=red bold]XX[reset]"
               : conditions.stealth_armor_active ? "[fg=green bold]On[reset]"
                                                 : "[fg=green]Rdy[reset]");
    }

    if (technology_secondary & NULLSIGSYS_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "NullSigSys(%s)  ",
               conditions.null_signature_destroyed ? "[fg=red bold]XX[reset]"
               : conditions.null_signature_active  ? "[fg=green bold]On[reset]"
                                                   : "[fg=green]Rdy[reset]");
    }

    if (technology & SLITE_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SLITE(%s)  ",
               conditions.searchlight_destroyed ? "[fg=red bold]XX[reset]"
               : mech_searchlight_active(mech)  ? "[fg=green bold]On[reset]"
                                                : "[fg=green]Off[reset]");
    }

    if (has_c3_master)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3M[reset]  ",
               conditions.c3_destroyed           ? "[fg=red]"
               : mech_is_any_ecm_disturbed(mech) ? "[fg=yellow]"
               : mech_c3_network_size(mech) > 0  ? "[fg=green bold]"
                                                 : "[fg=green]");

    if (has_c3_slave)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3S[reset]  ",
               conditions.c3_destroyed           ? "[fg=red]"
               : mech_is_any_ecm_disturbed(mech) ? "[fg=yellow]"
               : mech_c3_network_size(mech) > 0  ? "[fg=green bold]"
                                                 : "[fg=green]");

    if (has_c3i)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3i[reset]  ",
               conditions.c3i_destroyed          ? "[fg=red]"
               : mech_is_any_ecm_disturbed(mech) ? "[fg=yellow]"
               : mech_c3i_network_size(mech) > 0 ? "[fg=green bold]"
                                                 : "[fg=green]");

    if (technology & TRIPLE_MYOMER_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "TSM(%s)  ",
               mech_excess_heat(mech) >= 9.0 ? "[fg=green bold]On[reset]"
                                             : "[fg=green]Off[reset]");

    if (has_tag) {
      DbRef tag_target_dbref = mech_tag_target_dbref(mech);
      Mech *tag_target =
          btech_context_get_mech(mech_context(mech), tag_target_dbref);
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "TAG(%s)  ",
               mech_tag_is_destroyed(mech) ? "[fg=red bold]XX[reset]"
               : (!tag_target ||
                  mech_tagged_by_dbref(tag_target) != mech_dbref(mech))
                   ? (mech_event_count(mech, EVENT_TAG_RECYCLE)
                          ? "[fg=yellow bold]Not Rdy[reset]"
                          : "[fg=green]Rdy[reset]")
                   : tprintf("%s%s[reset]",
                             (mech_event_count(mech, EVENT_TAG_RECYCLE)
                                  ? "[fg=yellow bold]"
                                  : "[bold]"),
                             mech_to_mech_display_id(mech, tag_target).text));
    }

    if (technology_secondary & SUPERCHARGER_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SCHARGE: %s%d[reset] (%s)",
               conditions.supercharger_counter > 3   ? "[fg=red bold]"
               : conditions.supercharger_counter > 0 ? "[fg=yellow bold]"
                                                     : "[fg=green]",
               conditions.supercharger_counter,
               conditions.supercharger_enabled ? "On" : "Off");

    if (technology & MASC_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "MASC: %s%d[reset] (%s)",
               conditions.masc_counter > 3   ? "[fg=red bold]"
               : conditions.masc_counter > 0 ? "[fg=yellow bold]"
                                             : "[fg=green]",
               conditions.masc_counter, conditions.masc_enabled ? "On" : "Off");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (technology_secondary & CARRIER_TECH) {
    strcpy(tempbuff, "Carrier: ");

    snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
             "%d tons free, %d tons max unit size",
             mech_cargo_space(mech) / 100, mech_carrier_maximum_tonnage(mech));
    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((technology & AA_TECH) || (technology & BEAGLE_PROBE_TECH) ||
      (technology_secondary & BLOODHOUND_PROBE_TECH)) {
    strcpy(tempbuff, "AdvSensors:");

    if (technology & AA_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " Radar");

    if (technology & BEAGLE_PROBE_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BeagleProbe");

    if (technology_secondary & BLOODHOUND_PROBE_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BloodhoundProbe");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((infantry_technology & CS_PURIFIER_STEALTH_TECH) ||
      (infantry_technology & DC_KAGE_STEALTH_TECH) ||
      (infantry_technology & FWL_ACHILEUS_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATOR_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)) {

    strcpy(tempbuff, "AdvItems:");

    if (infantry_technology & CS_PURIFIER_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " PurifierStealth");

    if (infantry_technology & DC_KAGE_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " KageStealth");

    if (infantry_technology & FWL_ACHILEUS_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " AchileusStealth");

    if (infantry_technology & FC_INFILTRATOR_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " InfiltratorStealth");

    if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " InfiltratorIIStealth");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((infantry_technology & INF_SWARM_TECH) ||
      (infantry_technology & INF_MOUNT_TECH) ||
      (infantry_technology & INF_ANTILEG_TECH) ||
      (infantry_technology & CAN_JETTISON_TECH)) {

    strcpy(tempbuff, "Special Actions:");

    if (infantry_technology & INF_MOUNT_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " MountFriends");

    if (infantry_technology & INF_SWARM_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " SwarmAttack");

    if (infantry_technology & INF_ANTILEG_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " AntiLegAttack");

    if (infantry_technology & CAN_JETTISON_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BackPackJettison");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (infantry_technology & MUST_JETTISON_TECH) {
    strcpy(tempbuff, "Requirements: Must jettison backpack before using "
                     "special abilities or jumping");
    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }
  mech_update_recycling(mech);
  if (mech_class(mech) == CLASS_MECH && !compact) {
    tempbuff[0] = 0;

#define SHOW(part, loc)                                                        \
  snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),   \
           "%s: %s ", part, loc)

    bool is_quad = mech_movement_type(mech) == MOVE_QUAD;
    SHOW(is_quad ? "FLLEG" : "LARM", section_recycle_status(mech, LARM));
    SHOW(is_quad ? "FRLEG" : "RARM", section_recycle_status(mech, RARM));
    SHOW(is_quad ? "RLLEG" : "LLEG", section_recycle_status(mech, LLEG));
    SHOW(is_quad ? "RRLEG" : "RLEG", section_recycle_status(mech, RLEG));

    if (hasPhysical(mech, LARM, PHY_AXE))
      SHOW("Axe[LA]", physical_recycle_status(mech, LARM, PHY_AXE));

    if (hasPhysical(mech, RARM, PHY_AXE))
      SHOW("Axe[RA]", physical_recycle_status(mech, RARM, PHY_AXE));

    if (hasPhysical(mech, LARM, PHY_SWORD))
      SHOW("Sword[LA]", physical_recycle_status(mech, LARM, PHY_SWORD));

    if (hasPhysical(mech, RARM, PHY_SWORD))
      SHOW("Sword[RA]", physical_recycle_status(mech, RARM, PHY_SWORD));

    if (hasPhysical(mech, LARM, PHY_CLAW))
      SHOW("Claw[LA]", physical_recycle_status(mech, LARM, PHY_CLAW));

    if (hasPhysical(mech, RARM, PHY_CLAW))
      SHOW("Claw[RA]", physical_recycle_status(mech, RARM, PHY_CLAW));

    if (hasPhysical(mech, LARM, PHY_MACE))
      SHOW("Mace[LA]", physical_recycle_status(mech, LARM, PHY_MACE));

    if (hasPhysical(mech, RARM, PHY_MACE))
      SHOW("Mace[RA]", physical_recycle_status(mech, RARM, PHY_MACE));

    if (hasPhysical(mech, LARM, PHY_SAW))
      SHOW("Saw[LA]", physical_recycle_status(mech, LARM, PHY_SAW));

    if (hasPhysical(mech, RARM, PHY_SAW))
      SHOW("Saw[RA]", physical_recycle_status(mech, RARM, PHY_SAW));

    notify(evaluation, player, tempbuff);

    if (conditions.arms_flipped)
      notify(evaluation, player,
             "*** Mech arms are flipped into the rear arc ***");
  } else if (mech_class(mech) == CLASS_BSUIT && !compact) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
      if (mech_section_internal(mech, i))
        break;
    if (i < NUM_BSUIT_MEMBERS) {
      snprintf(tempbuff, sizeof(tempbuff), "Team status (special attacks): %s",
               section_recycle_status(mech, i));
      notify(evaluation, player, tempbuff);
    }

  } else if (((mech_class(mech) == CLASS_VEH_GROUND) ||
              (mech_class(mech) == CLASS_VTOL)) &&
             !compact) {

    *tempbuff = 0;

    if (mech_section_recycle_ticks(mech, FSIDE)) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "Vehicle status (charge): %s",
               section_recycle_status(mech, FSIDE));
    }

    if (*tempbuff)
      notify(evaluation, player, tempbuff);
  }

  ammoweapcount = FindAmmunition(mech, ammoweap, ammo, ammomax, modearray, 0);
  if (!compact) {
    notify(evaluation, player,
           "==================WEAPON "
           "SYSTEMS===========================AMMUNITION========");
    if (mech_class(mech) == CLASS_BSUIT)
      notify(evaluation, player,
             "------ Weapon --------- [##] Holder ------ Status ||--- "
             "Ammo Type ---- Rounds");
    else
      notify(evaluation, player,
             "------ Weapon --------- [##] Location ---- Status ||--- "
             "Ammo Type ---- Rounds");
  }
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 0);
    if (count <= 0)
      continue;
    ArmorStringFromIndex(loop, tempbuff, mech_class(mech),
                         mech_movement_type(mech));
    snprintf(location, sizeof(location), "%-14.14s", tempbuff);
    if (compact) {
      strcpy(location, tempbuff);
      if ((tmpc = strchr(location, ' ')))
        *tmpc = '_';
    }
    for (ii = 0; ii < count; ii++) {
      int fire_mode = mech_critical_fire_mode(mech, loop, critical[ii]);
      if (MechWeapons[weaparray[ii]].special & AMS)
        snprintf(weapbuff, sizeof(weapbuff), " %-16.16s %c%c%c%c%c [%2d] ",
                 &MechWeapons[weaparray[ii]].name[3], ' ',
                 conditions.ams_enabled ? ' ' : 'O',
                 conditions.ams_enabled ? 'O' : 'F',
                 conditions.ams_enabled ? 'N' : 'F', ' ', running_sum + ii);
      else {
        if (fire_mode & OS_MODE)
          strcpy(tmpbuf, "OS ");
        else
          tmpbuf[0] = 0;
        strcat(tmpbuf, &MechWeapons[weaparray[ii]].name[3]);
        snprintf(
            weapbuff, sizeof(weapbuff), " %-16.16s %c%c%c%c%c [%2d] ", tmpbuf,
            (fire_mode & REAR_MOUNT) ? 'R' : ' ',
            (((fire_mode & OS_USED) || (fire_mode & ROCKET_FIRED)) ? '-'
             : (fire_mode & OS_MODE)                               ? 'O'
                                                                   : ' '),
            GetWeaponAmmoModeLetter(mech, loop, critical[ii]),
            GetWeaponFireModeLetter(mech, loop, critical[ii]),
            ((fire_mode & ON_TC) && !conditions.targeting_computer_destroyed)
                ? 'T'
            : (fire_mode & IS_JETTISONED_MODE) ? 'J'
            : (fire_mode & WILL_JETTISON_MODE) ? 'P'
                                               : ' ',
            running_sum + ii);
      }
      if (compact)
        append_status(compact_buffer, compact_buffer_size, "%s|%s",
                      &MechWeapons[weaparray[ii]].name[3], location);
      strcat(weapbuff, location);

      int temporary_failure =
          mech_critical_temporary_failure(mech, loop, critical[ii]);
      if (mech_critical_is_broken(mech, loop, critical[ii]) ||
          temporary_failure == FAIL_DESTROYED)
        strcat(weapbuff, "[fg=black bold]*****[reset]  || ");
      else if (mech_critical_is_disabled(mech, loop, critical[ii]))
        strcat(weapbuff, "[fg=red]DISABLE[reset]|| ");
      else if (temporary_failure) {
        switch (temporary_failure) {
        case FAIL_JAMMED:
          strcat(weapbuff, "[fg=red]JAMMED[reset] || ");
          break;
        case FAIL_SHORTED:
          strcat(weapbuff, "[fg=red]SHORTED[reset]|| ");
          break;
        case FAIL_EMPTY:
          strcat(weapbuff, " [fg=red]EMPTY[reset] || ");
          break;
        case FAIL_DUD:
          strcat(weapbuff, "[fg=red]DUD[reset]    || ");
          break;
        case FAIL_AMMOJAMMED:
          strcat(weapbuff, "[fg=red]AMMOJAM[reset]|| ");
          break;
        }
      } else if (fire_mode & ROCKET_FIRED)
        strcat(weapbuff, "[fg=black bold]Empty[reset]  || ");
      else if (weapdata[ii])
        strcat(weapbuff, tprintf(" %2d    || ",
                                 weapdata[ii] / WEAPON_TICK +
                                     (weapdata[ii] % WEAPON_TICK ? 1 : 0)));
      else if (mech_weapon_damaged_slot_count_at(mech, loop, critical[ii]))
        strcat(weapbuff, "[fg=red]DAMAGED[reset]|| ");
      else
        strcat(weapbuff, "[fg=green]Ready[reset]  || ");

      if ((ii + running_sum) < ammoweapcount) {
        ammo_mode = GetWeaponAmmoModeLetter_Model_Mode(
            ammoweap[ii + running_sum], modearray[ii + running_sum]);
        snprintf(weapname, sizeof(weapname), "%-16.16s %c",
                 &MechWeapons[ammoweap[ii + running_sum]].name[3], ammo_mode);
        snprintf(tempbuff, sizeof(tempbuff), "  %s%3d%s",
                 evaluate_ammo_amount(ammo[ii + running_sum],
                                      ammomax[ii + running_sum]),
                 ammo[ii + running_sum], "[reset]");
        strcat(weapname, tempbuff);
        if (compact) {
          if (ammo_mode && ammo_mode != ' ')
            append_status(compact_buffer, compact_buffer_size, "|%s|%d|%c ",
                          &MechWeapons[ammoweap[ii + running_sum]].name[3],
                          ammo[ii + running_sum], ammo_mode);
          else
            append_status(compact_buffer, compact_buffer_size, "|%s|%d ",
                          &MechWeapons[ammoweap[ii + running_sum]].name[3],
                          ammo[ii + running_sum]);
        }
      } else {
        if (compact)
          append_status(compact_buffer, compact_buffer_size, " ");
        snprintf(weapname, sizeof(weapname), "   ");
      }
      strcat(weapbuff, weapname);
      if (!compact)
        notify(evaluation, player, weapbuff);
    }
    running_sum += count;
  }

  if (running_sum < ammoweapcount) {
    while (running_sum < ammoweapcount) {
      strcpy(astrAmmoSpacer,
             "                                                  || ");
      ammo_mode = GetWeaponAmmoModeLetter_Model_Mode(ammoweap[running_sum],
                                                     modearray[running_sum]);
      snprintf(weapname, sizeof(weapname), "%-16.16s %c",
               &MechWeapons[ammoweap[running_sum]].name[3], ammo_mode);
      snprintf(tempbuff, sizeof(tempbuff), "  %s%3d%s",
               evaluate_ammo_amount(ammo[running_sum], ammomax[running_sum]),
               ammo[running_sum], "[reset]");
      strcat(astrAmmoSpacer, weapname);
      strcat(astrAmmoSpacer, tempbuff);

      notify(evaluation, player, astrAmmoSpacer);

      running_sum++;
    }
  }
}
