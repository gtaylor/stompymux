/* Environmental and terrain-driven unit damage. */

#include "environment_damage_api.h"

#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_events.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_sensor_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
void reactor_explosion(Mech *wounded, Mech *attacker) {
  int z = MechZ(wounded);
  BattleMap *map =
      btech_context_get_map(wounded->xcode.context, wounded->mapindex);
  DbRef wounded_pilot = MechPilot(wounded);
  int dam;

  DestroySection(wounded, attacker, 0, CTORSO);
  DestroySection(wounded, attacker, 0, LTORSO);
  DestroySection(wounded, attacker, 0, RTORSO);
  DestroySection(wounded, attacker, 0, LLEG);
  DestroySection(wounded, attacker, 0, RLEG);

  /* Need to autoeject before the explosion reaches the head */
  if (!MapIsUnderground(map))
    autoeject(wounded_pilot, wounded, 0);

  DestroySection(wounded, attacker, 0, HEAD);
  MechZ(wounded) += 6;
  dam = MAX(MechTons(wounded) / 5, MechEngineSize(wounded) / 10);

  ScrambleInfraAndLiteAmp(
      wounded, 4, 0, "The searing blast of heat burns out your sensors!",
      "The blinding flash of light overloads your sensors!");

  blast_hit_hexesf(
      map, dam, 3, MAX(MechTons(wounded) / 10, MechEngineSize(wounded) / 25),
      MechFX(wounded), MechFY(wounded), MechFX(wounded), MechFY(wounded),
      "[fg=red bold]You bear full brunt of the blast![reset]",
      "is hit badly by the blast!",
      "[fg=yellow bold]You receive some damage from the blast![reset]",
      "is hit by the blast!",
      wounded->xcode.context->configuration->btech_explode_reactor > 1, 3, 5, 1,
      2);
  MechZ(wounded) = z;
  headhitmwdamage(wounded, attacker, 4);
}

