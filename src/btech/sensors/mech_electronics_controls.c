#include "mech_advanced_internal.h"
#include "mech_identity_api.h"

void mech_ecm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech_context(mech), MechCritStatus(mech) & ECM_DESTROYED,
                  "Your Guardian ECM has been destroyed already!");
  TOGGLE_SPECIALS_MACRO_CHECK(ECM_TECH, ECM_ENABLED, ECCM_ENABLED,
                              "You turn your ECM suite online (ECM mode).",
                              "You turn your ECM suite offline.",
                              "This unit isn't equipped with an ECM suite!");
  MarkForLOSUpdate(mech);
}

void mech_eccm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech_context(mech), MechCritStatus(mech) & ECM_DESTROYED,
                  "Your Guardian ECM has been destroyed already!");
  TOGGLE_SPECIALS_MACRO_CHECK(ECM_TECH, ECCM_ENABLED, ECM_ENABLED,
                              "You turn your ECM suite online (ECCM mode).",
                              "You turn your ECM suite offline.",
                              "This unit isn't equipped with an ECM suite!");
  MarkForLOSUpdate(mech);
}

void mech_perecm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  TOGGLE_INFANTRY_MACRO_CHECK(
      FC_INFILTRATORII_STEALTH_TECH, PER_ECM_ENABLED, PER_ECCM_ENABLED,
      "You turn your Personal ECM suite online (ECM mode).",
      "You turn your Personal ECM suite offline.",
      "This unit isn't equipped with a Personal ECM suite!");
  MarkForLOSUpdate(mech);
}

void mech_pereccm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  TOGGLE_INFANTRY_MACRO_CHECK(
      FC_INFILTRATORII_STEALTH_TECH, PER_ECCM_ENABLED, PER_ECM_ENABLED,
      "You turn your Personal ECM suite online (ECCM mode).",
      "You turn your Personal ECM suite offline.",
      "This unit isn't equipped with a Personal ECM suite!");
  MarkForLOSUpdate(mech);
}

void mech_angelecm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech_context(mech),
                  MechCritStatus(mech) & ANGEL_ECM_DESTROYED,
                  "Your Angel ECM has been destroyed already!");
  TOGGLE_SPECIALS_MACRO_CHECK2(
      ANGEL_ECM_TECH, ANGEL_ECM_ENABLED, ANGEL_ECCM_ENABLED,
      "You turn your Angel ECM suite online (ECM mode).",
      "You turn your Angel ECM suite offline.",
      "This unit isn't equipped with an Angel ECM suite!");
  MarkForLOSUpdate(mech);
}

void mech_angeleccm(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech_context(mech),
                  MechCritStatus(mech) & ANGEL_ECM_DESTROYED,
                  "Your Angel ECM has been destroyed already!");
  TOGGLE_SPECIALS_MACRO_CHECK2(
      ANGEL_ECM_TECH, ANGEL_ECCM_ENABLED, ANGEL_ECM_ENABLED,
      "You turn your Angel ECM suite online (ECCM mode).",
      "You turn your Angel ECM suite offline.",
      "This unit isn't equipped with an Angel ECM suite!");
  MarkForLOSUpdate(mech);
}

void MechSliteChangeEvent(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long wType = (long)e->data2;

  if (MechCritStatus(mech) & SLITE_DEST)
    return;

  if (!Started(mech))
    return;

  if (!Started(mech)) {
    MechStatus2(mech) &= ~SLITE_ON;
    MechCritStatus(mech) &= ~SLITE_LIT;
    return;
  }

  if (wType == 1) {
    MechStatus2(mech) |= SLITE_ON;
    MechCritStatus(mech) |= SLITE_LIT;

    mech_notify(mech, MECHALL, "Your searchlight comes on to full power.");
    MechLOSBroadcast(mech, "turns on a searchlight!");
  } else {
    MechStatus2(mech) &= ~SLITE_ON;
    MechCritStatus(mech) &= ~SLITE_LIT;

    mech_notify(mech, MECHALL, "Your searchlight shuts off.");
    MechLOSBroadcast(mech, "turns off a searchlight!");
  }

  MarkForLOSUpdate(mech);
}

