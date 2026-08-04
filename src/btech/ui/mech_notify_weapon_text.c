#include "mech_notify_internal.h"

const char *GetAmmoDesc_Model_Mode(int model, int mode) {
  if (mode & LBX_MODE)
    return " Shotgun";
  if (mode & ARTEMIS_MODE)
    return " Artemis IV";
  if (mode & NARC_MODE)
    return (MechWeapons[model].special & NARC) ? " Explosive" : " Narc";
  if (mode & INARC_EXPLO_MODE)
    return " iExplosive";
  if (mode & INARC_HAYWIRE_MODE)
    return " Haywire";
  if (mode & INARC_ECM_MODE)
    return " ECM";
  if (mode & INARC_NEMESIS_MODE)
    return " Nemesis";
  if (mode & SWARM_MODE)
    return " Swarm";
  if (mode & SWARM1_MODE)
    return " Swarm1";
  if (mode & INFERNO_MODE)
    return " Inferno";
  if (mode & CLUSTER_MODE)
    return " Cluster";
  if (mode & SMOKE_MODE)
    return " Smoke";
  if (mode & MINE_MODE)
    return " Mine";
  if (mode & AC_AP_MODE)
    return " Armor Piercing";
  if (mode & AC_FLECHETTE_MODE)
    return " Flechette";
  if (mode & AC_INCENDIARY_MODE)
    return " Incendiary";
  if (mode & AC_PRECISION_MODE)
    return " Precision";
  if (mode & STINGER_MODE)
    return " Stinger";
  if (mode & AC_CASELESS_MODE)
    return " Caseless";
  if (mode & SGUIDED_MODE)
    return " Sguided";
  if (mode & ATM_ER_MODE)
    return " ExtendedRange";
  if (mode & ATM_HE_MODE)
    return " HighExplosive";
  if (mode & MML_LRM_MODE)
    return " LRM";
  return "";
}

char GetWeaponAmmoModeLetter_Model_Mode(int model, int mode) {
  if (!(mode & AMMO_MODES))
    return ' ';
  if (mode & CLUSTER_MODE)
    return 'C';
  if (mode & SMOKE_MODE)
    return 'S';
  if (mode & MINE_MODE)
    return 'M';
  if (mode & LBX_MODE)
    return 'L';
  if (mode & ARTEMIS_MODE)
    return 'A';
  if (mode & NARC_MODE)
    return (MechWeapons[model].special & NARC) ? 'E' : 'N';
  if (mode & INARC_EXPLO_MODE)
    return 'X';
  if (mode & INARC_HAYWIRE_MODE)
    return 'Y';
  if (mode & INARC_ECM_MODE)
    return 'E';
  if (mode & INARC_NEMESIS_MODE)
    return 'Z';
  if (mode & INFERNO_MODE)
    return 'I';
  if (mode & SWARM_MODE)
    return 'W';
  if (mode & SWARM1_MODE)
    return '1';
  if (mode & AC_AP_MODE)
    return 'R';
  if (mode & AC_FLECHETTE_MODE)
    return 'F';
  if (mode & AC_INCENDIARY_MODE)
    return 'D';
  if (mode & AC_PRECISION_MODE)
    return 'P';
  if (mode & STINGER_MODE)
    return 'T';
  if (mode & AC_CASELESS_MODE)
    return 'U';
  if (mode & SGUIDED_MODE)
    return 'G';
  if (mode & ATM_ER_MODE)
    return 'R';
  if (mode & ATM_HE_MODE)
    return 'X';
  if (mode & MML_LRM_MODE)
    return '#';
  return ' ';
}

char GetWeaponFireModeLetter_Model_Mode(int model, int mode) {
  if (!(mode & FIRE_MODES))
    return ' ';
  if (mode & HOTLOAD_MODE)
    return 'H';
  if (mode & ULTRA_MODE)
    return 'U';
  if (mode & RFAC_MODE)
    return 'F';
  if (mode & GATTLING_MODE)
    return 'G';
  if (mode & HEAT_MODE)
    return 'H';
  if (mode & RAC_TWOSHOT_MODE)
    return '2';
  if (mode & RAC_FOURSHOT_MODE)
    return '4';
  if (mode & RAC_SIXSHOT_MODE)
    return '6';
  return ' ';
}

char GetWeaponAmmoModeLetter(Mech *mech, int loop, int crit) {
  return GetWeaponAmmoModeLetter_Model_Mode(
      Weapon2I(GetPartType(mech, loop, crit)),
      GetPartAmmoMode(mech, loop, crit));
}

