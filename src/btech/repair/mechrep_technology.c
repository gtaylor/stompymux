/*
 * Last modified: Thu Aug 13 23:41:12 1998 fingon
 * Copyright (c) 1999-2005 Kevin Stevens
 *   All right reserved
 */

#include <ctype.h>
#include <dirent.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_events.h"
#include "mech_lifecycle.h" // IWYU pragma: keep
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define MECH_STAT_C /* want to use the POSIX stat() call. */

#include "mech.h"
#include "mech_build_api.h"
#include "mech_consistency_api.h"
#include "mech_restrict_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
#include "template_api.h"

/* Selectors */
#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

extern char *strtok(char *s, const char *ct);

#define MECHREP_COMMON(a)                                                      \
  struct RepairFacility *rep = (struct RepairFacility *)data;                  \
  Mech *mech;                                                                  \
  DOCHECK_CONTEXT(rep->xcode.context,                                          \
                  !is_god(rep->xcode.context->database, player) &&             \
                      !is_wizard(rep->xcode.context->database, player),        \
                  "I'm sorry Dave, can't do that.");                           \
  if (a) {                                                                     \
    DOCHECK_CONTEXT(rep->xcode.context, rep->current_target == -1,             \
                    "You must set a target first!");                           \
    mech = btech_context_get_mech(rep->xcode.context, rep->current_target);    \
    DOCHECK_CONTEXT(rep->xcode.context, mech == nullptr,                       \
                    "The target's BTech data is not allocated.");              \
  }

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

void mechrep_Raddspecial(DbRef player, void *data, char *buffer) {
  char *args[4];
  char location[20];
  int argc;
  int index;
  int itemcode;
  int subsect;
  int newdata;
  int max;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 4);
  DOCHECK_CONTEXT(rep->xcode.context, argc <= 2,
                  "Invalid number of arguments!");
  itemcode = FindSpecialItemCodeFromString(rep->xcode.context, args[0]);

  if (itemcode == -1)
    if (strcasecmp(args[0], "empty")) {
      notify(btech_context_evaluation(rep->xcode.context), player,
             "That is not a valid special object!");
      DumpMechSpecialObjects(mech->xcode.context, player);
      return;
    }
  index = ArmorSectionFromString(MechType(mech), MechMove(mech), args[1]);

  if (index == -1) {
    // Invalid section entered. Emit error and valid sections.
    invalid_section(player, mech);
    return;
  }
  subsect = atoi(args[2]);
  subsect--;
  max = CritsInLoc(mech, index);
  DOCHECK_CONTEXT(rep->xcode.context, subsect < 0 || subsect >= max,
                  "Critslot out of range!");
  if (argc == 4)
    newdata = atoi(args[3]);
  else
    newdata = 0;
  MechSections(mech)[index].criticals[subsect].type =
      itemcode < 0 ? 0 : I2Special(itemcode);
  MechSections(mech)[index].criticals[subsect].data = newdata;
  switch (itemcode) {
  case CASE:
    MechSections(mech)[(MechType(mech) == CLASS_VEH_GROUND) ? BSIDE : index]
        .config |= CASE_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "CASE Technology added to section.");
    break;
  case TRIPLE_STRENGTH_MYOMER:
    MechSpecials(mech) |= TRIPLE_MYOMER_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Triple Strength Myomer Technology added to 'Mech.");
    break;
  case MASC:
    MechSpecials(mech) |= MASC_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Myomer Accelerator Signal Circuitry added to 'Mech.");
    break;
  case C3_MASTER:
    MechSpecials(mech) |= C3_MASTER_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "C3 Command Unit added to 'Mech.");
    break;
  case C3_SLAVE:
    MechSpecials(mech) |= C3_SLAVE_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "C3 Slave Unit added to 'Mech.");
    break;
  case ARTEMIS_IV:
    MechSpecials(mech) |= ARTEMIS_IV_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Artemis IV Fire-Control System added to 'Mech.");
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "System will control the weapon which starts at slot %d.",
                  newdata);
    break;
  case ECM:
    MechSpecials(mech) |= ECM_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Guardian ECM Suite added to 'Mech.");
    break;
  case ANGELECM:
    MechSpecials2(mech) |= ANGEL_ECM_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Angel ECM Suite added to 'Mech.");
    break;
  case BEAGLE_PROBE:
    MechSpecials(mech) |= BEAGLE_PROBE_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Beagle Active Probe added to 'Mech.");
    break;
  case LIGHT_BAP:
    MechSpecials(mech) |= LIGHT_BAP_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Light Beagle Active Probe added to 'Mech.");
    break;
  case TAG:
    MechSpecials2(mech) |= TAG_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "TAG added to 'Mech.");
    break;
  case C3I:
    MechSpecials2(mech) |= C3I_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Improved C3 added to 'Mech.");
    break;
  case BLOODHOUND_PROBE:
    MechSpecials2(mech) |= BLOODHOUND_PROBE_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Bloodhound Active Probe added to 'Mech.");
    break;
  case TARGETING_COMPUTER:
    MechSpecials2(mech) |= TCOMP_TECH;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Targeting Computer added to 'Mech.");
    break;
  case SPLIT_CRIT_LEFT:
  case SPLIT_CRIT_RIGHT:
    MechSections(mech)[index].criticals[subsect].data--;
    break;
  }
  ArmorStringFromIndex(index, location, MechType(mech), MechMove(mech));
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Critical slot %s (%d) filled.", location, subsect + 1);
}