void mech_slite(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  if (!(MechSpecials(mech) & SLITE_TECH)) {
    mech_notify(mech, MECHALL, "Your 'mech isn't equipped with searchlight!");
    return;
  }

  DOCHECK_CONTEXT(mech_context(mech), MechCritStatus(mech) & SLITE_DEST,
                  "Your searchlight has been destroyed already!");

  if (mech_event_count(mech, EVENT_SLITECHANGING)) {
    if (MechStatus2(mech) & SLITE_ON)
      mech_notify(mech, MECHALL,
                  "Your searchlight is already in the process of turning off.");
    else
      mech_notify(mech, MECHALL,
                  "Your searchlight is already in the process of turning on.");

    return;
  }

  if (MechStatus2(mech) & SLITE_ON) {
    mech_notify(mech, MECHALL, "Your searchlight starts to cool down.");
    mech_event_schedule(mech, EVENT_SLITECHANGING, MechSliteChangeEvent, 5, 0);
  } else {
    mech_notify(mech, MECHALL, "Your searchlight starts to warm up.");
    mech_event_schedule(mech, EVENT_SLITECHANGING, MechSliteChangeEvent, 5, 1);
  }
}

void changeStealthArmorEvent(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long wType = (long)e->data2;

  if (!Started(mech))
    return;

  if (!HasWorkingECMSuite(mech))
    return;

  if (wType) {
    mech_notify(mech, MECHALL, "Stealth Armor system engaged!");

    EnableStealthArmor(mech);
    checkECM(mech);
    MarkForLOSUpdate(mech);
  } else {
    mech_notify(mech, MECHALL, "Stealth Armor system disengaged!");

    DisableStealthArmor(mech);
    checkECM(mech);
    MarkForLOSUpdate(mech);
  }
}

void mech_stealtharmor(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  if (!(MechSpecials2(mech) & STEALTH_ARMOR_TECH)) {
    mech_notify(mech, MECHALL,
                "Your 'mech isn't equipped with a Stealth Armor system!");

    return;
  }

  if (!HasWorkingECMSuite(mech)) {
    mech_notify(mech, MECHALL,
                "Your 'mech doesn't have a working Guardian ECM suite!");

    return;
  }

  if (mech_event_count(mech, EVENT_STEALTH_ARMOR)) {
    mech_notify(
        mech, MECHALL,
        "You are already changing the status of your Stealth Armor system!");

    return;
  }

  if (!StealthArmorActive(mech)) {
    mech_notify(mech, MECHALL,
                "Your Stealth Armor system begins to come online.");

    mech_event_schedule(mech, EVENT_STEALTH_ARMOR, changeStealthArmorEvent, 30,
                        1);
  } else {
    mech_notify(mech, MECHALL, "Your Stealth Armor system begins to shutdown.");

    mech_event_schedule(mech, EVENT_STEALTH_ARMOR, changeStealthArmorEvent, 30,
                        0);
  }
}

void changeNullSigSysEvent(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long wType = (long)e->data2;

  if (!Started(mech))
    return;

  if (NullSigSysDest(mech))
    return;

  if (wType) {
    mech_notify(mech, MECHALL, "Null Signature System engaged!");

    EnableNullSigSys(mech);
    MarkForLOSUpdate(mech);
  } else {
    mech_notify(mech, MECHALL, "Null Signature System disengaged!");

    DisableNullSigSys(mech);
    MarkForLOSUpdate(mech);
  }
}

