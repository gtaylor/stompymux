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

typedef struct WeaponStatusWorkspace {
  char message_buffer[LBUF_SIZE];
  char tmpbuf[LBUF_SIZE];
  char weapname[LBUF_SIZE];
  char weapbuff[LBUF_SIZE];
  char tempbuff[LBUF_SIZE];
  char ammo_spacer[MBUF_SIZE];
} WeaponStatusWorkspace;

typedef struct WeaponAdvancedStatusRequest {
  EvaluationContext *evaluation;
  Mech *mech;
  DbRef player;
  WeaponStatusWorkspace *workspace;
  int technology;
  int technology_secondary;
  int infantry_technology;
  MechConditionSummary conditions;
} WeaponAdvancedStatusRequest;

static void
weapon_advanced_status_notify(const WeaponAdvancedStatusRequest *request) {
  EvaluationContext *evaluation = request->evaluation;
  Mech *mech = request->mech;
  const DbRef PLAYER = request->player;
  WeaponStatusWorkspace *workspace = request->workspace;
  const int TECHNOLOGY = request->technology;
  const int TECHNOLOGY_SECONDARY = request->technology_secondary;
  const int INFANTRY_TECHNOLOGY = request->infantry_technology;
  const MechConditionSummary CONDITIONS = request->conditions;
  const bool HAS_C3_MASTER = (TECHNOLOGY & C3_MASTER_TECH) != 0;
  const bool HAS_C3_SLAVE = (TECHNOLOGY & C3_SLAVE_TECH) != 0;
  const bool HAS_C3I = (TECHNOLOGY_SECONDARY & C3I_TECH) != 0;
  const bool HAS_TAG =
      ((TECHNOLOGY_SECONDARY & TAG_TECH) || HAS_C3_MASTER) != 0;

  if (!((TECHNOLOGY & ECM_TECH) ||
        (TECHNOLOGY_SECONDARY & STEALTH_ARMOR_TECH) ||
        (TECHNOLOGY_SECONDARY & NULLSIGSYS_TECH) || (TECHNOLOGY & SLITE_TECH) ||
        HAS_C3_MASTER || HAS_C3_SLAVE || HAS_C3I || (TECHNOLOGY & MASC_TECH) ||
        (TECHNOLOGY_SECONDARY & SUPERCHARGER_TECH) ||
        (TECHNOLOGY & TRIPLE_MYOMER_TECH) ||
        (TECHNOLOGY_SECONDARY & ANGEL_ECM_TECH) || HAS_TAG ||
        (INFANTRY_TECHNOLOGY & FC_INFILTRATORII_STEALTH_TECH)))
    return;

  (void)string_copy_bounded(workspace->tempbuff, sizeof(workspace->tempbuff),
                            "AdvTech: ");
  if (TECHNOLOGY & ECM_TECH)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff), "ECM(%s)  ",
                  ecm_status(CONDITIONS.ecm_destroyed, CONDITIONS.ecm_enabled,
                             CONDITIONS.ecm_active, CONDITIONS.eccm_enabled,
                             CONDITIONS.ecm_countered));
  if (TECHNOLOGY_SECONDARY & ANGEL_ECM_TECH)
    append_status(
        workspace->tempbuff, sizeof(workspace->tempbuff), "AngelECM(%s)  ",
        ecm_status(CONDITIONS.angel_ecm_destroyed, CONDITIONS.angel_ecm_enabled,
                   CONDITIONS.angel_ecm_active, CONDITIONS.angel_eccm_enabled,
                   CONDITIONS.ecm_countered));
  if (INFANTRY_TECHNOLOGY & FC_INFILTRATORII_STEALTH_TECH)
    append_status(
        workspace->tempbuff, sizeof(workspace->tempbuff), "PersonalECM(%s)  ",
        ecm_status(false, CONDITIONS.personal_ecm_enabled,
                   CONDITIONS.personal_ecm_active,
                   CONDITIONS.personal_eccm_enabled, CONDITIONS.ecm_countered));
  if (TECHNOLOGY_SECONDARY & STEALTH_ARMOR_TECH) {
    const char *status = CONDITIONS.stealth_armor_active
                             ? "[fg=green bold]On[reset]"
                             : "[fg=green]Rdy[reset]";
    if (CONDITIONS.ecm_destroyed)
      status = "[fg=red bold]XX[reset]";
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "SthArmor(%s)  ", status);
  }
  if (TECHNOLOGY_SECONDARY & NULLSIGSYS_TECH) {
    const char *status = CONDITIONS.null_signature_active
                             ? "[fg=green bold]On[reset]"
                             : "[fg=green]Rdy[reset]";
    if (CONDITIONS.null_signature_destroyed)
      status = "[fg=red bold]XX[reset]";
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "NullSigSys(%s)  ", status);
  }
  if (TECHNOLOGY & SLITE_TECH) {
    const char *status = mech_searchlight_active(mech)
                             ? "[fg=green bold]On[reset]"
                             : "[fg=green]Off[reset]";
    if (CONDITIONS.searchlight_destroyed)
      status = "[fg=red bold]XX[reset]";
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "SLITE(%s)  ", status);
  }
  if (HAS_C3_MASTER)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "%sC3M[reset]  ",
                  network_status_color(CONDITIONS.c3_destroyed,
                                       mech_is_any_ecm_disturbed(mech),
                                       mech_c3_network_size(mech) > 0));
  if (HAS_C3_SLAVE)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "%sC3S[reset]  ",
                  network_status_color(CONDITIONS.c3_destroyed,
                                       mech_is_any_ecm_disturbed(mech),
                                       mech_c3_network_size(mech) > 0));
  if (HAS_C3I)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "%sC3i[reset]  ",
                  network_status_color(CONDITIONS.c3i_destroyed,
                                       mech_is_any_ecm_disturbed(mech),
                                       mech_c3i_network_size(mech) > 0));
  if (TECHNOLOGY & TRIPLE_MYOMER_TECH)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff), "TSM(%s)  ",
                  mech_excess_heat(mech) >= 9.0F ? "[fg=green bold]On[reset]"
                                                 : "[fg=green]Off[reset]");
  if (HAS_TAG) {
    const DbRef TAG_TARGET_DBREF = mech_tag_target_dbref(mech);
    Mech *tag_target =
        btech_context_get_mech(mech_context(mech), TAG_TARGET_DBREF);
    const bool RECYCLING = mech_event_count(mech, EVENT_TAG_RECYCLE) != 0;
    const char *status;
    if (mech_tag_is_destroyed(mech)) {
      status = "[fg=red bold]XX[reset]";
    } else if (tag_target == nullptr ||
               mech_tagged_by_dbref(tag_target) != mech_dbref(mech)) {
      status =
          RECYCLING ? "[fg=yellow bold]Not Rdy[reset]" : "[fg=green]Rdy[reset]";
    } else {
      (void)snprintf(workspace->message_buffer,
                     sizeof(workspace->message_buffer), "%s%s[reset]",
                     RECYCLING ? "[fg=yellow bold]" : "[bold]",
                     mech_to_mech_display_id(mech, tag_target).text);
      status = workspace->message_buffer;
    }
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff), "TAG(%s)  ",
                  status);
  }
  if (TECHNOLOGY_SECONDARY & SUPERCHARGER_TECH)
    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "SCHARGE: %s%d[reset] (%s)",
                  counter_status_color(CONDITIONS.supercharger_counter),
                  CONDITIONS.supercharger_counter,
                  CONDITIONS.supercharger_enabled ? "On" : "Off");
  if (TECHNOLOGY & MASC_TECH)
    append_status(
        workspace->tempbuff, sizeof(workspace->tempbuff),
        "MASC: %s%d[reset] (%s)", counter_status_color(CONDITIONS.masc_counter),
        CONDITIONS.masc_counter, CONDITIONS.masc_enabled ? "On" : "Off");
  mecha_notify(evaluation, PLAYER, workspace->tempbuff);
  workspace->tempbuff[0] = 0;
}

