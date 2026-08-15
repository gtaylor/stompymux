#include "btconfig.h"
#include "equipment_types.h"
#include "mech_lifecycle.h"
#include "mech_status_api.h"
#include "mech_status_render_internal.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btech_text_builder.h"
#include "failures.h"
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
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

static void recycle_status_append(char *buffer, size_t capacity,
                                  const char *part, const char *status) {
  size_t length = strlen(buffer);
  if (length >= capacity)
    return;
  (void)snprintf(
      checked_storage_region(buffer, capacity, length, capacity - length),
      capacity - length, "%s: %s ", part, status);
}
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

enum { AMMO_STATUS_CAPACITY = 8 * MAX_WEAPS_SECTION };

static unsigned char weapon_status_byte(const unsigned char *values,
                                        size_t capacity, int index) {
  if (index < 0)
    abort();
  return *(const unsigned char *)checked_storage_at_const(
      values, capacity, sizeof(*values), (size_t)index);
}

static unsigned short weapon_status_short(const unsigned short *values,
                                          int index) {
  if (index < 0)
    abort();
  return *(const unsigned short *)checked_storage_at_const(
      values, AMMO_STATUS_CAPACITY, sizeof(*values), (size_t)index);
}

static unsigned int weapon_status_mode(const unsigned int *values, int index) {
  if (index < 0)
    abort();
  return *(const unsigned int *)checked_storage_at_const(
      values, AMMO_STATUS_CAPACITY, sizeof(*values), (size_t)index);
}

static int weapon_status_critical(const int *values, int index) {
  if (index < 0)
    abort();
  return *(const int *)checked_storage_at_const(values, MAX_WEAPS_SECTION,
                                                sizeof(*values), (size_t)index);
}

static char *section_recycle_status(Mech *mech, int section,
                                    char buffer[static 32]) {
  int recycle = mech_section_recycle_ticks(mech, section);
  if (mech_section_is_destroyed(mech, section)) {
    (void)snprintf(buffer, 32, "%s", "[fg=black bold]*****[reset]");
  } else if (recycle > 0) {
    (void)snprintf(buffer, 32, "%-5d",
                   (recycle / WEAPON_TICK) + (recycle % WEAPON_TICK));
  } else {
    (void)snprintf(buffer, 32, "%s", "[fg=green]Ready[reset]");
  }
  return buffer;
}

static const char *ecm_status(bool destroyed, bool enabled, bool active,
                              bool eccm_enabled, bool countered) {
  if (destroyed)
    return "[fg=red bold]XX[reset]";
  if (enabled)
    return active ? "[fg=green bold]ECM[reset]" : "[fg=red bold]ECM[reset]";
  if (eccm_enabled)
    return "[fg=green bold]ECCM[reset]";
  return countered ? "[fg=red]Off[reset]" : "[fg=green]Off[reset]";
}

static const char *network_status_color(bool destroyed, bool disturbed,
                                        bool connected) {
  if (destroyed)
    return "[fg=red]";
  if (disturbed)
    return "[fg=yellow]";
  return connected ? "[fg=green bold]" : "[fg=green]";
}

static const char *counter_status_color(int counter) {
  if (counter > 3)
    return "[fg=red bold]";
  return counter > 0 ? "[fg=yellow bold]" : "[fg=green]";
}

static char *physical_recycle_status(Mech *mech, int section,
                                     MechPhysicalWeaponType physical_type,
                                     char buffer[static 32]) {
  int recycle = mech_section_recycle_ticks(mech, section);
  if (!can_use_physical(&(PhysicalWeaponRequest){
          .mech = mech, .section = section, .type = physical_type})) {
    (void)snprintf(buffer, 32, "%s", "[fg=red bold]XX[reset]");
  } else if (recycle > 0) {
    (void)snprintf(buffer, 32, "%-3d",
                   (recycle / WEAPON_TICK) + (recycle % WEAPON_TICK));
  } else {
    (void)snprintf(buffer, 32, "%s", "[fg=green]Rdy[reset]");
  }
  return buffer;
}