void mech_nullsig(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  if (!(MechSpecials2(mech) & NULLSIGSYS_TECH)) {
    mech_notify(mech, MECHALL,
                "Your 'mech isn't equipped with a Null Signature System!");

    return;
  }

  if (NullSigSysDest(mech)) {
    mech_notify(mech, MECHALL, "Your Null Signature System is destroyed!");

    return;
  }

  if (mech_event_count(mech, EVENT_NSS)) {
    mech_notify(
        mech, MECHALL,
        "You are already changing the status of your Null Signature System!");

    return;
  }

  if (!NullSigSysActive(mech)) {
    mech_notify(mech, MECHALL,
                "Your Null Signature System begins to come online.");

    mech_event_schedule(mech, EVENT_NSS, changeNullSigSysEvent, 30, 1);
  } else {
    mech_notify(mech, MECHALL,
                "Your Null Signature System begins to shutdown.");

    mech_event_schedule(mech, EVENT_NSS, changeNullSigSysEvent, 30, 0);
  }
}

void show_narc_pods(DbRef player, Mech *mech, char *buffer) {
  char location[50];
  int i;

  cch(MECH_USUALO);

  if (!(checkAllSections(mech, NARC_ATTACHED) ||
        checkAllSections(mech, INARC_HOMING_ATTACHED) ||
        checkAllSections(mech, INARC_HAYWIRE_ATTACHED) ||
        checkAllSections(mech, INARC_ECM_ATTACHED) ||
        checkAllSections(mech, INARC_NEMESIS_ATTACHED))) {

    notify(btech_context_evaluation(mech_context(mech)), player,
           "There are no NARC or iNARC pods attached to this unit.");

    return;
  }

  notify(btech_context_evaluation(mech_context(mech)), player,
         "=========================Attached NARC and iNARC "
         "Pods========================");
  notify(btech_context_evaluation(mech_context(mech)), player,
         "-- Location ---||- NARC -||- iHoming -||- iHaywire -||- iECM "
         "-||- iNemesis --");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (GetSectOInt(mech, i) > 0) {
      ArmorStringFromIndex(i, location, MechType(mech), MechMove(mech));

      if (SectIsDestroyed(mech, i)) {
        notify_printf(btech_context_evaluation(mech_context(mech)), player,
                      " %-14.13s||********||***********||************||********"
                      "||************* ",
                      location);
      } else {
        notify_printf(
            btech_context_evaluation(mech_context(mech)), player,
            " %-14.13s||....%s...||.....%s.....||......%s.....||....%s...||...."
            "..%s...... ",
            location,
            checkSectionForSpecial(mech, NARC_ATTACHED, i) ? "X" : ".",
            checkSectionForSpecial(mech, INARC_HOMING_ATTACHED, i) ? "X" : ".",
            checkSectionForSpecial(mech, INARC_HAYWIRE_ATTACHED, i) ? "X" : ".",
            checkSectionForSpecial(mech, INARC_ECM_ATTACHED, i) ? "X" : ".",
            checkSectionForSpecial(mech, INARC_NEMESIS_ATTACHED, i) ? "X"
                                                                    : ".");
      }
    }
  }
}

int findArmBTHMod(Mech *mech, int wSec) {
  int wRet = 0;

  if (PartIsNonfunctional(mech, wSec, 1) ||
      GetPartType(mech, wSec, 1) != I2Special(UPPER_ACTUATOR))
    wRet += 2;
  if (PartIsNonfunctional(mech, wSec, 2) ||
      GetPartType(mech, wSec, 2) != I2Special(LOWER_ACTUATOR))
    wRet += 2;
  if (PartIsNonfunctional(mech, wSec, 3) ||
      GetPartType(mech, wSec, 3) != I2Special(HAND_OR_FOOT_ACTUATOR))
    wRet += 1;

  return wRet;
}