char GetWeaponFireModeLetter(Mech *mech, int loop, int crit) {
  return GetWeaponFireModeLetter_Model_Mode(
      Weapon2I(GetPartType(mech, loop, crit)),
      GetPartFireMode(mech, loop, crit));
}

const char *GetMoveTypeID(int movetype) {
  switch (movetype) {
  case MOVE_QUAD:
    return "QUAD";
  case MOVE_BIPED:
    return "BIPED";
  case MOVE_TRACK:
    return "TRACKED";
  case MOVE_WHEEL:
    return "WHEELED";
  case MOVE_HOVER:
    return "HOVER";
  case MOVE_VTOL:
    return "VTOL";
  case MOVE_FLY:
    return "FLY";
  case MOVE_HULL:
    return "HULL";
  case MOVE_SUB:
    return "SUBMARINE";
  case MOVE_FOIL:
    return "HYDROFOIL";
  default:
    return "Unknown";
  }
}

static const struct {
  const char *onmsg;
  const char *offmsg;
  int flag;
  int infolvl;
} temp_flag_info_struct[] = {
    {"DESTROYED", nullptr, DESTROYED, 1},
    {nullptr, "SHUTDOWN", STARTED, 1},
    {"Torso is 60 degrees right", nullptr, TORSO_RIGHT, 0},
    {"Torso is 60 degrees left", nullptr, TORSO_LEFT, 0},
    {nullptr, nullptr, 0, 0}};

