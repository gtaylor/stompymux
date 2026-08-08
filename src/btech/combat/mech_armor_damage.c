#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_ood_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"

static const char *const MyColorStrings[] = {"", "[fg=green bold]",
                                             "[fg=yellow bold]", "[fg=red]"};
static const char *const MyMessageStrings[] = {
    "ERROR[reset]", "low.[reset]", "critical![reset]", "BREACHED![reset]"};
static inline const char *MySeriousColorStr(Mech *mech, int index) {
  return MyColorStrings[index % 4];
}

static inline const char *MySeriousStr(Mech *mech, int index) {
  return MyMessageStrings[index % 4];
}

static inline int MySeriousnessCheck(Mech *mech, int hitloc) {
  int orig, new;

  if (!(orig = mech_section_original_armor(mech, hitloc)))
    return 0;
  if (!(new = mech_section_armor(mech, hitloc)))
    return 3;
  if (new < orig / 4)
    return 2;
  if (new < orig / 2)
    return 1;
  return 0;
}

static inline int MySeriousnessCheckR(Mech *mech, int hitloc) {
  int orig, new;

  if (!(orig = mech_section_original_rear_armor(mech, hitloc)))
    return 0;
  if (!(new = mech_section_rear_armor(mech, hitloc)))
    return 3;
  if (new < orig / 4)
    return 2;
  if (new < orig / 2)
    return 1;
  return 0;
}

int cause_armordamage(Mech *wounded, Mech *attacker, int LOS,
                      DbRef attack_pilot, int isrear, int iscritical,
                      int hitloc, int damage, int *crits, int wWeapIndx,
                      int wAmmoMode) {
  int intDamage = 0, r;
  int seriousness = 0;
  int tAPCritical = 0;
  int wPercentLeft = 0;

  if (mech_class(wounded) == CLASS_MW)
    return (damage > 0) ? damage : 0;

  if ((mech_technology_flags(wounded) & HARDA_TECH) && damage > 0)
    damage = (damage + 1) / 2;

  /* Now decrement armor, and if neccessary, handle criticals... */
  if (mech_class(wounded) == CLASS_MECH && isrear &&
      (hitloc == CTORSO || hitloc == RTORSO || hitloc == LTORSO)) {

    if ((mech_section_rear_armor(wounded, hitloc) - damage) >= 0) {

      wPercentLeft =
          (((mech_section_rear_armor(wounded, hitloc) - damage) * 100) /
           mech_section_original_rear_armor(wounded, hitloc));
    }

    intDamage = damage - mech_section_rear_armor(wounded, hitloc);

    if (intDamage > 0) {
      mech_section_rear_armor_set(wounded, hitloc, 0);
      if (intDamage != damage)
        seriousness = 3;
    } else {
      seriousness = MySeriousnessCheckR(wounded, hitloc);
      mech_section_rear_armor_set(
          wounded, hitloc, mech_section_rear_armor(wounded, hitloc) - damage);
      seriousness = (seriousness == MySeriousnessCheckR(wounded, hitloc))
                        ? 0
                        : MySeriousnessCheckR(wounded, hitloc);
    }

  } else {

    /* Silly stuff */
    /*
       mech_section_armor_set(wounded, hitloc, MAX(0, intDamage =
       mech_section_armor(wounded, hitloc) - damage));
       intDamage = abs(intDamage);
     */

    if (mech_section_original_armor(wounded, hitloc) &&
        ((mech_section_armor(wounded, hitloc) - damage) >= 0)) {

      wPercentLeft = (((mech_section_armor(wounded, hitloc) - damage) * 100) /
                      mech_section_original_armor(wounded, hitloc));
    }

    intDamage = damage - mech_section_armor(wounded, hitloc);

    if (intDamage > 0) {
      mech_section_armor_set(wounded, hitloc, 0);
      if (intDamage != damage)
        seriousness = 3;
    } else {
      seriousness = MySeriousnessCheck(wounded, hitloc);
      mech_section_armor_set(wounded, hitloc,
                             mech_section_armor(wounded, hitloc) - damage);
      seriousness = (seriousness == MySeriousnessCheck(wounded, hitloc))
                        ? 0
                        : MySeriousnessCheck(wounded, hitloc);
    }

    if (!mech_section_armor(wounded, hitloc))
      mech_flood_section(wounded, hitloc, mech_position_z(wounded));
  }

  if (!iscritical && (wAmmoMode & AC_AP_MODE) && (intDamage <= 0) &&
      (wPercentLeft < 50))
    tAPCritical = 1;

  if (iscritical || tAPCritical) {
    BtechContext *context = mech_context(wounded);
    r = btech_random_roll(context);
    btech_context_critical_roll_record(context, r);
    /* Do the AP ammo thang */
    if (tAPCritical) {
      if (!strcmp(&MechWeapons[wWeapIndx].name[3], "AC/2"))
        r -= 4;
      else if (!strcmp(&MechWeapons[wWeapIndx].name[3], "LightAC/2"))
        r -= 4;
      else if (!strcmp(&MechWeapons[wWeapIndx].name[3], "AC/5"))
        r -= 3;
      else if (!strcmp(&MechWeapons[wWeapIndx].name[3], "LightAC/5"))
        r -= 3;
      else if (!strcmp(&MechWeapons[wWeapIndx].name[3], "AC/10"))
        r -= 2;
      else if (!strcmp(&MechWeapons[wWeapIndx].name[3], "AC/20"))
        r -= 1;
      else
        r -= 10;
    }

    switch (r) {
    case 8:
    case 9:
      mech_critical_handle(wounded, attacker, LOS, hitloc, 1);
      (*crits) += 1;
      break;
    case 10:
    case 11:
      mech_critical_handle(wounded, attacker, LOS, hitloc, 2);
      (*crits) += 2;
      break;
    case 12:
      mech_critical_handle(wounded, attacker, LOS, hitloc, 3);
      (*crits) += 3;
      break;
    default:
      break;
    }
    iscritical = 0;
  }

  if (mech_class(wounded) == CLASS_AERO && intDamage >= 0) {
    mech_section_destroy(wounded, attacker, LOS, hitloc);
    if (mech_is_destroyed(wounded)) {
      intDamage = 0;
      return 0;
    }
    switch (hitloc) {
    case AERO_AFT:
      mech_make_fall(wounded);
      mech_current_speed_set(wounded, 0);
      mech_max_speed_set(wounded, 0);
      mech_vertical_speed_set(wounded, 0);
      if (!mech_is_landed(wounded))
        mech_notify(wounded, MECHALL, "You feel the thrust die..");
      else
        mech_notify(wounded, MECHALL, "The computer reports engine destroyed!");
      if (!mech_is_landed(wounded))
        mech_event_schedule(wounded, EVENT_FALL, mech_fall_event, FALL_TICK,
                            -1);
      break;
    }
  }

  if (seriousness > 0 && mech_armor_warning_enabled(wounded))
    mech_printf(wounded, MECHALL, "%sWARNING: %s%s Armor %s",
                MySeriousColorStr(wounded, seriousness),
                armor_section_abbreviation(mech_class(wounded),
                                           mech_movement_type(wounded), hitloc)
                    .text,
                isrear ? " (Rear)" : "", MySeriousStr(wounded, seriousness));

  return intDamage > 0 ? intDamage : 0;
}

