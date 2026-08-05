#include "mech_template_api.h"
#include "mech_utils_internal.h"

void ArmorStringFromIndex(int index, char *buffer, char type, char mtype) {
  char **locs = ProperSectionStringFromType(type, mtype);
  int high = 0;

  switch (type) {
  case CLASS_MECH:
  case CLASS_MW:
    high = NUM_SECTIONS;
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
    high = (NUM_VEH_SECTIONS - 1);
    break;
  case CLASS_VTOL:
    high = NUM_VEH_SECTIONS;
    break;
  case CLASS_AERO:
    high = NUM_AERO_SECTIONS;
    break;
  case CLASS_SPHEROID_DS:
  case CLASS_DS:
    high = NUM_DS_SECTIONS;
    break;
  case CLASS_BSUIT:
    high = NUM_BSUIT_MEMBERS;
    break;
  default:
    strcpy(buffer, "Invalid!!");
    return;
  }
  if (high > 0 && index < high && locs) {
    strcpy(buffer, locs[index]);
    return;
  }
  strcpy(buffer, "Invalid!!");
}

int IsInWeaponArc(Mech *mech, float x, float y, int section, int critical) {
  int weaponarc, isrear;
  int wantarc = NOARC;

  if (MechType(mech) == CLASS_MECH &&
      (section == LLEG || section == RLEG ||
       (MechIsQuad(mech) && (section == LARM || section == RARM)))) {
    int ts = MechStatus(mech) & (TORSO_LEFT | TORSO_RIGHT);
    MechStatus(mech) &= ~(ts);
    weaponarc = InWeaponArc(mech, x, y);
    MechStatus(mech) |= ts;
  } else
    weaponarc = InWeaponArc(mech, x, y);

  switch (MechType(mech)) {
  case CLASS_MECH:
  case CLASS_BSUIT:
  case CLASS_MW:
    if (GetPartFireMode(mech, section, critical) & REAR_MOUNT)
      wantarc = REARARC;
    else if (section == LARM && (MechStatus(mech) & FLIPPED_ARMS))
      wantarc = REARARC | LSIDEARC;
    else if (section == LARM)
      wantarc = FORWARDARC | LSIDEARC;
    else if (section == RARM && (MechStatus(mech) & FLIPPED_ARMS))
      wantarc = REARARC | RSIDEARC;
    else if (section == RARM)
      wantarc = FORWARDARC | RSIDEARC;
    else
      wantarc = FORWARDARC;
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
    switch (section) {
    case TURRET:
      wantarc = TURRETARC;
      break;
    case FSIDE:
      wantarc = FORWARDARC;
      break;
    case LSIDE:
      wantarc = LSIDEARC;
      break;
    case RSIDE:
      wantarc = RSIDEARC;
      break;
    case BSIDE:
      wantarc = REARARC;
      break;
    }
    break;
  case CLASS_DS:
    switch (section) {
    case DS_NOSE:
      wantarc = FORWARDARC;
      break;
    case DS_LWING:
    case DS_LRWING:
      wantarc = LSIDEARC;
      break;
    case DS_RWING:
    case DS_RRWING:
      wantarc = RSIDEARC;
      break;
    case DS_AFT:
      wantarc = REARARC;
      break;
    }
    break;
  case CLASS_SPHEROID_DS:
    switch (section) {
    case DS_NOSE:
      wantarc = FORWARDARC;
      break;
    case DS_LWING:
      wantarc = FORWARDARC | LSIDEARC;
      break;
    case DS_LRWING:
      wantarc = REARARC | LSIDEARC;
      break;
    case DS_RWING:
      wantarc = FORWARDARC | RSIDEARC;
      break;
    case DS_RRWING:
      wantarc = REARARC | RSIDEARC;
      break;
    case DS_AFT:
      wantarc = REARARC;
      break;
    }
    break;

  case CLASS_AERO:
    isrear = (GetPartFireMode(mech, section, critical) & REAR_MOUNT);
    switch (section) {
    case AERO_NOSE:
      wantarc = FORWARDARC | LSIDEARC | RSIDEARC;
      break;
    case AERO_LWING:
      wantarc = LSIDEARC | (isrear ? REARARC : FORWARDARC);
      break;
    case AERO_RWING:
      wantarc = RSIDEARC | (isrear ? REARARC : FORWARDARC);
      break;
    case AERO_AFT:
      wantarc = REARARC;
      break;
    }
    break;
  }
  return wantarc ? (wantarc & weaponarc) : 0;
}

int GetWeaponCrits(Mech *mech, int weapindx) {
  return (MechType(mech) == CLASS_MECH) ? (MechWeapons[weapindx].criticals) : 1;
}

int listmatch(char *const *foo, char *mat) {
  int i;

  for (i = 0; foo[i]; i++)
    if (!strcasecmp(foo[i], mat))
      return i;
  return -1;
}