void Mech_ShowFlags(EvaluationContext *evaluation, DbRef player, Mech *mech,
                    int spaces, int level) {
  char buf[LBUF_SIZE];
  int i;

  for (i = 0; i < spaces; i++)
    buf[i] = ' ';
  buf[spaces] = 0;

  if (MechStatus(mech) & COMBAT_SAFE) {
    strcpy(buf + spaces, "[fg=blue bold]COMBAT SAFE[reset]");
    notify(evaluation, player, buf);
  }
  if (Fortified(mech)) {
    strcpy(buf + spaces, "[fg=green bold]FORTIFIED[reset]");
    notify(evaluation, player, buf);
  }
  if (WeaponsHold(mech)) {
    strcpy(buf + spaces, "[fg=red bold]WEAPONS HOLD[reset]");
    notify(evaluation, player, buf);
  }
  if (Fallen(mech)) {
    switch (MechMove(mech)) {
    case MOVE_BIPED:
      strcpy(buf + spaces, "[fg=red bold]FALLEN[reset]");
      break;
    case MOVE_QUAD:
      strcpy(buf + spaces, "[fg=red bold]FALLEN[reset]");
      break;
    case MOVE_TRACK:
      strcpy(buf + spaces, "[fg=red bold]TRACK DESTROYED[reset]");
      break;
    case MOVE_WHEEL:
      strcpy(buf + spaces, "[fg=red bold]AXLE DESTROYED[reset]");
      break;
    case MOVE_HOVER:
      strcpy(buf + spaces, "[fg=red bold]LIFT FAN DESTROYED[reset]");
      break;
    case MOVE_VTOL:
      strcpy(buf + spaces, "[fg=red bold]ROTOR DESTROYED[reset]");
      break;
    case MOVE_FLY:
      strcpy(buf + spaces, "[fg=red bold]ENGINE DESTROYED[reset]");
      break;
    case MOVE_HULL:
      strcpy(buf + spaces, "[fg=red bold]ENGINE ROOM DESTROYED[reset]");
      break;
    case MOVE_SUB:
      strcpy(buf + spaces, "[fg=red bold]ENGINE ROOM DESTROYED[reset]");
      break;
    case MOVE_FOIL:
      strcpy(buf + spaces, "[fg=red bold]FOIL DESTROYED[reset]");
      break;
    }
    notify(evaluation, player, buf);
  }
  if (IsHulldown(mech)) {
    strcpy(buf + spaces, "[fg=green bold]HULLDOWN[reset]");
    notify(evaluation, player, buf);
  }
  if (MechDugIn(mech)) {
    strcpy(buf + spaces, "[fg=green bold]DUG IN[reset]");
    notify(evaluation, player, buf);
  }
  if (Digging(mech)) {
    strcpy(buf + spaces, "[fg=green]DIGGING IN[reset]");
    notify(evaluation, player, buf);
  }
  if (Staggering(mech)) {
    strcpy(buf + spaces, "[fg=red bold]STAGGERING[reset]");
    notify(evaluation, player, buf);
  }
  if (MechCritStatus(mech) & SLITE_DEST) {
    strcpy(buf + spaces, "[fg=red bold]SEARCHLIGHT DESTROYED[reset]");
    notify(evaluation, player, buf);
  }
  if (MechLites(mech)) {
    strcpy(buf + spaces, "[fg=green bold]SEARCHLIGHT ON[reset]");
    notify(evaluation, player, buf);
  } else if (MechLit(mech)) {
    strcpy(buf + spaces, "[fg=green bold]ILLUMINATED[reset]");
    notify(evaluation, player, buf);
  }
  if (mech_event_count(mech, EVENT_VEHICLEBURN) || Jellied(mech)) {
    strcpy(buf + spaces, "[fg=red bold]ON FIRE[reset]");
    notify(evaluation, player, buf);
  }
  if (MechCritStatus(mech) & HIDDEN) {
    strcpy(buf + spaces, tprintf("[fg=green bold]HIDDEN[reset]"));
    notify(evaluation, player, buf);
  }
  if (IsMechSwarmed(mech)) {
    strcpy(buf + spaces, "[fg=red bold]SWARMED BY ENEMY SUITS[reset]");
    notify(evaluation, player, buf);
  }
  if (IsMechMounted(mech)) {
    strcpy(buf + spaces, "[fg=red bold]MOUNTED BY FRIENDLY SUITS[reset]");
    notify(evaluation, player, buf);
  }
  if (MechSwarmTarget(mech) > 0) {
    if (btech_context_get_mech(mech->xcode.context, MechSwarmTarget(mech))) {
      if (MechTeam(btech_context_get_mech(
              mech->xcode.context, MechSwarmTarget(mech))) == MechTeam(mech))
        strcpy(buf + spaces, "[fg=green bold]MOUNTED ON FRIENDLY UNIT[reset]");
      else
        strcpy(buf + spaces, "[fg=green bold]SWARMING ENEMY UNIT[reset]");

      notify(evaluation, player, buf);
    }
  }
#ifdef BT_MOVEMENT_MODES
  if (MechStatus2(mech) & DODGING) {
    strcpy(buf + spaces, tprintf("[fg=red bold]DODGING[reset]"));
    notify(evaluation, player, buf);
  }
  if (MechStatus2(mech) & EVADING) {
    strcpy(buf + spaces, tprintf("[fg=red bold]EVADING[reset]"));
    notify(evaluation, player, buf);
  }
  if (MechStatus2(mech) & SPRINTING) {
    strcpy(buf + spaces, tprintf("[fg=red bold]SPRINTING[reset]"));
    notify(evaluation, player, buf);
  }
  if (mech_event_count(mech, EVENT_MOVEMODE)) {
    strcpy(buf + spaces,
           tprintf("[fg=yellow bold]CHANGING MOVEMENT MODE[reset]"));
    notify(evaluation, player, buf);
  }
  if (mech_event_count(mech, EVENT_SIDESLIP)) {
    strcpy(buf + spaces, tprintf("[fg=yellow bold]SIDESLIPPING[reset]"));
    notify(evaluation, player, buf);
  }
  if (MechTankCritStatus(mech) & CREW_STUNNED ||
      MechCritStatus(mech) & MECH_STUNNED) {
    strcpy(buf + spaces, "[fg=red bold]STUNNED[reset]");
    notify(evaluation, player, buf);
  }
#endif
  if (level == 0) { /* our own 'status' */
    if (ECMProtected(mech)) {
      strcpy(buf + spaces, "[fg=green bold]PROTECTED BY ECM[reset]");
      notify(evaluation, player, buf);
    }
    if (AngelECMProtected(mech)) {
      strcpy(buf + spaces, "[fg=green bold]PROTECTED BY ANGEL ECM[reset]");
      notify(evaluation, player, buf);
    }
    if (ECMDisturbed(mech)) {
      strcpy(buf + spaces, "[fg=yellow bold]AFFECTED BY ECM[reset]");
      notify(evaluation, player, buf);
    }
    if (AngelECMDisturbed(mech)) {
      strcpy(buf + spaces, "[fg=yellow bold]AFFECTED BY ANGEL ECM[reset]");
      notify(evaluation, player, buf);
    }
    if (ECMCountered(mech)) {
      strcpy(buf + spaces, "[fg=yellow bold]COUNTERED BY ECCM[reset]");
      notify(evaluation, player, buf);
    }
    if (StealthArmorActive(mech)) {
      strcpy(buf + spaces, "[fg=green bold]STEALTH ARMOR ACTIVE[reset]");
      notify(evaluation, player, buf);
    }
    if (NullSigSysActive(mech)) {
      strcpy(buf + spaces,
             "[fg=green bold]NULL SIGNATURE SYSTEM ACTIVE[reset]");
      notify(evaluation, player, buf);
    }
    if (checkAllSections(mech, NARC_ATTACHED)) {
      strcpy(buf + spaces, "[fg=yellow bold]NARC POD ATTACHED[reset]");
      notify(evaluation, player, buf);
    }
    if (checkAllSections(mech, INARC_HOMING_ATTACHED)) {
      strcpy(buf + spaces, "[fg=yellow bold]INARC HOMING POD ATTACHED[reset]");
      notify(evaluation, player, buf);
    }
    if (checkAllSections(mech, INARC_HAYWIRE_ATTACHED)) {
      strcpy(buf + spaces, "[fg=yellow bold]INARC HAYWIRE POD ATTACHED[reset]");
      notify(evaluation, player, buf);
    }
    if (checkAllSections(mech, INARC_ECM_ATTACHED)) {
      strcpy(buf + spaces, "[fg=yellow bold]INARC ECM POD ATTACHED[reset]");
      notify(evaluation, player, buf);
    }
    if (mech_event_count(mech, EVENT_VEHICLE_EXTINGUISH)) {
      strcpy(buf + spaces, "[fg=yellow bold]EXTINGUISHING FIRE[reset]");
      notify(evaluation, player, buf);
    }
    if (MechStatus2(mech) & AUTOTURN_TURRET) {
      strcpy(buf + spaces, "[fg=green bold]TURRET AUTO-TURN ENGAGED[reset]");
      notify(evaluation, player, buf);
    }
    if (MechSections(mech)[RARM].specials & CARRYING_CLUB) {
      strcpy(buf + spaces, "[fg=green bold]CARRYING CLUB - RIGHT ARM[reset]");
      notify(evaluation, player, buf);
    }
    if (MechSections(mech)[LARM].specials & CARRYING_CLUB) {
      strcpy(buf + spaces, "[fg=green bold]CARRYING CLUB - LEFT ARM[reset]");
      notify(evaluation, player, buf);
    }
  }
  for (i = 0; temp_flag_info_struct[i].flag; i++)
    if (temp_flag_info_struct[i].infolvl >= level) {
      if (MechStatus(mech) & temp_flag_info_struct[i].flag) {
        if (temp_flag_info_struct[i].onmsg) {
          strcpy(buf + spaces, temp_flag_info_struct[i].onmsg);
          notify(evaluation, player, buf);
        }
      } else {
        if (temp_flag_info_struct[i].offmsg) {
          strcpy(buf + spaces, temp_flag_info_struct[i].offmsg);
          notify(evaluation, player, buf);
        }
      }
    }
}