void mech_parts_destroy(Mech *attacker, Mech *wounded, int hitloc, int breach,
                        int is_disable) {
  float oldjs;
  int i;
  int critType;
  int nhs = 0;
  int tDoAutoFall = 0;
  int tIsLeg = ((hitloc == RLEG || hitloc == LLEG) ||
                ((hitloc == RARM || hitloc == LARM) && (MechIsQuad(wounded))));

  if (!(MechType(wounded) == CLASS_MECH || MechType(wounded) == CLASS_MW ||
        MechType(wounded) == CLASS_BSUIT)) {
    for (i = 0; i < CritsInLoc(wounded, hitloc); i++)
      if (GetPartType(wounded, hitloc, i) &&
          !PartIsDestroyed(wounded, hitloc, i)) {
        if (is_disable == 1)
          DisablePart(wounded, hitloc, i);
        else
          DestroyPart(wounded, hitloc, i);
      }
    return;
  }
  oldjs = MechJumpSpeed(wounded);
  for (i = 0; i < CritsInLoc(wounded, hitloc); i++)
    if (!PartIsDestroyed(wounded, hitloc, i)) {
      if (is_disable == 1)
        DisablePart(wounded, hitloc, i);
      else if (PartIsDisabled(wounded, hitloc, i)) {
        DestroyPart(wounded, hitloc, i);
        continue;
      } else
        DestroyPart(wounded, hitloc, i);

      critType = GetPartType(wounded, hitloc, i);
      if (IsAmmo(critType)) {
        GetPartData(wounded, hitloc, i) = 0;
      }
      if ((IsSpecial(critType))) {
        switch (Special2I(critType)) {
        case UPPER_ACTUATOR:
        case LOWER_ACTUATOR:
        case HAND_OR_FOOT_ACTUATOR:
          break;
        case SHOULDER_OR_HIP:
          if (tIsLeg) {
            if (!(MechCritStatus(wounded) & HIP_DAMAGED)) {
              MechCritStatus(wounded) |= HIP_DAMAGED;
            } else {
              if (!MechIsQuad(wounded))
                MechCritStatus(wounded) |= HIP_DESTROYED;
            }
          }
          break;
        case HEAT_SINK:
          if (MechSpecials(wounded) & DOUBLE_HEAT_TECH) {
            if ((nhs++) % 3 == 2)
              MechRealNumsinks(wounded)++;
          }
          MechRealNumsinks(wounded)--;
          break;
        case JUMP_JET:
          MechJumpSpeed(wounded) -= MP1;
          if (MechJumpSpeed(wounded) < 0)
            MechJumpSpeed(wounded) = 0;
          if (attacker && MechJumpSpeed(wounded) == 0 && Jumping(wounded)) {
            mech_notify(wounded, MECHALL,
                        "Losing your last Jump Jet you fall from the sky!!!!!");
            MechLOSBroadcast(wounded, "falls from the sky!");
            MechFalls(wounded, (int)(oldjs * MP_PER_KPH), 0);
            domino_space(wounded, 2);
          }
          break;
        case ENGINE:
          if (MechEngineHeat(wounded) < 10)
            MechEngineHeat(wounded) += 5;
          else if (MechEngineHeat(wounded) < 15) {
            MechEngineHeat(wounded) = 15;
            if (attacker) {
              mech_notify(wounded, MECHALL, "Your engine is destroyed!!");
              if (wounded != attacker)
                mech_notify(attacker, MECHALL, "You destroy the engine!!");
            }
            // check_stackpole(wounded, attacker);

            if (wounded->xcode.context->configuration->btech_stackpole &&
                (MechBoomStart(wounded) + MAX_BOOM_TIME) >=
                    wounded->xcode.context->events->tick &&
                btech_random_roll(wounded->xcode.context) >= BOOM_BTH &&
                (Started(wounded) ||
                 mech_event_count(wounded, EVENT_STARTUP))) {

              HexLOSBroadcast(
                  btech_context_get_map(wounded->xcode.context,
                                        wounded->mapindex),
                  MechX(wounded), MechY(wounded),
                  "[fg=red bold]The hit destroys the last safety systems, "
                  "releasing the fusion reaction![reset]");

              reactor_explosion(wounded, attacker);
            }

            if ((MechType(wounded) == CLASS_MECH) &&
                (hitloc == LTORSO || hitloc == RTORSO) &&
                (MechSpecials(wounded) & XL_TECH))
              DestroyMech(wounded, attacker, 1,
                          (wounded == attacker) ? KILL_TYPE_SELF_DESTRUCT
                                                : KILL_TYPE_XLENGINE);
            else
              DestroyMech(wounded, attacker, 1,
                          (wounded == attacker) ? KILL_TYPE_SELF_DESTRUCT
                                                : KILL_TYPE_NORMAL);
          }
          break;
        case ECM:
          if (!(MechCritStatus(wounded) & ECM_DESTROYED)) {
            MechCritStatus(wounded) |= ECM_DESTROYED;
            mech_notify(wounded, MECHALL,
                        "Your ECM system has been destroyed!");
            DisableECM(wounded);
            DisableECCM(wounded);
            checkECM(wounded);
          }
          break;
        case TARGETING_COMPUTER:
          if (!(MechCritStatus(wounded) & TC_DESTROYED)) {
            if (attacker)
              mech_notify(wounded, MECHALL,
                          "Your Targeting Computer is Destroyed");
            MechCritStatus(wounded) |= TC_DESTROYED;
          }
          break;
        }
      }
    }
  if (breach)
    if (MechType(wounded) == CLASS_VEH_GROUND ||
        MechType(wounded) == CLASS_VEH_NAVAL)
      DestroyMech(wounded, attacker, 0, KILL_TYPE_NORMAL);
  if (MechType(wounded) == CLASS_MECH || MechType(wounded) == CLASS_MW) {
    if (breach && hitloc == HEAD) {
      if (InVacuum(wounded))
        mech_notify(wounded, MECHALL, "You are exposed to vacuum!");
      else
        mech_notify(wounded, MECHALL, "Water floods into your cockpit!");

      KillMechContentsIfIC(wounded);
      DestroyMech(wounded, attacker, 0, KILL_TYPE_FLOOD);
      return;
    }
    if (!MechIsQuad(wounded))
      if (hitloc == LARM || hitloc == RARM)
        return;
    if (hitloc == RLEG || hitloc == LLEG || hitloc == LARM || hitloc == RARM) {
      tDoAutoFall = 1;
      mech_event_cancel(wounded, EVENT_STAND);
    }
    NormalizeAllActuatorCrits(wounded);
    if (tIsLeg && !Fallen(wounded) && !Jumping(wounded) && !OODing(wounded) &&
        attacker) {
      if (tDoAutoFall) {
        mech_notify(wounded, MECHALL,
                    "You realize remaining standing is no longer an option and "
                    "crash to the ground!");
        MechLOSBroadcast(wounded, "crashes to the ground!");
        MechFalls(wounded, 1, 0);
      } else if (!MadePilotSkillRoll(wounded, 0)) {
        mech_notify(wounded, MECHALL, "You lose your balance and fall down!");
        MechLOSBroadcast(wounded, "loses balance and falls down!");
        MechFalls(wounded, 1, 0);
      }
    }
  }
}

int mech_location_breach(Mech *attacker, Mech *mech, int hitloc) {
  char buf[SBUF_SIZE];

  if (!InSpecial(mech))
    return 0;
  if (!InVacuum(mech))
    return 0;
  if (SectIsDestroyed(mech, hitloc) || SectIsBreached(mech, hitloc))
    return 0;
  ArmorStringFromIndex(hitloc, buf, MechType(mech), MechMove(mech));
  mech_notify(mech, MECHALL, tprintf("Your %s has been breached!", buf));
  SetSectBreached(mech, hitloc);
  mech_parts_destroy(attacker, mech, hitloc, 1, 1);
  return 1;
}

int mech_location_maybe_breach(Mech *attacker, Mech *mech, int hitloc) {
  if (!InSpecial(mech))
    return 0;
  if (btech_random_roll(mech->xcode.context) < 10)
    return 0;
  return mech_location_breach(attacker, mech, hitloc);
}
