/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "failures.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_sensor.h"
#include "mech_tag_api.h"
#include "mech_tech_commands_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"

int HandleMechCrit(Mech *wounded, Mech *attacker, int LOS, int hitloc,
                   int critHit, int critType, int critData) {
  Mech *mech = wounded;
  int weapindx, damage, destroycrit, weapon_slot, wFirstCrit;
  int temp;
  char locname[30];
  char msgbuf[MBUF_SIZE];
  int tLocIsArm = ((hitloc == LARM || hitloc == RARM) && !MechIsQuad(wounded));
  int tLocIsLeg = ((hitloc == LLEG || hitloc == RLEG) ||
                   ((hitloc == LARM || hitloc == RARM) && MechIsQuad(wounded)));
  char partBuf[100];

  int fCrit;
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, wounded->mapindex);

  ArmorStringFromIndex(hitloc, locname, MechType(wounded), MechMove(wounded));
  mech_notify(wounded, MECHALL, "[fg=yellow bold]CRITICAL HIT!![reset]");

  if (IsAmmo(critType)) {
    /* BOOM! */
    /* That's going to hurt... */
    weapindx = Ammo2WeaponI(critType);
    damage = critData * MechWeapons[weapindx].damage;
    if (IsMissile(weapindx) || IsArtillery(weapindx)) {
      const MissileHitEntry *entry = missile_hit_registry_find_weapon(
          &wounded->xcode.context->missile_hits, weapindx);
      if (entry != nullptr)
        damage *= entry->num_missiles[10];
    }
    if (MechWeapons[weapindx].special & (GAUSS | NOBOOM)) {
      if (MechWeapons[weapindx].special & GAUSS)
        mech_notify(wounded, MECHALL,
                    "One of your Gauss Rifle ammo feeds is destroyed");
      DestroyPart(wounded, hitloc, critHit);
    } else if (damage) {
      mech_ammunition_explode(attacker, wounded, hitloc, critHit, damage);
    } else {
      mech_notify(wounded, MECHALL,
                  "You have no ammunition left in that location, lucky you!");
      DestroyPart(wounded, hitloc, critHit);
    }
    return 1;
  }

  if (PartIsBroken(wounded, hitloc, critHit) && IsWeapon(critType) &&
      !PartIsDisabled(wounded, hitloc, critHit)) {
    while (--critHit && GetPartType(wounded, hitloc, critHit) == critType)
      if (PartIsDestroyed(wounded, hitloc, critHit))
        break;
    mech_printf(wounded, MECHALL, "Your destroyed %s is damaged some more!",
                &MechWeapons[Weapon2I(critType)].name[3]);
    DestroyPart(wounded, hitloc, critHit + 1);
    return 1;
  }

  if (PartIsNonfunctional(wounded, hitloc, critHit)) {
    if (IsSpecial(critType)) {
      switch (Special2I(critType)) {
      case LIFE_SUPPORT:
        strcpy(partBuf, "life support");
        break;
      case COCKPIT:
        strcpy(partBuf, "cockpit");
        break;
      case SENSORS:
        strcpy(partBuf, "sensors");
        break;
      case HEAT_SINK:
        strcpy(partBuf, "heatsink");
        break;
      case JUMP_JET:
        strcpy(partBuf, "jump jet");
        break;
      case ENGINE:
        strcpy(partBuf, "engine");
        break;
      case TARGETING_COMPUTER:
        strcpy(partBuf, "targeting computer");
        break;
      case GYRO:
        strcpy(partBuf, "gyro");
        break;
      case SHOULDER_OR_HIP:
        if (tLocIsArm)
          strcpy(partBuf, "shoulder");
        else
          strcpy(partBuf, "hip");
        break;
      case LOWER_ACTUATOR:
      case UPPER_ACTUATOR:
      case HAND_OR_FOOT_ACTUATOR:
        if (tLocIsArm) {
          if (Special2I(critType) == HAND_OR_FOOT_ACTUATOR)
            strcpy(partBuf, "hand actuator");
          else
            strcpy(partBuf, "arm actuator");
        } else {
          if (Special2I(critType) == HAND_OR_FOOT_ACTUATOR)
            strcpy(partBuf, "foot actuator");
          else
            strcpy(partBuf, "arm actuator");
        }
        break;
      case C3_MASTER:
        strcpy(partBuf, "C3 system");
        break;
      case C3_SLAVE:
        strcpy(partBuf, "C3 system");
        break;
      case C3I:
        strcpy(partBuf, "C3i system");
        break;
      case TAG:
        strcpy(partBuf, "TAG system");
        break;
      case ECM:
        strcpy(partBuf, "ECM system");
        break;
      case ANGELECM:
        strcpy(partBuf, "Angel ECM system");
        break;
      case BEAGLE_PROBE:
        strcpy(partBuf, "Beagle Active Probe");
        break;
      case BLOODHOUND_PROBE:
        strcpy(partBuf, "Bloodhound Active Probe");
        break;
      case LIGHT_BAP:
        strcpy(partBuf, "Light Beagle Active Probe");
        break;
      case ARTEMIS_IV:
        strcpy(partBuf, "ArtemisIV system");
        break;
      case AXE:
        strcpy(partBuf, "axe");
        break;
      case SWORD:
        strcpy(partBuf, "sword");
        break;
      case MACE:
        strcpy(partBuf, "mace");
        break;
      case DUAL_SAW:
        strcpy(partBuf, "dual saw");
        break;
      case DS_AERODOOR:
        strcpy(partBuf, "aero doors");
        break;
      case DS_MECHDOOR:
        strcpy(partBuf, "mech doors");
        break;
      case NULL_SIGNATURE_SYSTEM:
        strcpy(partBuf, "Null Signature System");
        break;
      } // end switch() - Part Names
    } // end if()

    if (IsWeapon(critType)) {
      mech_printf(wounded, MECHALL, "Part of your non-working %s has been hit!",
                  &MechWeapons[Weapon2I(critType)].name[3]);
    } else {
      mech_printf(wounded, MECHALL, "Part of your non-working %s has been hit!",
                  partBuf);
    }
    DestroyPart(wounded, hitloc, critHit);
    return 1;
  }

  if (IsWeapon(critType)) {
    if (handleWeaponCrit(attacker, wounded, hitloc, critHit, critType, LOS)) {
      return 1;
    }

    scoreEnhancedWeaponCriticalHit(mech, attacker, LOS, hitloc, critHit);

    /* Have to destroy all the weapons of this type in this section */
    /* DestroyWeapon(wounded, hitloc, critType, 1, GetWeaponCrits(wounded,
     * Weapon2I(critType))); */

    return 1;
  }

  if (IsSpecial(critType)) {
    destroycrit = 1;
    switch (Special2I(critType)) {
    case LIFE_SUPPORT:
      MechCritStatus(wounded) |= LIFE_SUPPORT_DESTROYED;
      mech_notify(wounded, MECHALL, "Your life support has been destroyed!");
      break;
    case COCKPIT:
      /* Destroy Mech for now, but later kill pilot as well */
      mech_notify(wounded, MECHALL,
                  "Your cockpit is destroyed, your blood boils, and your body "
                  "is fried! [fg=yellow]You're dead![reset]");
      if (!Destroyed(wounded)) {
        DestroyMech(wounded, attacker, 0, KILL_TYPE_COCKPIT);
      }

      if (LOS && attacker)
        mech_notify(attacker, MECHALL,
                    "You destroy the cockpit! The pilot's blood splatters down "
                    "the sides!");
      mech_los_broadcast(wounded,
                         "spasms for a second then remains oddly still.");
      MechPilot(wounded) = -1;
      KillMechContentsIfIC(wounded);
      break;
    case SENSORS:
      if (!(MechCritStatus(wounded) & SENSORS_DAMAGED)) {
        MechLRSRange(wounded) /= 2;
        MechTacRange(wounded) /= 2;
        MechScanRange(wounded) /= 2;
        MechBTH(wounded) += 2;
        MechCritStatus(wounded) |= SENSORS_DAMAGED;
        mech_notify(wounded, MECHALL, "Your sensors have been damaged!");
      } else {
        MechLRSRange(wounded) = 0;
        MechTacRange(wounded) = 0;
        MechScanRange(wounded) = 0;
        MechBTH(wounded) = 75;
        mech_notify(wounded, MECHALL, "Your sensors have been destroyed!");
      }
      break;
    case SPLIT_CRIT_LEFT:
    case SPLIT_CRIT_RIGHT:
      fCrit = GetPartData(wounded, hitloc, critHit);
      temp = ReverseSplitCritLoc(wounded, hitloc, critHit);
      if (temp < 0) {
        mech_printf(wounded, MECHALL,
                    "ERROR: Could not find split weapon parent location. "
                    "Loc:%d Crit:%d temp:%d fCrit:%d",
                    hitloc, critHit, temp, fCrit);
        break; // sanity check
      }
      destroycrit = 0;
      if (handleWeaponCrit(attacker, wounded, temp, fCrit,
                           GetPartType(wounded, temp, fCrit), LOS))
        break;
      scoreEnhancedWeaponCriticalHit(wounded, attacker, LOS, temp, fCrit);
      break;
    case HEAT_SINK:
      if (MechHasDHS(mech)) {
        wFirstCrit = FindFirstWeaponCrit(wounded, hitloc, critHit, 0, critType,
                                         HS_Size(mech));
        MechRealNumsinks(wounded) -= 2;
        DestroyWeapon(wounded, hitloc, critType, wFirstCrit, 1, HS_Size(mech));
        destroycrit = 0;
      } else
        MechRealNumsinks(wounded)--;
      mech_notify(wounded, MECHALL, "You lost a heat sink!");
      if (!Destroyed(wounded)) {
        snprintf(msgbuf, MBUF_SIZE, "'s %s is covered in a green mist!",
                 locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      break;
    case JUMP_JET:
      if (!Destroyed(wounded) && Started(wounded)) {
        snprintf(msgbuf, MBUF_SIZE,
                 "'s %s flares as superheated plasma spews out!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      /* IMPROVED JJ CHECK HERE. SIMILIAR TO DHS */
      if ((MechSpecials2(mech) & IMPROVED_JJ_TECH)) {
        wFirstCrit =
            FindFirstWeaponCrit(wounded, hitloc, critHit, 0, critType, 2);
        DestroyWeapon(wounded, hitloc, critType, wFirstCrit, 1, 2);
        destroycrit = 0;
      }
      MechJumpSpeed(wounded) -= MP1;
      if (MechJumpSpeed(wounded) < 0)
        MechJumpSpeed(wounded) = 0;
      mech_notify(wounded, MECHALL,
                  "One of your jump jet engines has shut down!");
      if (attacker && MechJumpSpeed(wounded) < MP1 && Jumping(wounded)) {
        mech_notify(wounded, MECHALL,
                    "Losing your last jump jet, you fall from the sky!");
        mech_los_broadcast(wounded, "falls from the sky!");
        mech_fall(wounded, 1, 0);
        mech_domino_resolve(wounded, MECH_DOMINO_FALL);
      }
      break;
    case ENGINE:
      if (!Destroyed(wounded) && Started(wounded)) {
        snprintf(msgbuf, MBUF_SIZE, "'s %s spews black smoke!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }
      if (MechEngineHeat(wounded) < 10) {
        MechEngineHeat(wounded) += 5;
        mech_notify(
            wounded, MECHALL,
            "Your engine shielding takes a hit! It's getting hotter in here!");
      } else if (MechEngineHeat(wounded) < 15) {
        MechEngineHeat(wounded) = 15;
        mech_notify(wounded, MECHALL, "Your engine is destroyed!");
        if (wounded != attacker && !(MechStatus(wounded) & DESTROYED) &&
            attacker)
          mech_notify(attacker, MECHALL, "You destroy the engine!");
        if (unit_is_fixable(mech))
          DestroyMech(wounded, attacker, 1, KILL_TYPE_ENGINE);
        else
          DestroyMech(wounded, attacker, 1, KILL_TYPE_NORMAL);
      }
      break;
    case TARGETING_COMPUTER:
      if (!(MechCritStatus(wounded) & TC_DESTROYED)) {
        mech_notify(wounded, MECHALL, "Your targeting computer is destroyed!");
        MechCritStatus(wounded) |= TC_DESTROYED;
      }
      break;
    case GYRO:
      /* Hardened Gyro's take one extra hit before damaged */
      if (MechSpecials2(wounded) & HDGYRO_TECH)
        if (!(MechCritStatus2(wounded) & HDGYRO_DAMAGED)) {
          snprintf(msgbuf, MBUF_SIZE,
                   "emits a screech as its "
                   "hardened gyro buckles slightly!");
          mech_los_broadcast(wounded, msgbuf);
          MechCritStatus2(wounded) |= HDGYRO_DAMAGED;
          mech_notify(wounded, MECHALL, "Your hardened gyro takes a hit!");
          break;
        }

      if (!(MechCritStatus(wounded) & GYRO_DAMAGED)) {
        if (!Destroyed(wounded) && Started(wounded)) {
          snprintf(msgbuf, MBUF_SIZE,
                   "emits a loud screech as "
                   "its gyro buckles under the impact!");
          mech_los_broadcast(wounded, msgbuf);
        }
        MechCritStatus(wounded) |= GYRO_DAMAGED;
        MechPilotSkillBase(wounded) += 3;
        mech_notify(wounded, MECHALL, "Your Gyro has been damaged!");
        if (attacker)
          if (!MadePilotSkillRoll(wounded, 0) && !Fallen(wounded)) {
            if (!Jumping(wounded) && !OODing(wounded)) {
              mech_notify(wounded, MECHALL,
                          "You lose your balance and fall down!");
              mech_los_broadcast(wounded, "stumbles and falls down.");
              mech_fall(wounded, 1, 0);
            } else {
              mech_notify(wounded, MECHALL, "You fall from the sky!");
              mech_los_broadcast(wounded, "falls from the sky!");
              mech_fall(wounded, JumpSpeedMP(wounded, map), 0);
              mech_domino_resolve(wounded, MECH_DOMINO_FALL);
            }
          }
      } else if (!(MechCritStatus(wounded) & GYRO_DESTROYED)) {
        MechCritStatus(wounded) |= GYRO_DESTROYED;
        mech_notify(wounded, MECHALL, "Your Gyro has been destroyed!");

        if (attacker) {
          if (!Fallen(wounded) && !Jumping(wounded) && !OODing(wounded)) {
            mech_notify(wounded, MECHALL, "You fall and you can't get up!");
            mech_los_broadcast(wounded, "is knocked over!");
            mech_fall(wounded, 1, 0);
          } else if (!Fallen(wounded) &&
                     (Jumping(wounded) || OODing(wounded))) {
            mech_notify(wounded, MECHALL, "You fall from the sky!");
            mech_los_broadcast(wounded, "falls from the sky!");
            mech_fall(wounded, JumpSpeedMP(wounded, map), 0);
            mech_domino_resolve(wounded, MECH_DOMINO_FALL);
          }
        }
      } else {
        mech_notify(wounded, MECHALL, "Your destroyed gyro takes another hit!");
      }
      break;
    case SHOULDER_OR_HIP:
      DestroyPart(wounded, hitloc, critHit);
      destroycrit = 0;

      if (tLocIsArm) {
        mech_notify(wounded, MECHALL,
                    "Your shoulder joint takes a hit and is frozen!");
        NormalizeLocActuatorCrits(wounded, hitloc);
      } else if (tLocIsLeg) {
        if (!Destroyed(wounded) && Started(wounded)) {
          snprintf(msgbuf, MBUF_SIZE, "'s hip locks into place!");
          mech_los_broadcast(wounded, msgbuf);
        }

        mech_notify(wounded, MECHALL,
                    "Your hip takes a direct hit and freezes up!");

        if (!(MechCritStatus(wounded) & HIP_DAMAGED)) {
          MechCritStatus(wounded) |= HIP_DAMAGED;
        } else {
          if (!MechIsQuad(wounded))
            MechCritStatus(wounded) |= HIP_DESTROYED;
        }

        NormalizeAllActuatorCrits(wounded);

        if (attacker && !Jumping(wounded) && !OODing(wounded) &&
            !MadePilotSkillRoll(wounded, 0)) {
          mech_notify(wounded, MECHALL, "You lose your balance and fall down!");
          mech_los_broadcast(wounded, "stumbles and falls down!");
          mech_fall(wounded, 1, 0);
        }
      }
      break;
    case LOWER_ACTUATOR:
    case UPPER_ACTUATOR:
    case HAND_OR_FOOT_ACTUATOR:
      DestroyPart(wounded, hitloc, critHit);
      destroycrit = 0;

      if (tLocIsArm) {
        if (Special2I(critType) == HAND_OR_FOOT_ACTUATOR)
          mech_printf(wounded, MECHALL, "Your %s hand actuator is destroyed!",
                      hitloc == LARM ? "left" : "right");
        else
          mech_printf(wounded, MECHALL, "Your %s %s arm actuator is destroyed!",
                      hitloc == LARM ? "left" : "right",
                      Special2I(critType) == LOWER_ACTUATOR ? "lower"
                                                            : "upper");

        if ((Special2I(critType) == HAND_OR_FOOT_ACTUATOR) &&
            (MechSections(mech)[hitloc].specials & CARRYING_CLUB))
          mech_drop_club(mech);
        if (MechCarrying(mech) > 0) {
          mech_notify(mech, MECHALL, "The hit causes your tow line to let go!");
          mech_los_broadcast(mech,
                             "'s tow lines release and flap freely behind it!");
          mech_dropoff(GOD, mech, "");
        }
        NormalizeLocActuatorCrits(wounded, hitloc);
      } else if (tLocIsLeg) {
        mech_notify(wounded, MECHALL,
                    "One of your leg actuators is destroyed!");

        if (OkayCritSectS(
                hitloc, 0,
                SHOULDER_OR_HIP)) { /* don't need to bother with crits if we
                                       already have a hip crit here */
          if (!Destroyed(wounded) && Started(wounded)) {
            snprintf(msgbuf, MBUF_SIZE, "'s %s twists in an odd way!", locname);
            mech_los_broadcast(wounded, msgbuf);
          }

          NormalizeAllActuatorCrits(wounded);

          if (attacker && !Jumping(wounded) && !OODing(wounded) &&
              !MadePilotSkillRoll(wounded, 0)) {
            mech_notify(wounded, MECHALL,
                        "You lose your balance and fall down!");
            mech_los_broadcast(wounded, "stumbles and falls down!");
            mech_fall(wounded, 1, 0);
          }
        }
      }
      break;
    case C3_MASTER:
      temp = MechWorkingC3Masters(mech);
      MechWorkingC3Masters(mech) = mech_c3_working_master_count(mech);

      if (temp == MechWorkingC3Masters(mech))
        mech_notify(wounded, MECHALL,
                    "Your destroyed C3 system takes another hit!");
      else {
        if (MechWorkingC3Masters(mech) == 0) {
          MechCritStatus(wounded) |= C3_DESTROYED;

          mech_tag_check(mech);
        }

        if (MechTotalC3Masters(mech))
          mech_notify(wounded, MECHALL,
                      "One of your C3 systems has been destroyed!");
        else
          mech_notify(wounded, MECHALL, "Your C3 system has been destroyed!");
      }

      break;
    case C3_SLAVE:
      MechCritStatus(wounded) |= C3_DESTROYED;
      mech_notify(wounded, MECHALL, "Your C3 system has been destroyed!");
      break;
    case C3I:
      MechCritStatus(wounded) |= C3I_DESTROYED;
      mech_notify(wounded, MECHALL, "Your C3i system has been destroyed!");

      mech_c3i_network_clear(mech, 1);
      break;
    case TAG:
      MechCritStatus(wounded) |= TAG_DESTROYED;
      mech_notify(wounded, MECHALL, "Your TAG system has been destroyed!");

      mech_tag_check(mech);
      break;
    case ECM:
      MechCritStatus(wounded) |= ECM_DESTROYED;
      mech_notify(wounded, MECHALL, "Your ECM system has been destroyed!");
      DisableECM(wounded);
      DisableECCM(wounded);

      if (StealthArmorActive(wounded)) {
        mech_notify(wounded, MECHALL, "Your stealth armor system shuts down!");
        DisableStealthArmor(wounded);
      }

      break;
    case ANGELECM:
      MechCritStatus(wounded) |= ANGEL_ECM_DESTROYED;
      mech_notify(wounded, MECHALL,
                  "Your Angel ECM system has been destroyed!");
      DisableAngelECM(wounded);
      DisableAngelECCM(wounded);

      break;
    case BEAGLE_PROBE:
      MechCritStatus(wounded) |= BEAGLE_DESTROYED;
      MechSpecials(wounded) &= ~BEAGLE_PROBE_TECH;
      mech_notify(wounded, MECHALL,
                  "Your Beagle Active Probe has been destroyed!");
      if (((sensors[(short)MechSensor(wounded)[0]].required_special ==
            BEAGLE_PROBE_TECH) &&
           (sensors[(short)MechSensor(wounded)[0]].specials_set == 1)) ||
          ((sensors[(short)MechSensor(wounded)[1]].required_special ==
            BEAGLE_PROBE_TECH) &&
           (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))) {

        if ((sensors[(short)MechSensor(wounded)[0]].required_special ==
             BEAGLE_PROBE_TECH) &&
            (sensors[(short)MechSensor(wounded)[0]].specials_set == 1))
          MechSensor(wounded)[0] = 0;

        if ((sensors[(short)MechSensor(wounded)[1]].required_special ==
             BEAGLE_PROBE_TECH) &&
            (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))
          MechSensor(wounded)[1] = 0;

        MarkForLOSUpdate(wounded);
      }
      break;
    case BLOODHOUND_PROBE:
      MechCritStatus(wounded) |= BLOODHOUND_DESTROYED;
      MechSpecials2(wounded) &= ~BLOODHOUND_PROBE_TECH;
      mech_notify(wounded, MECHALL,
                  "Your Bloodhound Probe has been destroyed!");

      if (((sensors[(short)MechSensor(wounded)[0]].required_special ==
            BLOODHOUND_PROBE_TECH) &&
           (sensors[(short)MechSensor(wounded)[0]].specials_set == 1)) ||
          ((sensors[(short)MechSensor(wounded)[1]].required_special ==
            BLOODHOUND_PROBE_TECH) &&
           (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))) {

        if ((sensors[(short)MechSensor(wounded)[0]].required_special ==
             BLOODHOUND_PROBE_TECH) &&
            (sensors[(short)MechSensor(wounded)[0]].specials_set == 1))
          MechSensor(wounded)[0] = 0;

        if ((sensors[(short)MechSensor(wounded)[1]].required_special ==
             BLOODHOUND_PROBE_TECH) &&
            (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))
          MechSensor(wounded)[1] = 0;

        MarkForLOSUpdate(wounded);
      }
      break;
    case LIGHT_BAP:
      MechCritStatus2(wounded) |= LIGHT_BAP_DESTROYED;
      MechSpecials(wounded) &= ~LIGHT_BAP_TECH;
      mech_notify(wounded, MECHALL,
                  "Your Light Beagle Active Probe has been destroyed!");

      if (((sensors[(short)MechSensor(wounded)[0]].required_special ==
            LIGHT_BAP_TECH) &&
           (sensors[(short)MechSensor(wounded)[0]].specials_set == 1)) ||
          ((sensors[(short)MechSensor(wounded)[1]].required_special ==
            LIGHT_BAP_TECH) &&
           (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))) {

        if ((sensors[(short)MechSensor(wounded)[0]].required_special ==
             LIGHT_BAP_TECH) &&
            (sensors[(short)MechSensor(wounded)[0]].specials_set == 1))
          MechSensor(wounded)[0] = 0;

        if ((sensors[(short)MechSensor(wounded)[1]].required_special ==
             LIGHT_BAP_TECH) &&
            (sensors[(short)MechSensor(wounded)[1]].specials_set == 1))
          MechSensor(wounded)[1] = 0;

        MarkForLOSUpdate(wounded);
      }
      break;
    case ARTEMIS_IV:
      weapon_slot = GetPartData(wounded, hitloc, critHit);
      if (weapon_slot > NUM_CRITICALS) {
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Artemis IV error on mech %ld", wounded->mynum));
        break;
      }
      GetPartAmmoMode(wounded, hitloc, weapon_slot) &= ~ARTEMIS_MODE;
      mech_notify(wounded, MECHALL,
                  "Your Artemis IV system has been destroyed!");
      break;
    case AXE:
      mech_notify(wounded, MECHALL, "Your axe has been destroyed!");
      break;
    case SWORD:
      mech_notify(wounded, MECHALL, "Your sword has been destroyed!");
      break;
    case DUAL_SAW:
      mech_notify(wounded, MECHALL, "Your dual saw has been destroyed!");
      break;
    case MACE:
      mech_notify(wounded, MECHALL, "Your mace has been destroyed!");
      break;
    case DS_AERODOOR:
      mech_notify(wounded, MECHALL,
                  "One of the aero doors has been rendered useless!");
      break;
    case DS_MECHDOOR:
      mech_notify(wounded, MECHALL,
                  "One of the 'mech doors has been rendered useless!");
      [[fallthrough]];
    case NULL_SIGNATURE_SYSTEM:
      mech_notify(wounded, MECHALL,
                  "Your Null Signature System has been destroyed!");

      if (NullSigSysActive(wounded)) {
        mech_notify(wounded, MECHALL, "Your Null Signature System shuts down!");
        DisableNullSigSys(wounded);
      }

      DestroyNullSigSys(wounded);

      break;
    }

    if (destroycrit)
      DestroyPart(wounded, hitloc, critHit);
  }

  return 1;
}