/* Takes care of :
   JumpSpeed
   Numsinks

   TODO: More support(?)
 */

void do_sub_magic(Mech *mech, int loud) {
  int jjs = 0;
  int hses = 0;
  int wanths, wanths_f;
  int shs_size = HS_Size(mech);
  int hs_eff = HS_Efficiency(mech);
  int i, j;
  int inthses = MechEngineSize(mech) / 25;
  int dest_hses = 0;
  int maxjjs =
      (int)((float)MechMaxSpeed(mech) * MP_PER_KPH *
            ((!(MechSpecials2(mech) & IMPROVED_JJ_TECH)) ? (2 / 3) : 1));

  if (MechSpecials(mech) & ICE_TECH)
    inthses = 0;
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++)
      switch (Special2I(GetPartType(mech, i, j))) {
      case HEAT_SINK:
        hses++;
        if (PartIsNonfunctional(mech, i, j))
          dest_hses++;
        break;
      case JUMP_JET:
        jjs++;
        break;
      }
  if (MechHSEngOverRide(mech))
    inthses = MechHSEngOverRide(mech);
  hses += MIN(MechRealNumsinks(mech) * shs_size / hs_eff, inthses * shs_size);

  /* Improved are 2 crits per Jump MP */
  if ((MechSpecials2(mech) & IMPROVED_JJ_TECH))
    jjs = jjs / 2;

  if (jjs > maxjjs) {
    if (loud)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Error in #%ld (%s): %d JJs, yet %d maximum available "
                  "(due to walk MPs)?",
                  mech->mynum, MechType_Ref(mech), jjs, maxjjs));

    jjs = maxjjs;
  }
  MechJumpSpeed(mech) = MP1 * jjs;
  wanths_f = (hses / shs_size) * hs_eff;
  wanths = wanths_f - (dest_hses * hs_eff / shs_size);
  if (loud)
    MechNumOsinks(mech) =
        wanths - MIN(MechRealNumsinks(mech), inthses * hs_eff);
  if (wanths != MechRealNumsinks(mech) && loud) {
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in #%ld (%s): Set HS: %d. Existing HS: %d. "
                "Difference: %d. Please %s.",
                mech->mynum, MechType_Ref(mech), MechRealNumsinks(mech), wanths,
                MechRealNumsinks(mech) - wanths,
                wanths < MechRealNumsinks(mech) ? "add the extra HS critical(s)"
                                                : "fix the template"));
  } else
    MechRealNumsinks(mech) = wanths;
  MechNumOsinks(mech) = wanths_f;

  if ((MechNumOsinks(mech) * shs_size / hs_eff -
       (MechSpecials(mech) & ICE_TECH ? 0 : 10) * shs_size) < 0)
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Error in #%ld (%s): HS less then max possible in engine!",
                mech->mynum, MechType_Ref(mech)));
}

#define CV(fun) fun(mech) = fun(&opp)

/* Values to take care of:
   - JumpSpeed
   - MaxSpeed
   - Numsinks
   - EngineHeat
   - PilotSkillBase
   - LRS/Tac/ScanRange
   - BTH

   Status:
   - Destroyed

   Critstatus:
   - kokonaan paitsi

   section(s) / basetohit
 */

void do_fixextra(Mech *mech) {

  int i, j;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (SectIsFlooded(mech, i))
      UnSetSectFlooded(mech, i);
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (!IsAmmo(GetPartType(mech, i, j))) {
        if (!PartIsBroken(mech, i, j) && !PartIsDestroyed(mech, i, j))
          mech_RepairPart(mech, i, j);
        else {
          UnDisablePart(mech, i, j);
          mech_RepairPart(mech, i, j);
        }
      } else {
        UnDisablePart(mech, i, j);
        mech_FillPartAmmo(mech, i, j);
      }
    }
  }
}

