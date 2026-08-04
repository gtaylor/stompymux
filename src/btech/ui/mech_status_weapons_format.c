#include "mech_status_internal.h"

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

  if ((MechSpecials(mech) & ECM_TECH) ||
      (MechSpecials2(mech) & STEALTH_ARMOR_TECH) ||
      (MechSpecials2(mech) & NULLSIGSYS_TECH) ||
      (MechSpecials(mech) & SLITE_TECH) || HasC3(mech) || HasC3i(mech) ||
      (MechSpecials(mech) & MASC_TECH) ||
      (MechSpecials2(mech) & SUPERCHARGER_TECH) ||
      (MechSpecials(mech) & TRIPLE_MYOMER_TECH) ||
      (MechSpecials2(mech) & ANGEL_ECM_TECH) || HasTAG(mech) ||
      (MechInfantrySpecials(mech) & FC_INFILTRATORII_STEALTH_TECH)) {
    strcpy(tempbuff, "AdvTech: ");

    if (MechSpecials(mech) & ECM_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "ECM(%s)  ",
               (MechCritStatus(mech) & ECM_DESTROYED) ? "[fg=red bold]XX[reset]"
               : ECMEnabled(mech)
                   ? (ECMActive(mech) ? "[fg=green bold]ECM[reset]"
                                      : "[fg=red bold]ECM[reset]")
               : ECCMEnabled(mech)  ? "[fg=green bold]ECCM[reset]"
               : ECMCountered(mech) ? "[fg=red]Off[reset]"
                                    : "[fg=green]Off[reset]");
    }

    if (MechSpecials2(mech) & ANGEL_ECM_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "AngelECM(%s)  ",
               (!HasWorkingAngelECMSuite(mech)) ? "[fg=red bold]XX[reset]"
               : AngelECMEnabled(mech)
                   ? (AngelECMActive(mech) ? "[fg=green bold]ECM[reset]"
                                           : "[fg=red bold]ECM[reset]")
               : AngelECCMEnabled(mech) ? "[fg=green bold]ECCM[reset]"
               : ECMCountered(mech)     ? "[fg=red]Off[reset]"
                                        : "[fg=green]Off[reset]");
    }

    if (MechInfantrySpecials(mech) & FC_INFILTRATORII_STEALTH_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "PersonalECM(%s)  ",
               PerECMEnabled(mech)
                   ? (PerECMActive(mech) ? "[fg=green bold]ECM[reset]"
                                         : "[fg=red bold]ECM[reset]")
               : PerECCMEnabled(mech) ? "[fg=green bold]ECCM[reset]"
               : ECMCountered(mech)   ? "[fg=red]Off[reset]"
                                      : "[fg=green]Off[reset]");
    }

    if (MechSpecials2(mech) & STEALTH_ARMOR_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SthArmor(%s)  ",
               (MechCritStatus(mech) & ECM_DESTROYED) ? "[fg=red bold]XX[reset]"
               : StealthArmorActive(mech) ? "[fg=green bold]On[reset]"
                                          : "[fg=green]Rdy[reset]");
    }

    if (MechSpecials2(mech) & NULLSIGSYS_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "NullSigSys(%s)  ",
               NullSigSysDest(mech)     ? "[fg=red bold]XX[reset]"
               : NullSigSysActive(mech) ? "[fg=green bold]On[reset]"
                                        : "[fg=green]Rdy[reset]");
    }

    if (MechSpecials(mech) & SLITE_TECH) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SLITE(%s)  ",
               (MechCritStatus(mech) & SLITE_DEST) ? "[fg=red bold]XX[reset]"
               : (MechStatus2(mech) & SLITE_ON)    ? "[fg=green bold]On[reset]"
                                                   : "[fg=green]Off[reset]");
    }

    if (HasC3m(mech))
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3M[reset]  ",
               C3Destroyed(mech)             ? "[fg=red]"
               : AnyECMDisturbed(mech)       ? "[fg=yellow]"
               : MechC3NetworkSize(mech) > 0 ? "[fg=green bold]"
                                             : "[fg=green]");

    if (HasC3s(mech))
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3S[reset]  ",
               C3Destroyed(mech)             ? "[fg=red]"
               : AnyECMDisturbed(mech)       ? "[fg=yellow]"
               : MechC3NetworkSize(mech) > 0 ? "[fg=green bold]"
                                             : "[fg=green]");

    if (HasC3i(mech))
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "%sC3i[reset]  ",
               C3iDestroyed(mech)             ? "[fg=red]"
               : AnyECMDisturbed(mech)        ? "[fg=yellow]"
               : MechC3iNetworkSize(mech) > 0 ? "[fg=green bold]"
                                              : "[fg=green]");

    if (MechSpecials(mech) & TRIPLE_MYOMER_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "TSM(%s)  ",
               ((MechHeat(mech) >= 9.0) ? "[fg=green bold]On[reset]"
                                        : "[fg=green]Off[reset]"));

    if (HasTAG(mech)) {
      snprintf(
          tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
          "TAG(%s)  ",
          isTAGDestroyed(mech) ? "[fg=red bold]XX[reset]"
          : ((btech_context_get_mech(mech->xcode.context, TAGTarget(mech)) ==
              NULL) ||
             (TaggedBy(btech_context_get_mech(mech->xcode.context,
                                              TAGTarget(mech))) != mech->mynum))
              ? (mech_event_count(mech, EVENT_TAG_RECYCLE)
                     ? "[fg=yellow bold]Not Rdy[reset]"
                     : "[fg=green]Rdy[reset]")
              : tprintf("%s%s[reset]",
                        (mech_event_count(mech, EVENT_TAG_RECYCLE)
                             ? "[fg=yellow bold]"
                             : "[bold]"),
                        mech_to_mech_display_id(
                            mech, btech_context_get_mech(mech->xcode.context,
                                                         TAGTarget(mech)))
                            .text));
    }

    if (MechSpecials2(mech) & SUPERCHARGER_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "SCHARGE: %s%d[reset] (%s)",
               MechSChargeCounter(mech) > 3   ? "[fg=red bold]"
               : MechSChargeCounter(mech) > 0 ? "[fg=yellow bold]"
                                              : "[fg=green]",
               MechSChargeCounter(mech),
               MechStatus(mech) & SCHARGE_ENABLED ? "On" : "Off");

    if (MechSpecials(mech) & MASC_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "MASC: %s%d[reset] (%s)",
               MechMASCCounter(mech) > 3   ? "[fg=red bold]"
               : MechMASCCounter(mech) > 0 ? "[fg=yellow bold]"
                                           : "[fg=green]",
               MechMASCCounter(mech),
               MechStatus(mech) & MASC_ENABLED ? "On" : "Off");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (MechSpecials2(mech) & CARRIER_TECH) {
    strcpy(tempbuff, "Carrier: ");

    snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
             "%d tons free, %d tons max unit size", (CargoSpace(mech) / 100),
             CarMaxTon(mech));
    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((MechSpecials(mech) & AA_TECH) ||
      (MechSpecials(mech) & BEAGLE_PROBE_TECH) ||
      (MechSpecials2(mech) & BLOODHOUND_PROBE_TECH)) {
    /*||
    (MechSpecials(mech) & LIGHT_BAP_TECH)) { */

    strcpy(tempbuff, "AdvSensors:");

    if (MechSpecials(mech) & AA_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " Radar");

    if (MechSpecials(mech) & BEAGLE_PROBE_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BeagleProbe");

    if (MechSpecials2(mech) & BLOODHOUND_PROBE_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BloodhoundProbe");

    //		if(MechSpecials(mech) & LIGHT_BAP_TECH)
    //			snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) -
    // strleng(tempbuff), " LightBAP");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((MechInfantrySpecials(mech) & CS_PURIFIER_STEALTH_TECH) ||
      (MechInfantrySpecials(mech) & DC_KAGE_STEALTH_TECH) ||
      (MechInfantrySpecials(mech) & FWL_ACHILEUS_STEALTH_TECH) ||
      (MechInfantrySpecials(mech) & FC_INFILTRATOR_STEALTH_TECH) ||
      (MechInfantrySpecials(mech) & FC_INFILTRATORII_STEALTH_TECH)) {

    strcpy(tempbuff, "AdvItems:");

    if (MechInfantrySpecials(mech) & CS_PURIFIER_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " PurifierStealth");

    if (MechInfantrySpecials(mech) & DC_KAGE_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " KageStealth");

    if (MechInfantrySpecials(mech) & FWL_ACHILEUS_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " AchileusStealth");

    if (MechInfantrySpecials(mech) & FC_INFILTRATOR_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " InfiltratorStealth");

    if (MechInfantrySpecials(mech) & FC_INFILTRATORII_STEALTH_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " InfiltratorIIStealth");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if ((MechInfantrySpecials(mech) & INF_SWARM_TECH) ||
      (MechInfantrySpecials(mech) & INF_MOUNT_TECH) ||
      (MechInfantrySpecials(mech) & INF_ANTILEG_TECH) ||
      (MechInfantrySpecials(mech) & CAN_JETTISON_TECH)) {

    strcpy(tempbuff, "Special Actions:");

    if (MechInfantrySpecials(mech) & INF_MOUNT_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " MountFriends");

    if (MechInfantrySpecials(mech) & INF_SWARM_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " SwarmAttack");

    if (MechInfantrySpecials(mech) & INF_ANTILEG_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " AntiLegAttack");

    if (MechInfantrySpecials(mech) & CAN_JETTISON_TECH)
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               " BackPackJettison");

    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }

  if (MechInfantrySpecials(mech) & MUST_JETTISON_TECH) {
    strcpy(tempbuff, "Requirements: Must jettison backpack before using "
                     "special abilities or jumping");
    notify(evaluation, player, tempbuff);
    tempbuff[0] = 0;
  }
#define SHOWSECTSTAT(a)                                                        \
  (SectIsDestroyed(mech, a) ? "[fg=black bold]*****[reset]"                    \
   : (MechSections(mech)[(a)].recycle > 0)                                     \
       ? tprintf("%-5d", (MechSections(mech)[(a)].recycle / WEAPON_TICK) +     \
                             (MechSections(mech)[(a)].recycle % WEAPON_TICK))  \
       : "[fg=green]Ready[reset]")

  mech_update_recycling(mech);
  if (MechType(mech) == CLASS_MECH && !compact) {
    tempbuff[0] = 0;

#define SHOWPHYSTATUS(a, b)                                                    \
  (!canUsePhysical(mech, a, b) ? "[fg=red bold]XX[reset]"                      \
   : (MechSections(mech)[(a)].recycle > 0)                                     \
       ? tprintf("%-3d", (MechSections(mech)[(a)].recycle / WEAPON_TICK) +     \
                             +(MechSections(mech)[(a)].recycle % WEAPON_TICK)) \
       : "[fg=green]Rdy[reset]")

#define SHOW(part, loc)                                                        \
  snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),   \
           "%s: %s ", part, loc)

    SHOW(MechIsQuad(mech) ? "FLLEG" : "LARM", SHOWSECTSTAT(LARM));
    SHOW(MechIsQuad(mech) ? "FRLEG" : "RARM", SHOWSECTSTAT(RARM));
    SHOW(MechIsQuad(mech) ? "RLLEG" : "LLEG", SHOWSECTSTAT(LLEG));
    SHOW(MechIsQuad(mech) ? "RRLEG" : "RLEG", SHOWSECTSTAT(RLEG));

    if (hasPhysical(mech, LARM, PHY_AXE))
      SHOW("Axe[LA]", SHOWPHYSTATUS(LARM, PHY_AXE));

    if (hasPhysical(mech, RARM, PHY_AXE))
      SHOW("Axe[RA]", SHOWPHYSTATUS(RARM, PHY_AXE));

    if (hasPhysical(mech, LARM, PHY_SWORD))
      SHOW("Sword[LA]", SHOWPHYSTATUS(LARM, PHY_SWORD));

    if (hasPhysical(mech, RARM, PHY_SWORD))
      SHOW("Sword[RA]", SHOWPHYSTATUS(RARM, PHY_SWORD));

    if (hasPhysical(mech, LARM, PHY_CLAW))
      SHOW("Claw[LA]", SHOWPHYSTATUS(LARM, PHY_CLAW));

    if (hasPhysical(mech, RARM, PHY_CLAW))
      SHOW("Claw[RA]", SHOWPHYSTATUS(RARM, PHY_CLAW));

    if (hasPhysical(mech, LARM, PHY_MACE))
      SHOW("Mace[LA]", SHOWPHYSTATUS(LARM, PHY_MACE));

    if (hasPhysical(mech, RARM, PHY_MACE))
      SHOW("Mace[RA]", SHOWPHYSTATUS(RARM, PHY_MACE));

    if (hasPhysical(mech, LARM, PHY_SAW))
      SHOW("Saw[LA]", SHOWPHYSTATUS(LARM, PHY_SAW));

    if (hasPhysical(mech, RARM, PHY_SAW))
      SHOW("Saw[RA]", SHOWPHYSTATUS(RARM, PHY_SAW));

    notify(evaluation, player, tempbuff);

    if (MechStatus(mech) & FLIPPED_ARMS)
      notify(evaluation, player,
             "*** Mech arms are flipped into the rear arc ***");
  } else if (MechType(mech) == CLASS_BSUIT && !compact) {
    for (i = 0; i < NUM_BSUIT_MEMBERS; i++)
      if (GetSectInt(mech, i))
        break;
    if (i < NUM_BSUIT_MEMBERS) {
      snprintf(tempbuff, sizeof(tempbuff), "Team status (special attacks): %s",
               SHOWSECTSTAT(i));
      notify(evaluation, player, tempbuff);
    }

  } else if (((MechType(mech) == CLASS_VEH_GROUND) ||
              (MechType(mech) == CLASS_VTOL)) &&
             !compact) {

    *tempbuff = 0;

    if (MechSections(mech)[FSIDE].recycle) {
      snprintf(tempbuff + strlen(tempbuff), sizeof(tempbuff) - strlen(tempbuff),
               "Vehicle status (charge): %s", SHOWSECTSTAT(FSIDE));
    }

    if (*tempbuff)
      notify(evaluation, player, tempbuff);
  }

  ammoweapcount = FindAmmunition(mech, ammoweap, ammo, ammomax, modearray, 0);
  if (!compact) {
    notify(evaluation, player,
           "==================WEAPON "
           "SYSTEMS===========================AMMUNITION========");
    if (MechType(mech) == CLASS_BSUIT)
      notify(evaluation, player,
             "------ Weapon --------- [##] Holder ------ Status ||--- "
             "Ammo Type ---- Rounds");
    else
      notify(evaluation, player,
             "------ Weapon --------- [##] Location ---- Status ||--- "
             "Ammo Type ---- Rounds");
  }
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    count = FindWeapons(mech, loop, weaparray, weapdata, critical);
    if (count <= 0)
      continue;
    ArmorStringFromIndex(loop, tempbuff, MechType(mech), MechMove(mech));
    snprintf(location, sizeof(location), "%-14.14s", tempbuff);
    if (compact) {
      strcpy(location, tempbuff);
      if ((tmpc = strchr(location, ' ')))
        *tmpc = '_';
    }
    for (ii = 0; ii < count; ii++) {
      if (IsAMS(weaparray[ii]))
        snprintf(weapbuff, sizeof(weapbuff), " %-16.16s %c%c%c%c%c [%2d] ",
                 &MechWeapons[weaparray[ii]].name[3], ' ',
                 (MechStatus(mech) & AMS_ENABLED) ? ' ' : 'O',
                 (MechStatus(mech) & AMS_ENABLED) ? 'O' : 'F',
                 (MechStatus(mech) & AMS_ENABLED) ? 'N' : 'F', ' ',
                 running_sum + ii);
      else {
        if (GetPartFireMode(mech, loop, critical[ii]) & OS_MODE)
          strcpy(tmpbuf, "OS ");
        else
          tmpbuf[0] = 0;
        strcat(tmpbuf, &MechWeapons[weaparray[ii]].name[3]);
        snprintf(
            weapbuff, sizeof(weapbuff), " %-16.16s %c%c%c%c%c [%2d] ", tmpbuf,
            (GetPartFireMode(mech, loop, critical[ii]) & REAR_MOUNT) ? 'R'
                                                                     : ' ',
            (((GetPartFireMode(mech, loop, critical[ii]) & OS_USED) ||
              (GetPartFireMode(mech, loop, critical[ii]) & ROCKET_FIRED))
                 ? '-'
             : (GetPartFireMode(mech, loop, critical[ii]) & OS_MODE) ? 'O'
                                                                     : ' '),
            GetWeaponAmmoModeLetter(mech, loop, critical[ii]),
            GetWeaponFireModeLetter(mech, loop, critical[ii]),
            ((GetPartFireMode(mech, loop, critical[ii]) & ON_TC) &&
             (!(MechCritStatus(mech) & TC_DESTROYED)))
                ? 'T'
            : (GetPartFireMode(mech, loop, critical[ii]) & IS_JETTISONED_MODE)
                ? 'J'
            : (GetPartFireMode(mech, loop, critical[ii]) & WILL_JETTISON_MODE)
                ? 'P'
                : ' ',
            running_sum + ii);
      }
      if (compact)
        append_status(compact_buffer, compact_buffer_size, "%s|%s",
                      &MechWeapons[weaparray[ii]].name[3], location);
      strcat(weapbuff, location);

      if (PartIsBroken(mech, loop, critical[ii]) ||
          PartTempNuke(mech, loop, critical[ii]) == FAIL_DESTROYED)
        strcat(weapbuff, "[fg=black bold]*****[reset]  || ");
      else if (PartIsDisabled(mech, loop, critical[ii]))
        strcat(weapbuff, "[fg=red]DISABLE[reset]|| ");
      else if (PartTempNuke(mech, loop, critical[ii])) {
        switch (PartTempNuke(mech, loop, critical[ii])) {
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
      } else if (GetPartFireMode(mech, loop, critical[ii]) & ROCKET_FIRED)
        strcat(weapbuff, "[fg=black bold]Empty[reset]  || ");
      else if (weapdata[ii])
        strcat(weapbuff, tprintf(" %2d    || ",
                                 weapdata[ii] / WEAPON_TICK +
                                     (weapdata[ii] % WEAPON_TICK ? 1 : 0)));
      else if (countDamagedSlotsFromCrit(mech, loop, critical[ii]))
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

      /*
         if (compact) {
         if (ammo_mode && ammo_mode != ' ')
         snprintf(compact_buffer + strlen(compact_buffer), sizeof(wierdbuf) -
         strlen(wierdbuf), "|%s|%d|%c ",
         &MechWeapons[ammoweap[running_sum]].name[3], ammo[running_sum],
         ammo_mode); else snprintf(compact_buffer + strlen(compact_buffer),
         sizeof(wierdbuf)
         - strlen(wierdbuf), "|%s|%d ",
         &MechWeapons[ammoweap[running_sum]].name[3], ammo[running_sum]);
         }
       */
      running_sum++;
    }
  }
}