const char *GetArcID(Mech *mech, int arc) {
  int mechlike = (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW ||
                  MechType(mech) == CLASS_BSUIT);

  if (arc & FORWARDARC)
    return "Forward";
  if (arc & RSIDEARC)
    return mechlike ? "Right Arm" : "Right Side";
  if (arc & LSIDEARC)
    return mechlike ? "Left Arm" : "Left Side";
  if (arc & REARARC)
    return "Rear";
  return "NO";
}

MechDisplayId mech_to_mech_display_id_base(Mech *see, Mech *mech, int inlos) {
  char *mname;
  MechDisplayId id = {0};

  if (!is_good_obj(mech->xcode.context->database, mech->mynum))
    return id;

  if (!inlos)
    mname = "something";
  else
    mname = btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                 A_MECHNAME, (char[LBUF_SIZE]){0});

  snprintf(id.text, sizeof(id.text), "%s [%s]", mname,
           mech_id(mech, inlos && MechTeam(see) == MechTeam(mech)).text);
  return id;
}

MechDisplayId mech_to_mech_display_id(Mech *see, Mech *mech) {
  char *mname;
  int team;
  MechDisplayId id = {0};

  if (!mech) {
    dprintk("bad mech");
    return id;
  }
  if (!see) {
    dprintk("bad see");
    return id;
  }
  if (!is_good_obj(mech->xcode.context->database, mech->mynum))
    return id;

  if (!InLineOfSight_NB(see, mech, 0, 0, 0)) {
    mname = "something";
    team = 0;
  } else {
    mname = btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                 A_MECHNAME, (char[LBUF_SIZE]){0});
    team = (MechTeam(see) == MechTeam(mech));
  }

  snprintf(id.text, sizeof(id.text), "%s [%s]", mname,
           mech_id(mech, team).text);
  return id;
}

MechDisplayId mech_display_id(Mech *mech) {
  char *mname;
  MechDisplayId id = {0};

  if (!is_good_obj(mech->xcode.context->database, mech->mynum))
    return id;

  mname = btech_attribute_read(mech->xcode.context->database, mech->mynum,
                               A_MECHNAME, (char[LBUF_SIZE]){0});
  snprintf(id.text, sizeof(id.text), "%s [%s]", mname,
           mech_id(mech, false).text);
  return id;
}