void remove_inarc_pods_mech(DbRef player, Mech *mech, char *buffer) {
  int wLoc;
  int wArmToUse = -1;
  char *args[2];
  char strLocation[50], strPunchWith[50];
  int wBTH = 0;
  int wBTHModLARM = 0;
  int wBTHModRARM = 0;
  int wRAAvail = 1;
  int wLAAvail = 1;
  int wRoll;
  int wSelfDamage;
  int wPodType = INARC_HOMING_ATTACHED;
  char strPodType[30];

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), MechIsQuad(mech),
                  "Quads can not knock of iNARC pods!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_parseattributes(buffer, args, 2) != 2,
                  "Invalid number of arguments!");

  wLoc = ArmorSectionFromString(MechType(mech), MechMove(mech), args[0]);

  DOCHECK_CONTEXT(mech_context(mech), wLoc == -1, "Invalid section!");
  DOCHECK_CONTEXT(mech_context(mech), !GetSectOInt(mech, wLoc),
                  "Invalid section!");
  DOCHECK_CONTEXT(mech_context(mech), !GetSectInt(mech, wLoc),
                  "That section is destroyed!");

  ArmorStringFromIndex(wLoc, strLocation, MechType(mech), MechMove(mech));

  /* Figure out wot type of pods we want to remove */
  switch (toupper(args[1][0])) {
  case 'Y':
    strcpy(strPodType, "Haywire");
    wPodType = INARC_HAYWIRE_ATTACHED;
    break;

  case 'E':
    strcpy(strPodType, "ECM");
    wPodType = INARC_ECM_ATTACHED;
    break;

  default:
    strcpy(strPodType, "Homing");
    wPodType = INARC_HOMING_ATTACHED;
    break;
  }

  DOCHECK_CONTEXT(mech_context(mech),
                  !checkSectionForSpecial(mech, wPodType, wLoc),
                  tprintf("There are no iNarc %s pods attached to your %s!",
                          strPodType, strLocation));

  DOCHECK_CONTEXT(
      mech_context(mech),
      ((!GetSectInt(mech, RARM)) && (!GetSectInt(mech, LARM))),
      "You need at least one functioning arm to remove iNarc pods!");

  if (wLoc == RARM) {
    DOCHECK_CONTEXT(mech_context(mech), !GetSectInt(mech, LARM),
                    "Your Left Arm needs to be intact to take "
                    "iNarc pods off your right arm!");
    DOCHECK_CONTEXT(mech_context(mech), SectHasBusyWeap(mech, LARM),
                    "You have weapons recycling on your Left Arm.");
    DOCHECK_CONTEXT(mech_context(mech), MechSections(mech)[LARM].recycle,
                    "Your Left Arm is still recovering from your last attack.");

    wArmToUse = LARM;
  }

  if (wLoc == LARM) {
    DOCHECK_CONTEXT(mech_context(mech), !GetSectInt(mech, RARM),
                    "Your Right Arm needs to be intact to "
                    "take iNarc pods off your Left Arm!");
    DOCHECK_CONTEXT(mech_context(mech), SectHasBusyWeap(mech, RARM),
                    "You have weapons recycling on your Right Arm.");
    DOCHECK_CONTEXT(
        mech_context(mech), MechSections(mech)[RARM].recycle,
        "Your Right Arm is still recovering from your last attack.");

    wArmToUse = RARM;
  }

  if (wArmToUse == -1) {
    if (SectHasBusyWeap(mech, RARM) || MechSections(mech)[RARM].recycle ||
        (!GetSectInt(mech, RARM)))
      wRAAvail = 0;

    if (SectHasBusyWeap(mech, LARM) || MechSections(mech)[LARM].recycle ||
        (!GetSectInt(mech, LARM)))
      wLAAvail = 0;

    DOCHECK_CONTEXT(
        mech_context(mech), !(wLAAvail || wRAAvail),
        "You need at least one arm that is not recycling and does not have "
        "weapons recycling in it!");

    if (!wLAAvail)
      wBTHModLARM = 1000;
    else
      wBTHModLARM = findArmBTHMod(mech, LARM);

    if (!wRAAvail)
      wBTHModRARM = 1000;
    else
      wBTHModRARM = findArmBTHMod(mech, RARM);

    if (wBTHModRARM < wBTHModLARM) {
      wBTH = wBTHModRARM;
      wArmToUse = RARM;
    } else {
      wBTH = wBTHModLARM;
      wArmToUse = LARM;
    }
  } else {
    wBTH = findArmBTHMod(mech, wArmToUse);
  }

  wBTH += FindPilotPiloting(mech) + 4;
  wRoll = btech_random_roll(mech_context(mech));

  ArmorStringFromIndex(wArmToUse, strPunchWith, MechType(mech), MechMove(mech));

  mech_printf(mech, MECHALL,
              "You try to swat at the iNarc pods attached to your %s with your "
              "%s.  BTH:  %d,\tRoll:  %d",
              strLocation, strPunchWith, wBTH, wRoll);

  /* Oops, we failed! */
  if (wRoll < wBTH) {
    mech_notify(mech, MECHALL, "Uh oh. You miss the pod and hit yourself!");
    MechLOSBroadcast(
        mech, "tries to swat off an iNarc pod, but misses and hits itself!");

    wSelfDamage = (MechTons(mech) + 10 / 2) / 10;

    if (!OkayCritSectS(wArmToUse, 2, LOWER_ACTUATOR))
      wSelfDamage = wSelfDamage / 2;

    if (!OkayCritSectS(wArmToUse, 1, UPPER_ACTUATOR))
      wSelfDamage = wSelfDamage / 2;

    DamageMech(mech, mech, 1, MechPilot(mech), wLoc, 0, 0, wSelfDamage, 0, -1,
               0, -1, 0, 0);
  } else {
    MechSections(mech)[wLoc].specials &= ~wPodType;

    mech_printf(mech, MECHALL, "You knock a %s pod off your %s!", strPodType,
                strLocation);
    MechLOSBroadcast(mech, "knocks an iNarc pod off itself.");
  }

  mech_set_recycle_limb(mech, wArmToUse, PHYSICAL_RECYCLE_TIME);
}