extern char *specials[];
extern char *specials2[];
extern char *infantry_specials[];

char *techstatus_func(Mech *mech) {
  return (MechSpecials(mech) || MechSpecials2(mech))
             ? build_bit_string_delimited2(
                   specials, specials2, MechSpecials(mech), MechSpecials2(mech),
                   (char[BTECH_TEXT_CAPACITY]){0})
             : "";
}

void mechrep_Rshowtech(DbRef player, void *data, char *buffer) {
  int i;
  char *techstring;
  char location[20];

  MECHREP_COMMON(1);
  notify(btech_context_evaluation(mech->xcode.context), player,
         "--------Advanced Technology--------");
  if (MechSpecials(mech) & TRIPLE_MYOMER_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Triple Strength Myomer");
  if (MechSpecials(mech) & MASC_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Myomer Accelerator Signal Circuitry");
  for (i = 0; i < NUM_SECTIONS; i++)
    if (MechSections(mech)[i].config & CASE_TECH) {
      ArmorStringFromIndex(i, location, MechType(mech), MechMove(mech));
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "Cellular Ammunition Storage Equipment in %s", location);
    }
  if (MechSpecials(mech) & CLAN_TECH) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Mech is set to Clan Tech.  This means:");
    notify(btech_context_evaluation(rep->xcode.context), player,
           "    Mech automatically has Double Heat Sink Tech");
    notify(btech_context_evaluation(rep->xcode.context), player,
           "    Mech automatically has CASE in all sections");
  }
  if (MechSpecials(mech) & DOUBLE_HEAT_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Mech uses Double Heat Sinks");
  if (MechSpecials(mech) & CL_ANTI_MISSILE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Clan style Anti-Missile System");
  if (MechSpecials(mech) & IS_ANTI_MISSILE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Inner Sphere style Anti-Missile System");
  if (MechSpecials(mech) & FLIPABLE_ARMS)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "The arms may be flipped into the rear firing arc");
  if (MechSpecials(mech) & C3_MASTER_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "C3 Command Computer");
  if (MechSpecials(mech) & C3_SLAVE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "C3 Slave Computer");
  if (MechSpecials(mech) & ARTEMIS_IV_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Artemis IV Fire-Control System");
  if (MechSpecials(mech) & ECM_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Guardian ECM Suite");
  if (MechSpecials(mech) & LIGHT_BAP_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Light Beagle Active Probe");
  if (MechSpecials2(mech) & ANGEL_ECM_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Angel ECM Suite");
  if (MechSpecials(mech) & BEAGLE_PROBE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Beagle Active Probe");
  if (MechSpecials2(mech) & TAG_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Target Aquisition Gear");
  if (MechSpecials2(mech) & C3I_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player, "Improved C3");
  if (MechSpecials2(mech) & BLOODHOUND_PROBE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Bloodhound Active Probe");
  if (MechSpecials(mech) & ICE_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "It has ICE engine");

  /* Infantry related stuff */
  if (MechInfantrySpecials(mech) & INF_SWARM_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Can swarm enemy units");
  if (MechInfantrySpecials(mech) & INF_MOUNT_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Can mount friendly units");
  if (MechInfantrySpecials(mech) & INF_ANTILEG_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Can do anti-leg attacks");
  if (MechInfantrySpecials(mech) & CS_PURIFIER_STEALTH_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Has CS Purifier Stealth");
  if (MechInfantrySpecials(mech) & DC_KAGE_STEALTH_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Has DC Kage Stealth");
  if (MechInfantrySpecials(mech) & FWL_ACHILEUS_STEALTH_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Has FWL Achileus Stealth");
  if (MechInfantrySpecials(mech) & FC_INFILTRATOR_STEALTH_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Has FC Infiltrator Stealth");
  if (MechInfantrySpecials(mech) & FC_INFILTRATORII_STEALTH_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Has FC InfiltratorII Stealth");
  if (MechInfantrySpecials(mech) & MUST_JETTISON_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Must jettison backpack before jumping/using specials");
  if (MechInfantrySpecials(mech) & CAN_JETTISON_TECH)
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Can jettison backpack");

  notify(btech_context_evaluation(rep->xcode.context), player,
         "Brief version (May have something previous hadn't):");
  char tech_buffer[BTECH_TEXT_CAPACITY];
  mechrep_gettechstring(mech, tech_buffer);
  techstring = tech_buffer;
  if (techstring && techstring[0])
    notify(btech_context_evaluation(rep->xcode.context), player, techstring);
  else
    notify(btech_context_evaluation(rep->xcode.context), player, "-");
}

void mechrep_gettechstring(Mech *mech, char *buffer) {
  build_bit_string3(specials, specials2, infantry_specials, MechSpecials(mech),
                    MechSpecials2(mech), MechInfantrySpecials(mech), buffer);
}

void mechrep_Rdeltech(DbRef player, void *data, char *buffer) {
  int i, j;
  int Type;
  int nv, nv2;

  MECHREP_COMMON(1);
  /* Compare what the user gave to our specials lists */
  nv = BuildBitVector(specials, buffer);
  nv2 = BuildBitVector(specials2, buffer);

  /* Make sure what they gave was valid */
  if (((nv < 0) && (nv2 < 0)) && (strcasecmp(buffer, "all") != 0) &&
      (strcasecmp(buffer, "Case") != 0)) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Invalid tech: Available techs:");
    notify(btech_context_evaluation(rep->xcode.context), player, "\tAll");
    notify(btech_context_evaluation(rep->xcode.context), player, "\tCase");

    for (nv = 0; specials[nv]; nv++)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", specials[nv]);

    for (nv = 0; specials2[nv]; nv++)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", specials2[nv]);

    return;
  }

  /* Check to see if user specified anything */
  if (((!nv) && (!nv2)) && (strcasecmp(buffer, "all") != 0) &&
      (strcasecmp(buffer, "Case") != 0)) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Nothing specified");
    return;
  }

  /* Check to see if user specified 'ALL' */
  if (strcasecmp(buffer, "all") == 0) {

    for (i = 0; i < NUM_SECTIONS; i++) {

      if ((MechSections(mech)[i].config & CASE_TECH) ||
          (MechSpecials(mech) & TRIPLE_MYOMER_TECH) ||
          (MechSpecials(mech) & MASC_TECH)) {

        for (j = 0; j < NUM_CRITICALS; j++) {
          Type = MechSections(mech)[i].criticals[j].type;

          if (Type == I2Special((CASE)) ||
              Type == I2Special((TRIPLE_STRENGTH_MYOMER)) ||
              Type == I2Special((MASC))) {
            MechSections(mech)[i].criticals[j].type = EMPTY;
          }
        }
        MechSections(mech)[i].config &= ~CASE_TECH;
      }
    }

    MechSpecials(mech) = 0;
    MechSpecials2(mech) = 0;
    notify(btech_context_evaluation(rep->xcode.context), player,
           "All Advanced Technology Removed");
    return;
  }

  if (strcasecmp(buffer, "Case") == 0) {
    for (i = 0; i < NUM_SECTIONS; i++) {
      if (MechSections(mech)[i].config & CASE_TECH) {
        for (j = 0; j < NUM_CRITICALS; j++) {
          Type = MechSections(mech)[i].criticals[j].type;

          if (Type == I2Special((CASE))) {
            MechSections(mech)[i].criticals[j].type = EMPTY;
          }
        }
        MechSections(mech)[i].config &= ~CASE_TECH;
      }
    }
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Case Technology Removed");
    return;
  }

  if (nv > 0) {

    if (strcasecmp(buffer, "TripleMyomerTech") == 0) {
      if (MechSpecials(mech) & TRIPLE_MYOMER_TECH) {
        for (i = 0; i < NUM_SECTIONS; i++) {
          for (j = 0; j < NUM_CRITICALS; j++) {
            Type = MechSections(mech)[i].criticals[j].type;

            if (Type == I2Special((TRIPLE_STRENGTH_MYOMER))) {
              MechSections(mech)[i].criticals[j].type = EMPTY;
            }
          }
        }
      }
    } else if (strcasecmp(buffer, "Masc") == 0) {
      if (MechSpecials(mech) & MASC_TECH) {
        for (i = 0; i < NUM_SECTIONS; i++) {
          for (j = 0; j < NUM_CRITICALS; j++) {
            Type = MechSections(mech)[i].criticals[j].type;

            if (Type == I2Special((MASC))) {
              MechSections(mech)[i].criticals[j].type = EMPTY;
            }
          }
        }
      }
    }

    MechSpecials(mech) &= ~nv;
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "%s Technology Removed", buffer);

  } else {

    MechSpecials2(mech) &= ~nv2;
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "%s Technology Removed", buffer);
  }
  return;
}

void mechrep_Raddtech(DbRef player, void *data, char *buffer) {
  int nv, nv2;

  MECHREP_COMMON(1);
  nv = BuildBitVector(specials, buffer);
  nv2 = BuildBitVector(specials2, buffer);

  if ((nv < 0) && (nv2 < 0)) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Invalid tech: Available techs:");

    for (nv = 0; specials[nv]; nv++)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", specials[nv]);

    for (nv = 0; specials2[nv]; nv++)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", specials2[nv]);

    return;
  }

  if ((!nv) && (!nv2)) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Nothing set!");
    return;
  }

  if (nv > 0) {
    MechSpecials(mech) |= nv;
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player, "Set: %s",
        build_bit_string(specials, nv, (char[BTECH_TEXT_CAPACITY]){0}));
  } else {
    MechSpecials2(mech) |= nv2;
    notify_printf(
        btech_context_evaluation(rep->xcode.context), player, "Set: %s",
        build_bit_string(specials2, nv2, (char[BTECH_TEXT_CAPACITY]){0}));
  }
}

void mechrep_Rdelinftech(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  MechInfantrySpecials(mech) = 0;
  notify(btech_context_evaluation(mech->xcode.context), player,
         "Advanced Infantry Technology Deleted");
}

void mechrep_Raddinftech(DbRef player, void *data, char *buffer) {
  int nv;

  MECHREP_COMMON(1);
  nv = BuildBitVector(infantry_specials, buffer);

  if (MechType(mech) != CLASS_BSUIT) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "That is not a valid target for infantry technologies. Try a Suit!");
    return;
  }

  if (nv < 0) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Invalid infantry tech: Available techs:");

    for (nv = 0; infantry_specials[nv]; nv++)
      notify_printf(btech_context_evaluation(rep->xcode.context), player,
                    "\t%s", infantry_specials[nv]);
    return;
  }

  if (!nv) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Nothing set!");
    return;
  }

  if (nv > 0) {
    MechInfantrySpecials(mech) |= nv;
    notify_printf(btech_context_evaluation(rep->xcode.context), player,
                  "Set: %s",
                  build_bit_string(infantry_specials, nv,
                                   (char[BTECH_TEXT_CAPACITY]){0}));
  }
}

void mechrep_setcargospace(DbRef player, void *data, char *buffer) {
  char *args[2];
  int argc;
  int cargo;
  int max;

  MECHREP_COMMON(1);
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(rep->xcode.context, argc != 2,
                  "Invalid number of arguements!");

  cargo = (atoi(args[0]) * 50);
  DOCHECK_CONTEXT(rep->xcode.context, cargo < 0 || cargo > 250000,
                  "Doesn't that seem excessive?");
  CargoSpace(mech) = cargo;

  max = (atoi(args[1]));
  max = (BOUNDED(1, max, 100));
  CarMaxTon(mech) = (char)max;

  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "%3.2f cargospace and %d tons of maxton space set.",
                (float)((float)cargo / 100), (int)max);
}

struct MechReferenceCache {
  Mech mech;
  char reference[1024];
};

Mech *load_refmech(BtechContext *context, const char *reference) {
  MechReferenceCache *cache = context->reference_mech_cache;

  if (cache == nullptr) {
    cache = calloc(1, sizeof(*cache));
    if (cache == nullptr)
      return nullptr;
    cache->mech.xcode = (BtechSpecialObject){
        .type = GTYPE_MECH,
        .size = sizeof(cache->mech),
        .context = context,
    };
    context->reference_mech_cache = cache;
  }

  if (!strcmp(cache->reference, reference))
    return &cache->mech;
  if (mech_loadnew(GOD, &cache->mech, (char *)reference) < 1) {
    cache->reference[0] = '\0';
    return nullptr;
  }
  snprintf(cache->reference, sizeof(cache->reference), "%s", reference);
  return &cache->mech;
}

void mech_reference_cache_destroy(BtechContext *context) {
  if (context == nullptr)
    return;
  free(context->reference_mech_cache);
  context->reference_mech_cache = nullptr;
}