void do_magic(Mech *mech) {
  Mech opp;
  int i, j, t;
  int mask = 0;
  int tankCritMask = 0;

  if (MechType(mech) != CLASS_MECH)
    tankCritMask =
        (TURRET_LOCKED | TURRET_JAMMED | TAIL_ROTOR_DESTROYED | CREW_STUNNED);

  /* stop the burning */
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  StopPerformingAction(mech);

  memcpy(&opp, mech, sizeof(Mech));
  mech_template_load(GOD, &opp, MechType_Ref(mech));
  MechEngineSizeV(mech) = MechEngineSizeC(&opp); /* From intact template */
  opp.mynum = -1;
  /* Ok.. It's at perfect condition. Start inflicting some serious crits.. */
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      SetPartType(&opp, i, j, GetPartType(mech, i, j));
      SetPartBrand(&opp, i, j, GetPartBrand(mech, i, j));
      SetPartData(&opp, i, j, 0);
      SetPartFireMode(&opp, i, j, 0);
      SetPartAmmoMode(&opp, i, j, 0);
    }
  if (MechType(mech) == CLASS_MECH)
    do_sub_magic(&opp, 0);
  MechNumOsinks(mech) = MechNumOsinks(&opp);
  for (i = 0; i < NUM_SECTIONS; i++) {

    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (PartIsDestroyed(mech, i, j)) {
        if (!PartIsDestroyed(&opp, i, j)) {
          if (!IsAmmo((t = GetPartType(mech, i, j)))) {
            if (!IsWeapon(t))
              if (MechType(mech) == CLASS_MECH)
                mech_critical_effect_apply(&opp, nullptr, 0, i, j, t,
                                           GetPartData(mech, i, j));
          }
        }
      } else {
        t = GetPartType(mech, i, j);
        if (IsAMS(Weapon2I(t))) {
          if (MechWeapons[Weapon2I(t)].special & CLAT)
            MechSpecials(mech) |= CL_ANTI_MISSILE_TECH;
          else
            MechSpecials(mech) |= IS_ANTI_MISSILE_TECH;
        }
        GetPartFireMode(mech, i, j) &=
            ~(OS_USED | ROCKET_FIRED | IS_JETTISONED_MODE);
      }
    }

    MechSections(mech)[i].config &= ~STABILIZERS_DESTROYED;

    if (SectIsDestroyed(mech, i))
      mech_section_destroy(&opp, nullptr, 0, i);
    if (MechStall(mech) > 0)
      UnSetSectBreached(mech, i); /* Just in case ; this leads to 'unbreachable'
                                     legs once you've 'done your time' once */
  }
  CV(MechJumpSpeed);
  CV(MechMaxSpeed);
  CV(MechRealNumsinks);
  CV(MechEngineHeat);
  CV(MechPilotSkillBase);
  CV(MechLRSRange);
  CV(MechTacRange);
  CV(MechScanRange);
  CV(MechBTH);
  MechCritStatus(mech) &= mask;
  MechCritStatus(mech) |= MechCritStatus(&opp) & (~mask);

  MechTankCritStatus(mech) &= tankCritMask;
  MechTankCritStatus(mech) |= MechTankCritStatus(&opp) & (~tankCritMask);

  for (i = 0; i < NUM_SECTIONS; i++) {
    MechSections(mech)[i].basetohit = MechSections(&opp)[i].basetohit;
    MechSections(mech)[i].specials = MechSections(&opp)[i].specials;
    MechSections(mech)[i].specials &=
        ~(INARC_HOMING_ATTACHED | INARC_HAYWIRE_ATTACHED | INARC_ECM_ATTACHED |
          INARC_NEMESIS_ATTACHED);
  }

  /* Case of undestroying */
  if (!Destroyed(&opp) && Destroyed(mech))
    MechStatus(mech) &= ~DESTROYED;
  else if (Destroyed(&opp) && !Destroyed(mech))
    MechStatus(mech) |= DESTROYED;
  if (!Destroyed(mech) && MechType(mech) != CLASS_MECH)
    EvalBit(MechStatus(mech), FALLEN, Fallen(&opp));
  update_specials(mech);
}

void mech_RepairPart(Mech *mech, int loc, int pos) {
  int t = GetPartType(mech, loc, pos);

  UnDestroyPart(mech, loc, pos);
  if (IsWeapon(t) || IsAmmo(t)) {
    SetPartData(mech, loc, pos, 0);
    GetPartFireMode(mech, loc, pos) &=
        ~(OS_USED | IS_JETTISONED_MODE | ROCKET_FIRED);
  } else if (IsSpecial(t)) {
    switch (Special2I(t)) {
    case TARGETING_COMPUTER:
    case HEAT_SINK:
    case LIFE_SUPPORT:
    case COCKPIT:
    case SENSORS:
    case JUMP_JET:
    case ENGINE:
    case GYRO:
    case SHOULDER_OR_HIP:
    case LOWER_ACTUATOR:
    case UPPER_ACTUATOR:
    case HAND_OR_FOOT_ACTUATOR:
    case C3_MASTER:
    case C3_SLAVE:
    case C3I:
    case ECM:
    case ANGELECM:
    case NULL_SIGNATURE_SYSTEM:
    case BEAGLE_PROBE:
    case LIGHT_BAP:
    case ARTEMIS_IV:
    case TAG:
    case BLOODHOUND_PROBE:
      /* Magic stuff here :P */
      if (MechType(mech) == CLASS_MECH)
        do_magic(mech);
      break;
    }
  }
}

int no_locations_destroyed(Mech *mech) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++)
    if (GetSectOInt(mech, i) && SectIsDestroyed(mech, i))
      return 0;
  return 1;
}