void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size) {
  WeaponStatusWorkspace *workspace =
      checked_storage_allocate(sizeof(*workspace));
  unsigned char weaparray[MAX_WEAPS_SECTION] = {0};
  unsigned char weapdata[MAX_WEAPS_SECTION] = {0};
  int critical[MAX_WEAPS_SECTION] = {0};
  unsigned char ammoweap[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammo[8 * MAX_WEAPS_SECTION] = {0};
  unsigned short ammomax[8 * MAX_WEAPS_SECTION] = {0};
  unsigned int modearray[8 * MAX_WEAPS_SECTION] = {0};
  int count;
  int ammoweapcount;
  int loop;
  int ii;
  int i = 0;
  char *tmpc;
  char location[80] = {0};
  int running_sum = 0;
  char ammo_mode;
  int technology = mech_technology_flags(mech);
  int technology_secondary = mech_technology_flags_secondary(mech);
  int infantry_technology = mech_infantry_technology_flags(mech);
  MechConditionSummary conditions = mech_condition_summary(mech);
  weapon_advanced_status_notify(&(WeaponAdvancedStatusRequest){
      .evaluation = evaluation,
      .mech = mech,
      .player = player,
      .workspace = workspace,
      .technology = technology,
      .technology_secondary = technology_secondary,
      .infantry_technology = infantry_technology,
      .conditions = conditions,
  });

  if (technology_secondary & CARRIER_TECH) {
    (void)string_copy_bounded(workspace->tempbuff, sizeof(workspace->tempbuff),
                              "Carrier: ");

    append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                  "%d tons free, %d tons max unit size",
                  mech_cargo_space(mech) / 100,
                  mech_carrier_maximum_tonnage(mech));
    mecha_notify(evaluation, player, workspace->tempbuff);
    workspace->tempbuff[0] = 0;
  }

  if ((technology & AA_TECH) || (technology & BEAGLE_PROBE_TECH) ||
      (technology_secondary & BLOODHOUND_PROBE_TECH)) {
    (void)string_copy_bounded(workspace->tempbuff, sizeof(workspace->tempbuff),
                              "AdvSensors:");

    if (technology & AA_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff), " Radar");

    if (technology & BEAGLE_PROBE_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " BeagleProbe");

    if (technology_secondary & BLOODHOUND_PROBE_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " BloodhoundProbe");

    mecha_notify(evaluation, player, workspace->tempbuff);
    workspace->tempbuff[0] = 0;
  }

  if ((infantry_technology & CS_PURIFIER_STEALTH_TECH) ||
      (infantry_technology & DC_KAGE_STEALTH_TECH) ||
      (infantry_technology & FWL_ACHILEUS_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATOR_STEALTH_TECH) ||
      (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)) {

    (void)string_copy_bounded(workspace->tempbuff, sizeof(workspace->tempbuff),
                              "AdvItems:");

    if (infantry_technology & CS_PURIFIER_STEALTH_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " PurifierStealth");

    if (infantry_technology & DC_KAGE_STEALTH_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " KageStealth");

    if (infantry_technology & FWL_ACHILEUS_STEALTH_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " AchileusStealth");

    if (infantry_technology & FC_INFILTRATOR_STEALTH_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " InfiltratorStealth");

    if (infantry_technology & FC_INFILTRATORII_STEALTH_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " InfiltratorIIStealth");

    mecha_notify(evaluation, player, workspace->tempbuff);
    workspace->tempbuff[0] = 0;
  }

  if ((infantry_technology & INF_SWARM_TECH) ||
      (infantry_technology & INF_MOUNT_TECH) ||
      (infantry_technology & INF_ANTILEG_TECH) ||
      (infantry_technology & CAN_JETTISON_TECH)) {

    (void)string_copy_bounded(workspace->tempbuff, sizeof(workspace->tempbuff),
                              "Special Actions:");

    if (infantry_technology & INF_MOUNT_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " MountFriends");

    if (infantry_technology & INF_SWARM_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " SwarmAttack");

    if (infantry_technology & INF_ANTILEG_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " AntiLegAttack");

    if (infantry_technology & CAN_JETTISON_TECH)
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    " BackPackJettison");

    mecha_notify(evaluation, player, workspace->tempbuff);
    workspace->tempbuff[0] = 0;
  }

  if (infantry_technology & MUST_JETTISON_TECH) {
    (void)string_copy_bounded(
        workspace->tempbuff, sizeof(workspace->tempbuff),
        "Requirements: Must jettison backpack before using "
        "special abilities or jumping");
    mecha_notify(evaluation, player, workspace->tempbuff);
    workspace->tempbuff[0] = 0;
  }
  mech_update_recycling(mech);
  if (mech_class(mech) == CLASS_MECH && !compact) {
    workspace->tempbuff[0] = 0;

    bool is_quad = mech_movement_type(mech) == MOVE_QUAD;
    recycle_status_append(workspace->tempbuff, sizeof(workspace->tempbuff),
                          is_quad ? "FLLEG" : "LARM",
                          section_recycle_status(mech, LARM, (char[32]){0}));
    recycle_status_append(workspace->tempbuff, sizeof(workspace->tempbuff),
                          is_quad ? "FRLEG" : "RARM",
                          section_recycle_status(mech, RARM, (char[32]){0}));
    recycle_status_append(workspace->tempbuff, sizeof(workspace->tempbuff),
                          is_quad ? "RLLEG" : "LLEG",
                          section_recycle_status(mech, LLEG, (char[32]){0}));
    recycle_status_append(workspace->tempbuff, sizeof(workspace->tempbuff),
                          is_quad ? "RRLEG" : "RLEG",
                          section_recycle_status(mech, RLEG, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_AXE}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Axe[LA]",
          physical_recycle_status(mech, LARM, PHY_AXE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_AXE}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Axe[RA]",
          physical_recycle_status(mech, RARM, PHY_AXE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_SWORD}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Sword[LA]",
          physical_recycle_status(mech, LARM, PHY_SWORD, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_SWORD}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Sword[RA]",
          physical_recycle_status(mech, RARM, PHY_SWORD, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_CLAW}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Claw[LA]",
          physical_recycle_status(mech, LARM, PHY_CLAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_CLAW}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Claw[RA]",
          physical_recycle_status(mech, RARM, PHY_CLAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_MACE}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Mace[LA]",
          physical_recycle_status(mech, LARM, PHY_MACE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_MACE}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Mace[RA]",
          physical_recycle_status(mech, RARM, PHY_MACE, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = LARM, .type = PHY_SAW}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Saw[LA]",
          physical_recycle_status(mech, LARM, PHY_SAW, (char[32]){0}));

    if (has_physical(&(PhysicalWeaponRequest){
            .mech = mech, .section = RARM, .type = PHY_SAW}))
      recycle_status_append(
          workspace->tempbuff, sizeof(workspace->tempbuff), "Saw[RA]",
          physical_recycle_status(mech, RARM, PHY_SAW, (char[32]){0}));

    mecha_notify(evaluation, player, workspace->tempbuff);

    if (conditions.arms_flipped)
      mecha_notify(evaluation, player,
                   "*** Mech arms are flipped into the rear arc ***");
  } else if (mech_class(mech) == CLASS_BSUIT && !compact) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
      if (mech_section_internal(mech, i))
        break;
    if (i < NUM_BSUIT_MEMBERS) {
      (void)snprintf(workspace->tempbuff, sizeof(workspace->tempbuff),
                     "Team status (special attacks): %s",
                     section_recycle_status(mech, i, (char[32]){0}));
      mecha_notify(evaluation, player, workspace->tempbuff);
    }

  } else if (((mech_class(mech) == CLASS_VEH_GROUND) ||
              (mech_class(mech) == CLASS_VTOL)) &&
             !compact) {

    *workspace->tempbuff = 0;

    if (mech_section_recycle_ticks(mech, FSIDE)) {
      append_status(workspace->tempbuff, sizeof(workspace->tempbuff),
                    "Vehicle status (charge): %s",
                    section_recycle_status(mech, FSIDE, (char[32]){0}));
    }

    if (*workspace->tempbuff)
      mecha_notify(evaluation, player, workspace->tempbuff);
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
    armor_string_from_index(loop, workspace->tempbuff, mech_class(mech),
                            mech_movement_type(mech));
    (void)snprintf(location, sizeof(location), "%-14.14s", workspace->tempbuff);
    if (compact) {
      (void)string_copy_bounded(location, sizeof(location),
                                workspace->tempbuff);
      tmpc = strchr(location, ' ');
      if (tmpc)
        *tmpc = '_';
    }
    for (ii = 0; ii < count; ii++) {
      BtechTextBuilder weapon_text;
      btech_text_builder_initialize(&weapon_text, workspace->weapbuff,
                                    sizeof(workspace->weapbuff));
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
        (void)snprintf(workspace->tmpbuf, sizeof(workspace->tmpbuf), "%s%s",
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
            &weapon_text, " %-16.16s %c%c%c%c%c [%2d] ", workspace->tmpbuf,
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
        (void)snprintf(workspace->weapname, sizeof(workspace->weapname),
                       "%-16.16s %c  %s%3d%s", ammunition_name, ammo_mode,
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
        (void)snprintf(workspace->weapname, sizeof(workspace->weapname), "   ");
      }
      btech_text_builder_append(&weapon_text, workspace->weapname);
      if (!compact)
        mecha_notify(evaluation, player, workspace->weapbuff);
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
          workspace->ammo_spacer, sizeof(workspace->ammo_spacer),
          "                                                  || "
          "%-16.16s %c  %s%3d%s",
          checked_string_suffix(weapon_catalogue_name(AMMUNITION_WEAPON), 3),
          ammo_mode, evaluate_ammo_amount(AMMUNITION, MAXIMUM_AMMUNITION),
          AMMUNITION, "[reset]");

      mecha_notify(evaluation, player, workspace->ammo_spacer);

      running_sum++;
    }
  }
  free(workspace);
}