void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size) {
  char message_buffer[LBUF_SIZE];
  unsigned char weaparray[MAX_WEAPS_SECTION] = {0};
  unsigned char weapdata[MAX_WEAPS_SECTION] = {0};
  int critical[MAX_WEAPS_SECTION] = {0};
  unsigned char ammoweap[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammo[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammomax[8 * MAX_WEAPS_SECTION] = {0};
  unsigned int modearray[8 * MAX_WEAPS_SECTION] = {0};
  char tmpbuf[LBUF_SIZE] = {0};
  int count;
  int ammoweapcount;
  int loop;
  int ii;
  int i = 0;
  char weapname[LBUF_SIZE] = {0};
  char *tmpc;
  char weapbuff[LBUF_SIZE] = {0};
  char tempbuff[LBUF_SIZE] = {0};
  char location[80] = {0};
  char astr_ammo_spacer[MBUF_SIZE] = {0}; /* mem is cheap. over allocate */
  int running_sum = 0;
  char ammo_mode;
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
    (void)string_copy_bounded(tempbuff, sizeof(tempbuff), "AdvTech: ");

    if (technology & ECM_TECH) {
      append_status(tempbuff, sizeof(tempbuff), "ECM(%s)  ",
                    ecm_status(conditions.ecm_destroyed, conditions.ecm_enabled,
                               conditions.ecm_active, conditions.eccm_enabled,
                               conditions.ecm_countered));
    }

    if (technology_secondary & ANGEL_ECM_TECH) {
      append_status(
          tempbuff, sizeof(tempbuff), "AngelECM(%s)  ",
          ecm_status(conditions.angel_ecm_destroyed,
                     conditions.angel_ecm_enabled, conditions.angel_ecm_active,
                     conditions.angel_eccm_enabled, conditions.ecm_countered));
    }

    if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH) {
      append_status(tempbuff, sizeof(tempbuff), "PersonalECM(%s)  ",
                    ecm_status(false, conditions.personal_ecm_enabled,
                               conditions.personal_ecm_active,
                               conditions.personal_eccm_enabled,
                               conditions.ecm_countered));
    }

    if (technology_secondary & STEALTH_ARMOR_TECH) {
      const char *status = conditions.stealth_armor_active
                               ? "[fg=green bold]On[reset]"
                               : "[fg=green]Rdy[reset]";
      if (conditions.ecm_destroyed)
        status = "[fg=red bold]XX[reset]";
      append_status(tempbuff, sizeof(tempbuff), "SthArmor(%s)  ", status);
    }

    if (technology_secondary & NULLSIGSYS_TECH) {
      const char *status = conditions.null_signature_active
                               ? "[fg=green bold]On[reset]"
                               : "[fg=green]Rdy[reset]";
      if (conditions.null_signature_destroyed)
        status = "[fg=red bold]XX[reset]";
      append_status(tempbuff, sizeof(tempbuff), "NullSigSys(%s)  ", status);
    }

    if (technology & SLITE_TECH) {
      const char *status = mech_searchlight_active(mech)
                               ? "[fg=green bold]On[reset]"
                               : "[fg=green]Off[reset]";
      if (conditions.searchlight_destroyed)
        status = "[fg=red bold]XX[reset]";
      append_status(tempbuff, sizeof(tempbuff), "SLITE(%s)  ", status);
    }

    if (has_c3_master) {
      append_status(tempbuff, sizeof(tempbuff), "%sC3M[reset]  ",
                    network_status_color(conditions.c3_destroyed,
                                         mech_is_any_ecm_disturbed(mech),
                                         mech_c3_network_size(mech) > 0));
    }

    if (has_c3_slave) {
      append_status(tempbuff, sizeof(tempbuff), "%sC3S[reset]  ",
                    network_status_color(conditions.c3_destroyed,
                                         mech_is_any_ecm_disturbed(mech),
                                         mech_c3_network_size(mech) > 0));
    }

    if (has_c3i) {
      append_status(tempbuff, sizeof(tempbuff), "%sC3i[reset]  ",
                    network_status_color(conditions.c3i_destroyed,
                                         mech_is_any_ecm_disturbed(mech),
                                         mech_c3i_network_size(mech) > 0));
    }

    if (technology & TRIPLE_MYOMER_TECH)
      append_status(tempbuff, sizeof(tempbuff), "TSM(%s)  ",
                    mech_excess_heat(mech) >= 9.0F ? "[fg=green bold]On[reset]"
                                                   : "[fg=green]Off[reset]");

    if (has_tag) {
      DbRef tag_target_dbref = mech_tag_target_dbref(mech);
      Mech *tag_target =
          btech_context_get_mech(mech_context(mech), tag_target_dbref);
      const bool RECYCLING = mech_event_count(mech, EVENT_TAG_RECYCLE) != 0;
      const char *status;
      if (mech_tag_is_destroyed(mech)) {
        status = "[fg=red bold]XX[reset]";
      } else if (!tag_target ||
                 mech_tagged_by_dbref(tag_target) != mech_dbref(mech)) {
        status = RECYCLING ? "[fg=yellow bold]Not Rdy[reset]"
                           : "[fg=green]Rdy[reset]";
      } else {
        (void)snprintf(message_buffer, sizeof(message_buffer), "%s%s[reset]",
                       RECYCLING ? "[fg=yellow bold]" : "[bold]",
                       mech_to_mech_display_id(mech, tag_target).text);
        status = message_buffer;
      }
      append_status(tempbuff, sizeof(tempbuff), "TAG(%s)  ", status);
    }

    if (technology_secondary & SUPERCHARGER_TECH) {
      append_status(tempbuff, sizeof(tempbuff), "SCHARGE: %s%d[reset] (%s)",
                    counter_status_color(conditions.supercharger_counter),
                    conditions.supercharger_counter,
                    conditions.supercharger_enabled ? "On" : "Off");
    }

    if (technology & MASC_TECH) {
      append_status(tempbuff, sizeof(tempbuff), "MASC: %s%d[reset] (%s)",
                    counter_status_color(conditions.masc_counter),
                    conditions.masc_counter,
                    conditions.masc_enabled ? "On" : "Off");
    }

    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (technology_secondary & CARRIER_TECH) {
    (void)string_copy_bounded(tempbuff, sizeof(tempbuff), "Carrier: ");

    append_status(
        tempbuff, sizeof(tempbuff), "%d tons free, %d tons max unit size",
        mech_cargo_space(mech) / 100, mech_carrier_maximum_tonnage(mech));
    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((technology & AA_TECH) || (technology & BEAGLE_PROBE_TECH) ||
      (technology_secondary & BLOODHOUND_PROBE_TECH)) {
    (void)string_copy_bounded(tempbuff, sizeof(tempbuff), "AdvSensors:");

    if (technology & AA_TECH)
      append_status(tempbuff, sizeof(tempbuff), " Radar");

    if (technology & BEAGLE_PROBE_TECH)
      append_status(tempbuff, sizeof(tempbuff), " BeagleProbe");

    if (technology_secondary & BLOODHOUND_PROBE_TECH)
      append_status(tempbuff, sizeof(tempbuff), " BloodhoundProbe");

    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((infantry_technology & CS_PURIFIER_STEALTH_TECH) ||
      (infantry_technology & DC_KAGE_STEALTH_TECH) ||
      (infantry_technology & FWL_ACHILEUS_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATOR_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)) {

    (void)string_copy_bounded(tempbuff, sizeof(tempbuff), "AdvItems:");

    if (infantry_technology & CS_PURIFIER_STEALTH_TECH)
      append_status(tempbuff, sizeof(tempbuff), " PurifierStealth");

    if (infantry_technology & DC_KAGE_STEALTH_TECH)
      append_status(tempbuff, sizeof(tempbuff), " KageStealth");

    if (infantry_technology & FWL_ACHILEUS_STEALTH_TECH)
      append_status(tempbuff, sizeof(tempbuff), " AchileusStealth");

    if (infantry_technology & FC_INFILTRATOR_STEALTH_TECH)
      append_status(tempbuff, sizeof(tempbuff), " InfiltratorStealth");

    if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)
      append_status(tempbuff, sizeof(tempbuff), " InfiltratorIIStealth");

    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((infantry_technology & INF_SWARM_TECH) ||
      (infantry_technology & INF_MOUNT_TECH) ||
      (infantry_technology & INF_ANTILEG_TECH) ||
      (infantry_technology & CAN_JETTISON_TECH)) {

    (void)string_copy_bounded(tempbuff, sizeof(tempbuff), "Special Actions:");

    if (infantry_technology & INF_MOUNT_TECH)
      append_status(tempbuff, sizeof(tempbuff), " MountFriends");

    if (infantry_technology & INF_SWARM_TECH)
      append_status(tempbuff, sizeof(tempbuff), " SwarmAttack");

    if (infantry_technology & INF_ANTILEG_TECH)
      append_status(tempbuff, sizeof(tempbuff), " AntiLegAttack");

    if (infantry_technology & CAN_JETTISON_TECH)
      append_status(tempbuff, sizeof(tempbuff), " BackPackJettison");

    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (infantry_technology & MUST_JETTISON_TECH) {
    (void)string_copy_bounded(
        tempbuff, sizeof(tempbuff),
        "Requirements: Must jettison backpack before using "
        "special abilities or jumping");
    mecha_notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }
  mech_update_recycling(mech);
  if (mech_class(mech) == CLASS_MECH && !compact) {
    tempbuff[0] = 0;

    bool is_quad = mech_movement_type(mech) == MOVE_QUAD;
    recycle_status_append(tempbuff, sizeof(tempbuff),
                          is_quad ? "FLLEG" : "LARM",
                          section_recycle_status(mech, LARM, (char[32]){0}));
    recycle_status_append(tempbuff, sizeof(tempbuff),
                          is_quad ? "FRLEG" : "RARM",
                          section_recycle_status(mech, RARM, (char[32]){0}));
    recycle_status_append(tempbuff, sizeof(tempbuff),
                          is_quad ? "RLLEG" : "LLEG",
                          section_recycle_status(mech, LLEG, (char[32]){0}));
    recycle_status_append(tempbuff, sizeof(tempbuff),
                          is_quad ? "RRLEG" : "RLEG",
                          section_recycle_status(mech, RLEG, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_AXE}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Axe[LA]",
          physical_recycle_status(mech, LARM, PHY_AXE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_AXE}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Axe[RA]",
          physical_recycle_status(mech, RARM, PHY_AXE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_SWORD}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Sword[LA]",
          physical_recycle_status(mech, LARM, PHY_SWORD, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_SWORD}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Sword[RA]",
          physical_recycle_status(mech, RARM, PHY_SWORD, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_CLAW}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Claw[LA]",
          physical_recycle_status(mech, LARM, PHY_CLAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_CLAW}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Claw[RA]",
          physical_recycle_status(mech, RARM, PHY_CLAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_MACE}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Mace[LA]",
          physical_recycle_status(mech, LARM, PHY_MACE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_MACE}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Mace[RA]",
          physical_recycle_status(mech, RARM, PHY_MACE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_SAW}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Saw[LA]",
          physical_recycle_status(mech, LARM, PHY_SAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_SAW}))
      recycle_status_append(
          tempbuff, sizeof(tempbuff), "Saw[RA]",
          physical_recycle_status(mech, RARM, PHY_SAW, (char[32]){0}));

    mecha_notify(evaluation, player, tempbuff);

    if (conditions.arms_flipped)
      mecha_notify(evaluation, player,
                   "*** Mech arms are flipped into the rear arc ***");
  } else if (mech_class(mech) == CLASS_BSUIT && !compact) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
      if (mech_section_internal(mech, i))
        break;
    if (i < NUM_BSUIT_MEMBERS) {
      (void)snprintf(tempbuff, sizeof(tempbuff),
                     "Team status (special attacks): %s",
                     section_recycle_status(mech, i, (char[32]){0}));
      mecha_notify(evaluation, player, tempbuff);
    }

  } else if (((mech_class(mech) == CLASS_VEH_GROUND) ||
              (mech_class(mech) == CLASS_VTOL)) &&
             !compact) {

    *tempbuff = 0;

    if (mech_section_recycle_ticks(mech, FSIDE)) {
      append_status(tempbuff, sizeof(tempbuff), "Vehicle status (charge): %s",
                    section_recycle_status(mech, FSIDE, (char[32]){0}));
    }

    if (*tempbuff)
      mecha_notify(evaluation, player, tempbuff);
  }

  ammoweapcount = find_ammunition(mech, ammoweap, ammo, ammomax, modearray, 0);
  if (!compact) {
    mecha_notify(evaluation, player,
                 "==================WEAPON "
                 "SYSTEMS===========================AMMUNITION========");
    if (mech_class(mech) == CLASS_BSUIT)
      mecha_notify(evaluation, player,
                   "------ Weapon --------- [##] Holder ------ Status ||--- "
                   "Ammo Type ---- Rounds");
    else
      mecha_notify(evaluation, player,
                   "------ Weapon --------- [##] Location ---- Status ||--- "
                   "Ammo Type ---- Rounds");
  }
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    count = find_weapons_advanced(mech, loop, weaparray, weapdata, critical, 0);
    if (count <= 0)
      continue;
    armor_string_from_index(loop, tempbuff, mech_class(mech),
                            mech_movement_type(mech));
    (void)snprintf(location, sizeof(location), "%-14.14s", tempbuff);
    if (compact) {
      strlcpy(location, tempbuff, sizeof(location));
      tmpc = strchr(location, ' ');
      if (tmpc)
        *tmpc = '_';
    }
    for (ii = 0; ii < count; ii++) {
      BtechTextBuilder weapon_text;
      btech_text_builder_initialize(&weapon_text, weapbuff, sizeof(weapbuff));
      const int CRITICAL_INDEX = weapon_status_critical(critical, ii);
      const int WEAPON_INDEX =
          weapon_status_byte(weaparray, MAX_WEAPS_SECTION, ii);
      const char *weapon_name =
          checked_string_suffix(weapon_catalogue_name(WEAPON_INDEX), 3);
      int fire_mode = mech_critical_fire_mode(mech, loop, CRITICAL_INDEX);
      if (weapon_catalogue_has_special(WEAPON_INDEX, AMS)) {
        btech_text_builder_append_format(
            &weapon_text, " %-16.16s %c%c%c%c%c [%2d] ", weapon_name, ' ',
            conditions.ams_enabled ? ' ' : 'O',
            conditions.ams_enabled ? 'O' : 'F',
            conditions.ams_enabled ? 'N' : 'F', ' ', running_sum + ii);
      } else {
        (void)snprintf(tmpbuf, sizeof(tmpbuf), "%s%s",
                       fire_mode & OS_MODE ? "OS " : "", weapon_name);
        char one_shot_status = ' ';
        if ((fire_mode & OS_USED) || (fire_mode & ROCKET_FIRED))
          one_shot_status = '-';
        else if (fire_mode & OS_MODE)
          one_shot_status = 'O';
        char targeting_status = ' ';
        if ((fire_mode & ON_TC) && !conditions.targeting_computer_destroyed)
          targeting_status = 'T';
        else if (fire_mode & IS_JETTISONED_MODE)
          targeting_status = 'J';
        else if (fire_mode & WILL_JETTISON_MODE)
          targeting_status = 'P';
        btech_text_builder_append_format(
            &weapon_text, " %-16.16s %c%c%c%c%c [%2d] ", tmpbuf,
            (fire_mode & REAR_MOUNT) ? 'R' : ' ', one_shot_status,
            get_weapon_ammo_mode_letter(mech, loop, CRITICAL_INDEX),
            get_weapon_fire_mode_letter(mech, loop, CRITICAL_INDEX),
            targeting_status, running_sum + ii);
      }
      if (compact)
        append_status(compact_buffer, compact_buffer_size, "%s|%s", weapon_name,
                      location);
      btech_text_builder_append(&weapon_text, location);

      int temporary_failure =
          mech_critical_temporary_failure(mech, loop, CRITICAL_INDEX);
      if (mech_critical_is_broken(mech, loop, CRITICAL_INDEX) ||
          temporary_failure == FAIL_DESTROYED) {
        btech_text_builder_append(&weapon_text,
                                  "[fg=black bold]*****[reset]  || ");
      } else if (mech_critical_is_disabled(mech, loop, CRITICAL_INDEX)) {
        btech_text_builder_append(&weapon_text, "[fg=red]DISABLE[reset]|| ");
      } else if (temporary_failure) {
        switch (temporary_failure) {
        case FAIL_JAMMED:
          btech_text_builder_append(&weapon_text, "[fg=red]JAMMED[reset] || ");
          break;
        case FAIL_SHORTED:
          btech_text_builder_append(&weapon_text, "[fg=red]SHORTED[reset]|| ");
          break;
        case FAIL_EMPTY:
          btech_text_builder_append(&weapon_text, " [fg=red]EMPTY[reset] || ");
          break;
        case FAIL_DUD:
          btech_text_builder_append(&weapon_text, "[fg=red]DUD[reset]    || ");
          break;
        case FAIL_AMMOJAMMED:
          btech_text_builder_append(&weapon_text, "[fg=red]AMMOJAM[reset]|| ");
          break;
        }
      } else if (fire_mode & ROCKET_FIRED) {
        btech_text_builder_append(&weapon_text,
                                  "[fg=black bold]Empty[reset]  || ");
      } else if (weapon_status_byte(weapdata, MAX_WEAPS_SECTION, ii)) {
        const int RECYCLE = weapon_status_byte(weapdata, MAX_WEAPS_SECTION, ii);
        btech_text_builder_append_format(&weapon_text, " %2d    || ",
                                         (RECYCLE / WEAPON_TICK) +
                                             (RECYCLE % WEAPON_TICK ? 1 : 0));
      } else if (mech_weapon_damaged_slot_count_at(mech, loop,
                                                   CRITICAL_INDEX)) {
        btech_text_builder_append(&weapon_text, "[fg=red]DAMAGED[reset]|| ");
      } else {
        btech_text_builder_append(&weapon_text, "[fg=green]Ready[reset]  || ");
      }

      if ((ii + running_sum) < ammoweapcount) {
        const int AMMUNITION_INDEX = ii + running_sum;
        const int AMMUNITION_WEAPON = weapon_status_byte(
            ammoweap, AMMO_STATUS_CAPACITY, AMMUNITION_INDEX);
        const int AMMUNITION = weapon_status_short(ammo, AMMUNITION_INDEX);
        const int MAXIMUM_AMMUNITION =
            weapon_status_short(ammomax, AMMUNITION_INDEX);
        const char *ammunition_name =
            checked_string_suffix(weapon_catalogue_name(AMMUNITION_WEAPON), 3);
        ammo_mode = get_weapon_ammo_mode_letter_model_mode(
            AMMUNITION_WEAPON, weapon_status_mode(modearray, AMMUNITION_INDEX));
        (void)snprintf(weapname, sizeof(weapname), "%-16.16s %c  %s%3d%s",
                       ammunition_name, ammo_mode,
                       evaluate_ammo_amount(AMMUNITION, MAXIMUM_AMMUNITION),
                       AMMUNITION, "[reset]");
        if (compact) {
          if (ammo_mode && ammo_mode != ' ')
            append_status(compact_buffer, compact_buffer_size, "|%s|%d|%c ",
                          ammunition_name, AMMUNITION, ammo_mode);
          else
            append_status(compact_buffer, compact_buffer_size, "|%s|%d ",
                          ammunition_name, AMMUNITION);
        }
      } else {
        if (compact)
          append_status(compact_buffer, compact_buffer_size, " ");
        (void)snprintf(weapname, sizeof(weapname), "   ");
      }
      btech_text_builder_append(&weapon_text, weapname);
      if (!compact)
        mecha_notify(evaluation, player, weapbuff);
    }
    running_sum += count;
  }

  if (running_sum < ammoweapcount) {
    while (running_sum < ammoweapcount) {
      const int AMMUNITION_WEAPON =
          weapon_status_byte(ammoweap, AMMO_STATUS_CAPACITY, running_sum);
      const int AMMUNITION = weapon_status_short(ammo, running_sum);
      const int MAXIMUM_AMMUNITION = weapon_status_short(ammomax, running_sum);
      ammo_mode = get_weapon_ammo_mode_letter_model_mode(
          AMMUNITION_WEAPON, weapon_status_mode(modearray, running_sum));
      (void)snprintf(
          astr_ammo_spacer, sizeof(astr_ammo_spacer),
          "                                                  || "
          "%-16.16s %c  %s%3d%s",
          checked_string_suffix(weapon_catalogue_name(AMMUNITION_WEAPON), 3),
          ammo_mode, evaluate_ammo_amount(AMMUNITION, MAXIMUM_AMMUNITION),
          AMMUNITION, "[reset]");

      mecha_notify(evaluation, player, astr_ammo_spacer);

      running_sum++;
    }
  }
}