int cause_internaldamage(Mech *wounded, Mech *attacker, int LOS,
                         DbRef attack_pilot, int isrear, int hitloc,
                         int intDamage, int weapindx, int *crits) {
  BtechContext *context = mech_context(wounded);
  int r = btech_random_roll(context);
  char locname[30];
  char msgbuf[MBUF_SIZE];

  ArmorStringFromIndex(hitloc, locname, mech_class(wounded),
                       mech_movement_type(wounded));
  if ((mech_technology_flags(wounded) & REINFI_TECH) && intDamage > 0)
    intDamage = (intDamage + 1) / 2;
  else if (mech_technology_flags(wounded) & COMPI_TECH)
    intDamage = intDamage * 2;
  /* Critical hits? */
  btech_context_critical_roll_record(context, r);
  if (!(*crits))
    switch (r) {
    case 8:
    case 9:
      mech_critical_handle(wounded, attacker, LOS, hitloc, 1);
      break;
    case 10:
    case 11:
      mech_critical_handle(wounded, attacker, LOS, hitloc, 2);
      break;
    case 12:
      if (mech_class(wounded) == CLASS_MECH ||
          mech_class(wounded) == CLASS_MW) {
        switch (hitloc) {
        case RARM:
        case LARM:
        case RLEG:
        case LLEG:
        case HEAD:
          /* Limb blown off */
          mech_notify(wounded, MECHALL,
                      "[fg=yellow bold]CRITICAL HIT!![reset]");
          if (!mech_is_destroyed(wounded)) {
            snprintf(msgbuf, sizeof(msgbuf),
                     "'s %s is blown off in a shower of sparks and smoke!",
                     locname);
            mech_los_broadcast(wounded, msgbuf);
          }
          mech_section_destroy(wounded, attacker, LOS, hitloc);
          if (mech_class(wounded) != CLASS_MW)
            intDamage = 0;
          break;
        default:
          /* Ouch */
          mech_critical_handle(wounded, attacker, LOS, hitloc, 3);
          break;
        }
      } else {
        mech_critical_handle(wounded, attacker, LOS, hitloc, 3);
      }

      break;
    default:
      break;
      /* No critical hit */
    }
  /* Hmm.. This should be interesting */
  if (mech_class(wounded) == CLASS_MECH && intDamage && hitloc == CTORSO &&
      mech_section_internal(wounded, hitloc) ==
          mech_section_original_internal(wounded, hitloc))
    mech_reactor_instability_start_tick_set(wounded,
                                            btech_context_event_tick(context));

  if (mech_section_internal(wounded, hitloc) <= intDamage) {
    intDamage -= mech_section_internal(wounded, hitloc);
    mech_section_destroy(wounded, attacker, LOS, hitloc);

  } else {
    mech_section_internal_set(
        wounded, hitloc, mech_section_internal(wounded, hitloc) - intDamage);
    intDamage = 0;
  }
  return intDamage;
}
