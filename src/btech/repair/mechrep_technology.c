/* Implements BattleTech repair mechanics for mechrep technology. */

#include <limits.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h"
#include "equipment_types.h"
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_lifecycle.h"
#include "mech_template_api.h" // IWYU pragma: keep
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "repair_job.h"

#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "section_types.h"
#include "template_api.h"

/* Selectors */

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

void mechrep_raddspecial(DbRef player, void *data, char *buffer) {
  char *args[4];
  char location[20];
  int argc;
  int index;
  int itemcode;
  int subsect;
  int newdata;
  int max;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  argc = mech_parseattributes(buffer, args, 4);
  if (argc <= 2) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  itemcode = find_special_item_code_from_string(rep->xcode.context, args[0]);

  if (itemcode == -1)
    if (strcasecmp(args[0], "empty")) {
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "That is not a valid special object!");
      dump_mech_special_objects(rep->xcode.context, player);
      return;
    }
  index = armor_section_from_string(mech_class(mech), mech_movement_type(mech),
                                    args[1]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }
  if (!parse_int_checked(args[2], &subsect)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Critslot out of range!");
    return;
  }
  subsect--;
  max = crits_in_loc(mech, index);
  if (subsect < 0 || subsect >= max) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Critslot out of range!");
    return;
  }
  if (argc == 4) {
    if (!parse_int_checked(args[3], &newdata)) {
      mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                   "Invalid critical data!");
      return;
    }
  } else
    newdata = 0;
  mech_critical_part_type_set(mech, index, subsect,
                              itemcode < 0 ? 0
                                           : special_equipment_index(itemcode));
  mech_critical_data_set(mech, index, subsect, newdata);
  switch (itemcode) {
  case CASE:
    mech_section_configuration_add(
        mech, (mech_class(mech) == CLASS_VEH_GROUND) ? BSIDE : index,
        CASE_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "CASE Technology added to section.");
    break;
  case TRIPLE_STRENGTH_MYOMER:
    mech_technology_flags_add(mech, TRIPLE_MYOMER_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Triple Strength Myomer Technology added to 'Mech.");
    break;
  case MASC:
    mech_technology_flags_add(mech, MASC_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Myomer Accelerator Signal Circuitry added to 'Mech.");
    break;
  case C3_MASTER:
    mech_technology_flags_add(mech, C3_MASTER_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "C3 Command Unit added to 'Mech.");
    break;
  case C3_SLAVE:
    mech_technology_flags_add(mech, C3_SLAVE_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "C3 Slave Unit added to 'Mech.");
    break;
  case ARTEMIS_IV:
    mech_technology_flags_add(mech, ARTEMIS_IV_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Artemis IV Fire-Control System added to 'Mech.");
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "System will control the weapon which starts at slot %d.",
                  newdata);
    break;
  case ECM:
    mech_technology_flags_add(mech, ECM_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Guardian ECM Suite added to 'Mech.");
    break;
  case ANGELECM:
    mech_technology_flags_secondary_add(mech, ANGEL_ECM_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Angel ECM Suite added to 'Mech.");
    break;
  case BEAGLE_PROBE:
    mech_technology_flags_add(mech, BEAGLE_PROBE_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Beagle Active Probe added to 'Mech.");
    break;
  case LIGHT_BAP:
    mech_technology_flags_add(mech, LIGHT_BAP_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Light Beagle Active Probe added to 'Mech.");
    break;
  case TAG:
    mech_technology_flags_secondary_add(mech, TAG_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "TAG added to 'Mech.");
    break;
  case C3I:
    mech_technology_flags_secondary_add(mech, C3I_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Improved C3 added to 'Mech.");
    break;
  case BLOODHOUND_PROBE:
    mech_technology_flags_secondary_add(mech, BLOODHOUND_PROBE_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Bloodhound Active Probe added to 'Mech.");
    break;
  case TARGETING_COMPUTER:
    mech_technology_flags_secondary_add(mech, TCOMP_TECH);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Targeting Computer added to 'Mech.");
    break;
  case SPLIT_CRIT_LEFT:
  case SPLIT_CRIT_RIGHT:
    mech_critical_data_set(mech, index, subsect,
                           mech_critical_data(mech, index, subsect) - 1);
    break;
  }
  armor_string_from_index(index, location, mech_class(mech),
                          mech_movement_type(mech));
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Critical slot %s (%d) filled.", location, subsect + 1);
}

extern const char *specials[];
extern const char *specials2[];
extern const char *infantry_specials[];

const char *techstatus_func(Mech *mech) {
  int flags = mech_technology_flags(mech);
  int secondary_flags = mech_technology_flags_secondary(mech);
  return (flags || secondary_flags)
             ? template_bit_string_build(&(TemplateBitStringRequest){
                   .sets =
                       (TemplateBitSet[]){
                           {.descriptions = specials,
                            .count = primary_technology_name_count(),
                            .bits = flags},
                           {.descriptions = specials2,
                            .count = secondary_technology_name_count(),
                            .bits = secondary_flags}},
                   .set_count = 2,
                   .delimiter = '|',
                   .buffer = (char[BTECH_TEXT_CAPACITY]){0}})
             : "";
}

static bool bit_vector_to_flags(long value, int *flags) {
  if (value < INT_MIN || value > INT_MAX)
    return false;
  *flags = (int)value;
  return true;
}

void mechrep_rshowtech(DbRef player, void *data, char *buffer) {
  int i;
  int flags;
  int secondary_flags;
  int infantry_flags;
  char *techstring;
  char location[20];

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  flags = mech_technology_flags(mech);
  secondary_flags = mech_technology_flags_secondary(mech);
  infantry_flags = mech_infantry_technology_flags(mech);
  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "--------Advanced Technology--------");
  if (flags & TRIPLE_MYOMER_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Triple Strength Myomer");
  if (flags & MASC_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Myomer Accelerator Signal Circuitry");
  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_configuration_has(mech, i, CASE_TECH)) {
      armor_string_from_index(i, location, mech_class(mech),
                              mech_movement_type(mech));
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "Cellular Ammunition Storage Equipment in %s", location);
    }
  if (flags & CLAN_TECH) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Mech is set to Clan Tech.  This means:");
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "    Mech automatically has Double Heat Sink Tech");
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "    Mech automatically has CASE in all sections");
  }
  if (flags & DOUBLE_HEAT_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Mech uses Double Heat Sinks");
  if (flags & CL_ANTI_MISSILE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Clan style Anti-Missile System");
  if (flags & IS_ANTI_MISSILE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Inner Sphere style Anti-Missile System");
  if (flags & FLIPABLE_ARMS)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "The arms may be flipped into the rear firing arc");
  if (flags & C3_MASTER_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "C3 Command Computer");
  if (flags & C3_SLAVE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "C3 Slave Computer");
  if (flags & ARTEMIS_IV_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Artemis IV Fire-Control System");
  if (flags & ECM_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Guardian ECM Suite");
  if (flags & LIGHT_BAP_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Light Beagle Active Probe");
  if (secondary_flags & ANGEL_ECM_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Angel ECM Suite");
  if (flags & BEAGLE_PROBE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Beagle Active Probe");
  if (secondary_flags & TAG_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Target Aquisition Gear");
  if (secondary_flags & C3I_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Improved C3");
  if (secondary_flags & BLOODHOUND_PROBE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Bloodhound Active Probe");
  if (flags & ICE_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "It has ICE engine");

  /* Infantry related stuff */
  if (infantry_flags & INF_SWARM_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Can swarm enemy units");
  if (infantry_flags & INF_MOUNT_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Can mount friendly units");
  if (infantry_flags & INF_ANTILEG_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Can do anti-leg attacks");
  if (infantry_flags & CS_PURIFIER_STEALTH_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Has CS Purifier Stealth");
  if (infantry_flags & DC_KAGE_STEALTH_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Has DC Kage Stealth");
  if (infantry_flags & FWL_ACHILEUS_STEALTH_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Has FWL Achileus Stealth");
  if (infantry_flags & FC_INFILTRATOR_STEALTH_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Has FC Infiltrator Stealth");
  if (infantry_flags & FC_INFILTRATORII_STEALTH_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Has FC InfiltratorII Stealth");
  if (infantry_flags & MUST_JETTISON_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Must jettison backpack before jumping/using specials");
  if (infantry_flags & CAN_JETTISON_TECH)
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Can jettison backpack");

  mecha_notify(btech_context_evaluation(rep->xcode.context), player,
               "Brief version (May have something previous hadn't):");
  char tech_buffer[BTECH_TEXT_CAPACITY];
  mechrep_gettechstring(mech, tech_buffer);
  techstring = tech_buffer;
  if (techstring && techstring[0])
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 techstring);
  else
    mecha_notify(btech_context_evaluation(rep->xcode.context), player, "-");
}

void mechrep_gettechstring(Mech *mech, char *buffer) {
  template_bit_string_build(&(TemplateBitStringRequest){
      .sets =
          (TemplateBitSet[]){{.descriptions = specials,
                              .count = primary_technology_name_count(),
                              .bits = mech_technology_flags(mech)},
                             {.descriptions = specials2,
                              .count = secondary_technology_name_count(),
                              .bits = mech_technology_flags_secondary(mech)},
                             {.descriptions = infantry_specials,
                              .count = infantry_technology_name_count(),
                              .bits = mech_infantry_technology_flags(mech)}},
      .set_count = 3,
      .delimiter = ' ',
      .buffer = buffer});
}

static void remove_critical_type(Mech *mech, int part_type) {
  for (int section = 0; section < NUM_SECTIONS; ++section)
    for (int critical = 0; critical < NUM_CRITICALS; ++critical)
      if (mech_critical_part_type(mech, section, critical) == part_type)
        mech_critical_part_type_set(mech, section, critical, EMPTY);
}

static void remove_case_technology(Mech *mech) {
  remove_critical_type(mech, special_equipment_index(CASE));
  for (int section = 0; section < NUM_SECTIONS; ++section)
    mech_section_configuration_remove(mech, section, CASE_TECH);
}

void mechrep_rdeltech(DbRef player, void *data, char *buffer) {
  int nv, nv2;
  const long PARSED_NV =
      build_bit_vector(specials, primary_technology_name_count(), buffer);
  const long PARSED_NV2 =
      build_bit_vector(specials2, secondary_technology_name_count(), buffer);

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  /* Compare what the user gave to our specials lists */
  if (!bit_vector_to_flags(PARSED_NV, &nv) ||
      !bit_vector_to_flags(PARSED_NV2, &nv2)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Technology flags are out of range.");
    return;
  }

  /* Make sure what they gave was valid */
  if (((nv < 0) && (nv2 < 0)) && (strcasecmp(buffer, "all") != 0) &&
      (strcasecmp(buffer, "Case") != 0)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid tech: Available techs:");
    mecha_notify(btech_context_evaluation(rep->xcode.context), player, "\tAll");
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "\tCase");

    for (size_t index = 0; index < primary_technology_name_count(); ++index)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", primary_technology_name(index));

    for (size_t index = 0; index < secondary_technology_name_count(); ++index)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", secondary_technology_name(index));

    return;
  }

  /* Check to see if user specified anything */
  if (((!nv) && (!nv2)) && (strcasecmp(buffer, "all") != 0) &&
      (strcasecmp(buffer, "Case") != 0)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Nothing specified");
    return;
  }

  /* Check to see if user specified 'ALL' */
  if (strcasecmp(buffer, "all") == 0) {

    remove_case_technology(mech);
    remove_critical_type(mech, special_equipment_index(TRIPLE_STRENGTH_MYOMER));
    remove_critical_type(mech, special_equipment_index(MASC));
    mech_technology_flags_set(mech, 0);
    mech_technology_flags_secondary_set(mech, 0);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "All Advanced Technology Removed");
    return;
  }

  if (strcasecmp(buffer, "Case") == 0) {
    remove_case_technology(mech);
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Case Technology Removed");
    return;
  }

  if (nv > 0) {

    if (strcasecmp(buffer, "TripleMyomerTech") == 0) {
      if (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH)
        remove_critical_type(mech,
                             special_equipment_index(TRIPLE_STRENGTH_MYOMER));
    } else if (strcasecmp(buffer, "Masc") == 0) {
      if (mech_technology_flags(mech) & MASC_TECH)
        remove_critical_type(mech, special_equipment_index(MASC));
    }

    mech_technology_flags_remove(mech, nv);
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "%s Technology Removed", buffer);

  } else {

    mech_technology_flags_secondary_remove(mech, nv2);
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "%s Technology Removed", buffer);
  }
}

void mechrep_raddtech(DbRef player, void *data, char *buffer) {
  int nv, nv2;
  const long PARSED_NV =
      build_bit_vector(specials, primary_technology_name_count(), buffer);
  const long PARSED_NV2 =
      build_bit_vector(specials2, secondary_technology_name_count(), buffer);

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!bit_vector_to_flags(PARSED_NV, &nv) ||
      !bit_vector_to_flags(PARSED_NV2, &nv2)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Technology flags are out of range.");
    return;
  }

  if ((nv < 0) && (nv2 < 0)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid tech: Available techs:");

    for (size_t index = 0; index < primary_technology_name_count(); ++index)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", primary_technology_name(index));

    for (size_t index = 0; index < secondary_technology_name_count(); ++index)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", secondary_technology_name(index));

    return;
  }

  if ((!nv) && (!nv2)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Nothing set!");
    return;
  }

  if (nv > 0) {
    mech_technology_flags_add(mech, nv);
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player, "Set: %s",
        template_bit_string_build(&(TemplateBitStringRequest){
            .sets = &(TemplateBitSet){.descriptions = specials,
                                      .count = primary_technology_name_count(),
                                      .bits = nv},
            .set_count = 1,
            .delimiter = ' ',
            .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
  } else {
    mech_technology_flags_secondary_add(mech, nv2);
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player, "Set: %s",
        template_bit_string_build(&(TemplateBitStringRequest){
            .sets =
                &(TemplateBitSet){.descriptions = specials2,
                                  .count = secondary_technology_name_count(),
                                  .bits = nv2},
            .set_count = 1,
            .delimiter = ' ',
            .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
  }
}

void mechrep_rdelinftech(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  mech_infantry_technology_flags_set(mech, 0);
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "Advanced Infantry Technology Deleted");
}

void mechrep_raddinftech(DbRef player, void *data, char *buffer) {
  int nv;
  const long PARSED_NV = build_bit_vector(
      infantry_specials, infantry_technology_name_count(), buffer);

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  if (!bit_vector_to_flags(PARSED_NV, &nv)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Technology flags are out of range.");
    return;
  }

  if (mech_class(mech) != CLASS_BSUIT) {
    mecha_notify(
        btech_context_evaluation(rep->xcode.context), player,
        "That is not a valid target for infantry technologies. Try a Suit!");
    return;
  }

  if (nv < 0) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid infantry tech: Available techs:");

    for (size_t index = 0; index < infantry_technology_name_count(); ++index)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", infantry_technology_name(index));
    return;
  }

  if (!nv) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Nothing set!");
    return;
  }

  if (nv > 0) {
    mech_infantry_technology_flags_add(mech, nv);
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player, "Set: %s",
        template_bit_string_build(&(TemplateBitStringRequest){
            .sets = &(TemplateBitSet){.descriptions = infantry_specials,
                                      .count = infantry_technology_name_count(),
                                      .bits = nv},
            .set_count = 1,
            .delimiter = ' ',
            .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
  }
}

void mechrep_setcargospace(DbRef player, void *data, char *buffer) {
  char *args[2];
  int argc;
  int cargo;
  int max;

  RepairFacilityCommandContext repair_command;
  RepairCommandStatus repair_status =
      repair_facility_command_context_initialize(player, data, true,
                                                 &repair_command);
  if (repair_status != REPAIR_COMMAND_READY) {
    if (repair_command.evaluation)
      mecha_notify(repair_command.evaluation, player,
                   repair_command_status_message(repair_status));
    return;
  }
  RepairFacility *rep = repair_command.facility;
  Mech *mech = repair_command.mech;
  argc = mech_parseattributes(buffer, args, 2);
  if (argc != 2) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid number of arguements!");
    return;
  }

  if (!parse_int_checked(args[0], &cargo)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Doesn't that seem excessive?");
    return;
  }
  if (cargo < 0 || cargo > 5000) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Doesn't that seem excessive?");
    return;
  }
  cargo *= 50;
  mech_cargo_space_set(mech, cargo);

  if (!parse_int_checked(args[1], &max)) {
    mecha_notify(btech_context_evaluation(rep->xcode.context), player,
                 "Invalid maximum tonnage!");
    return;
  }
  max = (bounded(1, max, 100));
  mech_carrier_maximum_tonnage_set(mech, max);

  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "%3.2f cargospace and %d tons of maxton space set.",
                (double)cargo / 100.0, max);
}

struct MechReferenceCache {
  Mech *mech;
  char reference[1024];
};

Mech *load_refmech(BtechContext *context, const char *reference) {
  MechReferenceCache *cache = context->reference_mech_cache;

  if (cache == nullptr) {
    cache = calloc(1, sizeof(*cache));
    if (cache == nullptr)
      return nullptr;
    cache->mech = mech_temporary_create(context);
    if (cache->mech == nullptr) {
      free(cache);
      return nullptr;
    }
    context->reference_mech_cache = cache;
  }

  if (!strcmp(cache->reference, reference))
    return cache->mech;
  if (mech_template_load(GOD, cache->mech, reference) < 1) {
    cache->reference[0] = '\0';
    return nullptr;
  }
  (void)snprintf(cache->reference, sizeof(cache->reference), "%s", reference);
  return cache->mech;
}

void mech_reference_cache_destroy(BtechContext *context) {
  if (context == nullptr || context->reference_mech_cache == nullptr)
    return;
  mech_temporary_destroy(context->reference_mech_cache->mech);
  free(context->reference_mech_cache);
  context->reference_mech_cache = nullptr;
}