void removeiNarcPodsTank(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int i;

  if (Destroyed(mech))
    return;

  mech_notify(mech, MECHALL, "You remove all the iNARC pods from your unit.");

  MechLOSBroadcast(
      mech, "'s crew climbs out and knocks off all the attached iNarc pods!");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (GetSectOInt(mech, i) > 0) {
      MechSections(mech)[i].specials &=
          ~(INARC_HOMING_ATTACHED | INARC_HAYWIRE_ATTACHED |
            INARC_ECM_ATTACHED | INARC_NEMESIS_ATTACHED);
    }
  }
}

void remove_inarc_pods_tank(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALSO);

  DOCHECK_CONTEXT(
      mech_context(mech), (MechDesiredSpeed(mech) > 0),
      "You can not be moving when attempting to remove iNarc pods!");
  DOCHECK_CONTEXT(
      mech_context(mech), (MechSpeed(mech) > 0),
      "You can not be moving when attempting to remove iNarc pods!");

  if (MechType(mech) == CLASS_VTOL)
    DOCHECK_CONTEXT(mech_context(mech), !Landed(mech),
                    "You must land before attempting to remove iNarc pods!");

  DOCHECK_CONTEXT(mech_context(mech), mech_event_count(mech, EVENT_UNSTUN_CREW),
                  "You're too stunned to remove iNarc pods!");
  DOCHECK_CONTEXT(
      mech_context(mech), mech_event_count(mech, EVENT_UNJAM_TURRET),
      "You're too busy unjamming your turret to remove iNarc pods!");
  DOCHECK_CONTEXT(mech_context(mech), mech_event_count(mech, EVENT_UNJAM_AMMO),
                  "You're too busy unjamming a weapon to remove iNarc pods!");

  if (!(checkAllSections(mech, INARC_HOMING_ATTACHED) ||
        checkAllSections(mech, INARC_HAYWIRE_ATTACHED) ||
        checkAllSections(mech, INARC_ECM_ATTACHED) ||
        checkAllSections(mech, INARC_NEMESIS_ATTACHED))) {

    mech_notify(mech, MECHALL,
                "There are no iNarc pods attached to this unit.");

    return;
  }

  mech_notify(
      mech, MECHALL,
      "You begin to systematically remove all the iNarc pods from your unit.");

  mech_event_schedule(mech, EVENT_REMOVE_PODS, removeiNarcPodsTank, 60, 0);
}