void mech_ReAttach(Mech *mech, int loc) {
  if (!SectIsDestroyed(mech, loc))
    return;
  UnSetSectDestroyed(mech, loc);
  UnSetSectFlooded(mech, loc);
  SetSectInt(mech, loc, GetSectOInt(mech, loc));
  if (is_aero(mech))
    SetSectInt(mech, loc, 1);
  if (MechType(mech) != CLASS_MECH) {
    if (no_locations_destroyed(mech) && IsDS(mech))
      MechStatus(mech) &= ~DESTROYED;
    return;
  }
}

void mech_ReplaceSuit(Mech *mech, int loc) {
  if (!SectIsDestroyed(mech, loc))
    return;

  UnSetSectDestroyed(mech, loc);
  SetSectInt(mech, loc, GetSectOInt(mech, loc));
}

/*
 * Added for new flood code by Kipsta
 * 8/4/99
 */

void mech_ReSeal(Mech *mech, int loc) {
  int i;

  if (SectIsDestroyed(mech, loc))
    return;
  if (!SectIsFlooded(mech, loc))
    return;

  UnSetSectFlooded(mech, loc);

  for (i = 0; i < CritsInLoc(mech, loc); i++) {
    if (PartIsDisabled(mech, loc, i)) {
      if (!PartIsBroken(mech, loc, i) && !PartIsDamaged(mech, loc, i))
        mech_RepairPart(mech, loc, i);
      else
        UnDisablePart(mech, loc, i);
    }
  }
}

void mech_Detach(Mech *mech, int loc) {
  if (SectIsDestroyed(mech, loc))
    return;
  mech_section_destroy(mech, nullptr, 0, loc);
}

/* Figures out how much ammo there is when we're 'fully loaded', and
   fills it */
void mech_FillPartAmmo(Mech *mech, int loc, int pos) {
  int t, to;

  t = GetPartType(mech, loc, pos);

  if (!IsAmmo(t))
    return;
  if (!(to = MechWeapons[Ammo2Weapon(t)].ammoperton))
    return;
  SetPartData(mech, loc, pos, FullAmmo(mech, loc, pos));
}

int AcceptableDegree(int d) {
  /*
   * Silly billies, integer modulo (division) is still faster than loops.
   * And probably slightly faster than branches, too, but let's not worry
   * about that.
   */
  if (d < 0) {
    return (d % 360) + 360;
  } else if (d >= 360) {
    return (d % 360);
  } else {
    return d;
  }
}

void MarkForLOSUpdate(Mech *mech) {
  BattleMap *mech_map;

  if (!(mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
    return;
  mech_map->moves++;
  mech_map->mechflags[mech->mapnumber] = 1;
}

void multi_weap_sel(Mech *mech, DbRef player, char *buffer, int bitbybit,
                    MultiWeaponSelectionCallback callback, void *context) {
  /* Insight: buffer contains stuff in form:
     <num>
     <num>-<num>
     <num>,..
     <num>-<num>,..
   */
  /* Ugly recursive piece of code :> */
  char *c;
  int i1, i2, i3;
  int section, critical;

  skipws(buffer);
  if ((c = strstr(buffer, ","))) {
    *c = 0;
    c++;
  }
  if (sscanf(buffer, "%d-%d", &i1, &i2) == 2) {
    DOCHECK_CONTEXT(mech->xcode.context, i1 < 0 || i1 >= MAX_WEAPONS_PER_MECH,
                    tprintf("Invalid first number in range (%d)", i1));
    DOCHECK_CONTEXT(mech->xcode.context, i2 < 0 || i2 >= MAX_WEAPONS_PER_MECH,
                    tprintf("Invalid second number in range (%d)", i2));
    if (i1 > i2) {
      i3 = i1;
      i1 = i2;
      i2 = i3;
    }
  } else {
    DOCHECK_CONTEXT(mech->xcode.context, Readnum(i1, buffer),
                    tprintf("Invalid value: %s", buffer));
    DOCHECK_CONTEXT(mech->xcode.context, i1 < 0 || i1 >= MAX_WEAPONS_PER_MECH,
                    tprintf("Invalid weapon number: %d", i1));
    i2 = i1;
  }
  if (bitbybit / 2) {
    DOCHECK_CONTEXT(mech->xcode.context, i2 >= NUM_TICS,
                    tprintf("There are only %d tics!", i2));
  } else {
    DOCHECK_CONTEXT(
        mech->xcode.context,
        !(FindWeaponNumberOnMech(mech, i2, &section, &critical) != -1),
        tprintf("Error: the mech doesn't HAVE %d weapons!", i2 + 1));
  }
  if (bitbybit % 2) {
    for (i3 = i1; i3 <= i2; i3++)
      if (callback(mech, player, i3, i3, context))
        return;
  } else if (callback(mech, player, i1, i2, context))
    return;
  if (c)
    multi_weap_sel(mech, player, c, bitbybit, callback, context);
}
