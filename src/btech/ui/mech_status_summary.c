#include "mech_status_internal.h"

void append_status(char *buffer, size_t size, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
void print_weapon_status(EvaluationContext *evaluation, Mech *mech,
                         DbRef player, bool compact, char *compact_buffer,
                         size_t compact_buffer_size);

void append_status(char *buffer, size_t size, const char *fmt, ...) {
  size_t len = strlen(buffer);
  va_list ap;

  if (len >= size)
    return;

  va_start(ap, fmt);
  vsnprintf(buffer + len, size - len, fmt, ap);
  va_end(ap);
}

void DisplayTarget(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  int arc;
  Mech *tempMech = NULL;
  char location[50] = {0};
  char buff[MBUF_SIZE] = {0};
  char buff1[100] = {0};

  if (MechTarget(mech) != -1) {
    tempMech = btech_context_get_mech(mech->xcode.context, MechTarget(mech));
    if (tempMech) {
      if (InLineOfSight(mech, tempMech, MechX(tempMech), MechY(tempMech),
                        FaMechRange(mech, tempMech))) {
        snprintf(buff, sizeof(buff),
                 "Target: %s\t   Range: %.1f hexes   Bearing: %d deg\n",
                 mech_to_mech_display_id(mech, tempMech).text,
                 FaMechRange(mech, tempMech),
                 FindBearing(MechFX(mech), MechFY(mech), MechFX(tempMech),
                             MechFY(tempMech)));
        notify(evaluation, player, buff);
        arc = InWeaponArc(mech, MechFX(tempMech), MechFY(tempMech));
        strcpy(buff,
               tprintf("Target in %s Weapons Arc",
                       (arc & TURRETARC) ? "Turret" : GetArcID(mech, arc)));
        if (MechAim(mech) == NUM_SECTIONS ||
            MechAimType(mech) != MechType(tempMech))
          strcpy(location, "None");
        else
          ArmorStringFromIndex(MechAim(mech), location, MechType(tempMech),
                               MechMove(tempMech));
        snprintf(buff1, sizeof(buff1), "\t   Aimed Shot Location: %s",
                 location);
        strcat(buff, buff1);
      } else
        snprintf(buff, sizeof(buff), "Target: NOT in line of sight!\n");
    }
    notify(evaluation, player, buff);
  } else if (MechTargX(mech) != -1 && MechTargY(mech) != -1) {
    if (MechStatus(mech) & LOCK_BUILDING)
      notify_printf(evaluation, player, "Target: Building at %d %d\n",
                    MechTargX(mech), MechTargY(mech));
    else if (MechStatus(mech) & LOCK_HEX)
      notify_printf(evaluation, player, "Target: Hex %d %d\n", MechTargX(mech),
                    MechTargY(mech));
    else
      notify_printf(evaluation, player, "Target: %d %d\n", MechTargX(mech),
                    MechTargY(mech));
  }
  if (MechPKiller(mech))
    notify(evaluation, player,
           "Weapon Safeties are [fg=red bold]OFF[reset].\n");
  if (mech_has_pilot(mech) &&
      HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "maneuvering_ace"))
    notify_printf(evaluation, player, "Turn Mode: %s",
                  GetTurnMode(mech) ? "TIGHT" : "NORMAL");
  if (MechChargeTarget(mech) > 0 &&
      mech->xcode.context->configuration->btech_newcharge) {
    tempMech =
        btech_context_get_mech(mech->xcode.context, MechChargeTarget(mech));
    if (!tempMech)
      return;
    if (InLineOfSight(mech, tempMech, MechX(tempMech), MechY(tempMech),
                      FaMechRange(mech, tempMech))) {
      notify_printf(evaluation, player, "ChargeTarget: %s\t  ChargeTimer: %d\n",
                    mech_to_mech_display_id(mech, tempMech).text,
                    MechChargeTimer(mech) / 2);
    } else {
      notify_printf(evaluation, player,
                    "ChargeTarget: NOT in line of sight!\t Timer: %d\n",
                    MechChargeTimer(mech) / 2);
    }
  }
}

void show_miscbrands(Mech *mech, DbRef player) {
  /*   notify(evaluation, player,
   * tprintf("Radio: %s (%3d range) Computer: %s (%d Scan / %d LRS / %d Tac)",
   * brands[BOUNDED(1, MechRadio(mech), 5)+RADIO_INDEX].name, (int)
   * MechRadioRange(mech), brands[BOUNDED(1, MechComputer(mech),
   * 5)+COMPUTER_INDEX].name, (int) MechScanRange(mech), (int)
   * MechLRSRange(mech), (int) MechTacRange(mech)));
   */
}

void PrintGenericStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                        int own, int usex) {
  Mech *tempMech = NULL;
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);
  char buff[SBUF_SIZE];
  char mech_name[100] = {0};
  char mech_ref[100] = {0};
  char move_type[50] = {0};

  strcpy(mech_name,
         usex ? MechType_Name(mech)
              : btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                     A_MECHNAME, (char[LBUF_SIZE]){0}));
  strcpy(mech_ref,
         usex ? MechType_Ref(mech)
              : btech_attribute_read(mech->xcode.context->database, mech->mynum,
                                     A_MECHREF, (char[LBUF_SIZE]){0}));

  switch (MechType(mech)) {
  case CLASS_MW:
    notify_printf(evaluation, player, "MechWarrior: %-18.18s ID:[%s]",
                  game_object_name(mech->xcode.context->database, player),
                  mech_id(mech, false).text);
    notify_printf(evaluation, player, "MaxSpeed: %3d", (int)MMaxSpeed(mech));
    break;
  case CLASS_BSUIT:
    snprintf(buff, sizeof(buff),
             "%s Name: %-18.18s  ID:[%s]   %s Reference: %s",
             GetBSuitName(mech), mech_name, mech_id(mech, false).text,
             GetBSuitName(mech), mech_ref);
    notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "MaxSpeed: %3d                  JumpRange: %d",
                  (int)MMaxSpeed(mech), JumpSpeedMP(mech, map));
    show_miscbrands(mech, player);
    if (MechPilot(mech) == -1)
      notify(evaluation, player, "Leader: NONE");
    else {
      snprintf(buff, sizeof(buff),
               "%s Leader Name: %-16.16s %s Leader injury: %d",
               GetBSuitName(mech),
               game_object_name(mech->xcode.context->database, MechPilot(mech)),
               GetBSuitName(mech), MechPilotStatus(mech));
      notify(evaluation, player, buff);
    }

    snprintf(buff, sizeof(buff), "Max Suits: %d", MechMaxSuits(mech));
    notify(evaluation, player, buff);

    Mech_ShowFlags(evaluation, player, mech, 0, 0);

    if (Jumping(mech)) {
      snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d", MechGoingX(mech),
               MechGoingY(mech));
      if ((MechStatus(mech) & DFA_ATTACK) && MechDFATarget(mech) != -1) {
        tempMech =
            btech_context_get_mech(mech->xcode.context, MechDFATarget(mech));
        snprintf(buff + strlen(buff), sizeof(buff) - strlen(buff),
                 "  Death From Above Target: %s",
                 mech_to_mech_display_id(mech, tempMech).text);
      }
      notify(evaluation, player, buff);
    }
    break;
  case CLASS_MECH:
    snprintf(buff, sizeof(buff),
             "Mech Name: %-18.18s  ID:[%s]   Mech Reference: %s", mech_name,
             mech_id(mech, false).text, mech_ref);
    notify(evaluation, player, buff);
    notify_printf(evaluation, player,
                  "Tonnage:   %3d     MaxSpeed: %3d       JumpRange: %d",
                  MechTons(mech), (int)MMaxSpeed(mech), JumpSpeedMP(mech, map));
    show_miscbrands(mech, player);
    if (MechPilot(mech) == -1)
      notify(evaluation, player, "Pilot: NONE");
    else {
      snprintf(buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
               game_object_name(mech->xcode.context->database, MechPilot(mech)),
               MechPilotStatus(mech));
      notify(evaluation, player, buff);
    }
    Mech_ShowFlags(evaluation, player, mech, 0, 0);
    if (!Jumping(mech) && !Fallen(mech) && Started(mech) &&
        (MechChargeTarget(mech) != -1)) {
      tempMech =
          btech_context_get_mech(mech->xcode.context, MechChargeTarget(mech));
      if (tempMech) {
        snprintf(buff, sizeof(buff), "CHARGING --> %s",
                 mech_to_mech_display_id(mech, tempMech).text);
        notify(evaluation, player, buff);
      }
    }
    if (Jumping(mech)) {
      snprintf(buff, sizeof(buff), "JUMPING --> %3d,%3d", MechGoingX(mech),
               MechGoingY(mech));
      if ((MechStatus(mech) & DFA_ATTACK) && MechDFATarget(mech) != -1) {
        tempMech =
            btech_context_get_mech(mech->xcode.context, MechDFATarget(mech));
        snprintf(buff + strlen(buff), sizeof(buff) - strlen(buff),
                 "  Death From Above Target: %s",
                 mech_to_mech_display_id(mech, tempMech).text);
      }
      notify(evaluation, player, buff);
    }
    break;
  case CLASS_VTOL:
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    switch (MechMove(mech)) {
    case MOVE_TRACK:
      strcpy(move_type, "Tracked");
      break;
    case MOVE_WHEEL:
      strcpy(move_type, "Wheeled");
      break;
    case MOVE_HOVER:
      strcpy(move_type, "Hover");
      break;
    case MOVE_VTOL:
      strcpy(move_type, "VTOL");
      break;
    case MOVE_FLY:
      strcpy(move_type, "Flight");
      break;
    case MOVE_HULL:
      strcpy(move_type, "Displacement Hull");
      break;
    case MOVE_SUB:
      strcpy(move_type, "Submarine");
      break;
    case MOVE_FOIL:
      strcpy(move_type, "Hydrofoil");
      break;
    default:
      strcpy(move_type, "Magic");
      break;
    }
    if (MechMove(mech) != MOVE_NONE) {
      snprintf(buff, sizeof(buff),
               "Vehicle Name: %-15.15s  ID:[%s]   Vehicle Reference: %s",
               mech_name, mech_id(mech, false).text, mech_ref);
      notify(evaluation, player, buff);
      snprintf(buff, sizeof(buff),
               "Tonnage:   %3d      %s: %3d       Movement Type: %s",
               MechTons(mech), is_aero(mech) ? "Max thrust" : "FlankSpeed",
               (int)MMaxSpeed(mech), move_type);
      notify(evaluation, player, buff);
      show_miscbrands(mech, player);
      if (MechPilot(mech) == -1)
        notify(evaluation, player, "Pilot: NONE");
      else {
        snprintf(
            buff, sizeof(buff), "Pilot Name: %-28.28s Pilot Injury: %d",
            game_object_name(mech->xcode.context->database, MechPilot(mech)),
            MechPilotStatus(mech));
        notify(evaluation, player, buff);
      }
    } else {
      snprintf(buff, sizeof(buff), "Name: %-15.15s  ID:[%s]   Reference: %s",
               mech_name, mech_id(mech, false).text, mech_ref);
      notify(evaluation, player, buff);
    }
    if (MechType(mech) != CLASS_VTOL && !is_aero(mech))
      if (GetSectInt(mech, TURRET)) {
        if (MechTankCritStatus(mech) & TURRET_JAMMED)
          notify(evaluation, player, "     TURRET JAMMED");
        else if (MechTankCritStatus(mech) & TURRET_LOCKED)
          notify(evaluation, player, "     TURRET LOCKED");
      }
    if (FlyingT(mech) && Landed(mech))
      notify(evaluation, player, "LANDED");
    Mech_ShowFlags(evaluation, player, mech, 0, 0);
  }
}

void PrintShortInfo(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char buff[MBUF_SIZE] = {0};
  char typespecific[50] = {0};

  switch (MechType(mech)) {
  case CLASS_VTOL:
    snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
             MechVerticalSpeed(mech));
    break;
  case CLASS_MECH:
    snprintf(typespecific, sizeof(typespecific), " HT: %3d/%3d/%-3d ",
             (int)(10. * MechPlusHeat(mech)),
             (int)(10. * MechActiveNumsinks(mech)),
             (int)(10. * MechMinusHeat(mech)));
    break;
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    snprintf(typespecific, sizeof(typespecific),
             " VSPD: %3.1f  ANG: %2d  HT: %3d/%3d ", MechVerticalSpeed(mech),
             MechDesiredAngle(mech), (int)(10 * MechPlusHeat(mech)),
             (int)(10 * MechActiveNumsinks(mech)));
    break;
  case CLASS_VEH_NAVAL:
    if (MechMove(mech) == MOVE_FOIL)
      snprintf(typespecific, sizeof(typespecific), " VSPD: %3.1f ",
               MechVerticalSpeed(mech));
    /* FALLTHROUGH */
  case CLASS_VEH_GROUND:
    /* XXX This won't work for subs with turrets.. are they possible ? */
    if (GetSectOInt(mech, TURRET)) {
      snprintf(typespecific, sizeof(typespecific), " TUR: %3d ",
               AcceptableDegree(MechTurretFacing(mech) + MechFacing(mech)));
      break;
    }
    /* FALLTHROUGH */
  default:
    typespecific[0] = '\0';
    break;
  }

  snprintf(buff, sizeof(buff),
           "LOC: %3d,%3d,%3d  HD: %3d/%3d  SP: %3.1f/%3.1f %s ST:%s",
           MechX(mech), MechY(mech), MechZ(mech), MechFacing(mech),
           MechDesiredFacing(mech), MechSpeed(mech), MechDesiredSpeed(mech),
           typespecific, mech_status_string(mech, 2).text);
  notify(evaluation, player, buff);
  DisplayTarget(evaluation, player, mech);
}

#define HEAT_LEVEL_LGREEN 0
#define HEAT_LEVEL_BGREEN 7
#define HEAT_LEVEL_LYELLOW 13
#define HEAT_LEVEL_BYELLOW 16
#define HEAT_LEVEL_LRED 18
#define HEAT_LEVEL_BRED 24
#define HEAT_LEVEL_TOP 40

#define HEAT_LEVEL_NONE 27

static char *MakeHeatScaleInfo(Mech *mech, char *fillchar, char *heatstr,
                               int length) {
  int counter = 0, heat = MechPlusHeat(mech), minheat = MechMinusHeat(mech),
      start = 0;
  char state = 1;

  memset(heatstr, 0, sizeof(char) * length);

  strcat(heatstr, "[fg=black bold]");

  if (minheat > HEAT_LEVEL_NONE)
    start = minheat - HEAT_LEVEL_NONE;

  if (heat <= start) {
    heat = 0;
    state = 0;
  } else
    heat -= start;

  if (start)
    strcat(heatstr, "<[fg=black bold]");
  else
    strcat(heatstr, " [fg=black bold]");

  for (counter = start; counter < minheat; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[fg=green bold]|[reset][fg=green]");
  for (; counter < minheat + HEAT_LEVEL_BGREEN; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LYELLOW; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[reset][fg=yellow bold]|[reset][fg=yellow]");
  for (; counter < minheat + HEAT_LEVEL_BYELLOW; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_LRED; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[reset][fg=red bold]|[reset][fg=red]");
  for (; counter < minheat + HEAT_LEVEL_BRED; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  if (state)
    state++;

  strcat(heatstr, "[bold]");
  for (; counter < minheat + HEAT_LEVEL_TOP; counter++) {
    strncat(heatstr, &fillchar[(short)state], 1);
    if (heat && !--heat)
      state = 0;
  }
  strcat(heatstr, "[fg=white bold]|[reset]");
  return heatstr;
}

void PrintHeatBar(EvaluationContext *evaluation, DbRef player, Mech *mech) {
  char subbuff[256];
  char buff[sizeof(subbuff) + sizeof("Temp:")];
  char heatstr[9] = ".:::::::";

  MakeHeatScaleInfo(mech, heatstr, subbuff, 256);
  snprintf(buff, sizeof(buff), "Temp:%s", subbuff);
  notify(evaluation, player, buff);
}

void PrintInfoStatus(EvaluationContext *evaluation, DbRef player, Mech *mech,
                     int own) {
  char buff[256];
  Mech *tempMech;
  int f;

  switch (MechType(mech)) {
  case CLASS_MECH:
    snprintf(buff, 256,
             "X, Y, Z:%3d,%3d,%3d  Excess Heat:  %3d deg C.  Heat Production:  "
             "%3d deg C.",
             MechX(mech), MechY(mech), MechZ(mech), (int)(10. * MechHeat(mech)),
             (int)(10. * MechPlusHeat(mech)));
    notify(evaluation, player, buff);
    snprintf(buff, 256,
             "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
             "[fg=green bold]%3d[reset] "
             "deg     Heat Sinks:       %3d",
             (int)(MechSpeed(mech)), MechFacing(mech),
             MechActiveNumsinks(mech));
    notify(evaluation, player, buff);
    snprintf(buff, sizeof(buff),
             "Des. Speed: %3d KPH  Des. Heading: %3d deg     Heat Dissipation: "
             "%3d deg C.",
             (int)MechDesiredSpeed(mech), MechDesiredFacing(mech),
             (int)(10. * MechMinusHeat(mech)));
    notify(evaluation, player, buff);

    if (MechLateral(mech))
      notify_printf(evaluation, player, "You are moving laterally %s",
                    LateralDesc(mech));
    break;
  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
  case CLASS_AERO:
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    snprintf(buff, 256,
             "X, Y, Z:%3d,%3d,%3d  Heat Sinks:          %3d       %s",
             MechX(mech), MechY(mech), MechZ(mech), MechActiveNumsinks(mech),
             is_aero(mech)
                 ? tprintf("%s angle: [fg=green bold]%d[reset]",
                           MechDesiredAngle(mech) >= 0 ? "Climbing" : "Diving",
                           abs(MechDesiredAngle(mech)))
                 : "");
    notify(evaluation, player, buff);
    if (FlyingT(mech) || MechMove(mech) == MOVE_SUB) {
      snprintf(
          buff, sizeof(buff),
          "Speed:      [fg=green bold]%3d[reset] KPH  Vertical Speed:      "
          "[fg=green bold]%3d[reset] KPH   Des. Speed %3d KPH",
          (int)(MechSpeed(mech)), (int)(MechVerticalSpeed(mech)),
          (int)(MechDesiredSpeed(mech)));
      notify(evaluation, player, buff);
      f = MAX(0, AeroFuel(mech));
      if (MechMove(mech) == MOVE_SUB) {
        snprintf(buff, sizeof(buff), "Heading: %3d KPH  Des. Heading: %3d deg",
                 (int)MechFacing(mech), MechDesiredFacing(mech));
      } else if (mech_aero_has_free_fuel(mech)) {
        snprintf(buff, sizeof(buff),
                 "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
                 "    %3d "
                 "deg   Fuel: Unlimited",
                 MechFacing(mech), MechDesiredFacing(mech));
      } else {
        snprintf(buff, sizeof(buff),
                 "Heading:    [fg=green bold]%3d[reset] deg  Des. Heading:    "
                 "    %3d "
                 "deg   Fuel: %d (%.2f %%)",
                 MechFacing(mech), MechDesiredFacing(mech), f,
                 100.0 * f / AeroFuelOrig(mech));
      }

      notify(evaluation, player, buff);
    } else if (MechMove(mech) != MOVE_NONE) {
      snprintf(buff, sizeof(buff),
               "Speed:      [fg=green bold]%3d[reset] KPH  Heading:      "
               "[fg=green bold]%3d[reset] deg",
               (int)(MechSpeed(mech)), MechFacing(mech));
      notify(evaluation, player, buff);
      snprintf(buff, sizeof(buff), "Des. Speed: %3d KPH  Des. Heading: %3d deg",
               (int)MechDesiredSpeed(mech), MechDesiredFacing(mech));
      notify(evaluation, player, buff);
    }
    ShowTurretFacing(evaluation, player, 0, mech);
    if (MechHasHeat(mech)) {
      notify_printf(evaluation, player,
                    "Excess Heat:%3d deg  Heat Production:     %3d deg   Heat "
                    "Dissipation: %3d deg",
                    (int)(10. * MechHeat(mech)),
                    (int)(10. * MechPlusHeat(mech)),
                    (int)(10. * MechMinusHeat(mech)));
    }
    break;
  case CLASS_MW:
  case CLASS_BSUIT:
    snprintf(buff, sizeof(buff),
             "X, Y, Z:%3d,%3d,%3d  Speed:      [fg=green bold]%3d[reset] KPH  "
             "Heading:   "
             "   [fg=green bold]%3d[reset] deg",
             MechX(mech), MechY(mech), MechZ(mech), (int)(MechSpeed(mech)),
             MechFacing(mech));
    notify(evaluation, player, buff);
    snprintf(buff, sizeof(buff),
             "                     Des. Speed: %3d KPH  Des. Heading: %3d deg",
             (int)MechDesiredSpeed(mech), MechDesiredFacing(mech));
    notify(evaluation, player, buff);
    break;
  }

  if (MechHasHeat(mech)) {
    PrintHeatBar(evaluation, player, mech);

    // Little extra space to preserve formatting.
    //		if(MechTarget(mech) == -1 && MechTargX(mech) == -1)
    //			notify(evaluation,
    // player, "  ");
  }
  notify(evaluation, player, "  ");
  // Show our locked target info (hex or unit).
  DisplayTarget(evaluation, player, mech);

  if (MechCarrying(mech) > 0)
    if ((tempMech =
             btech_context_get_mech(mech->xcode.context, MechCarrying(mech))))
      notify_printf(evaluation, player, "Towing %s.",
                    mech_to_mech_display_id(mech, tempMech).text);
}

/* Status commands! */
void mech_status(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  EvaluationContext *evaluation = btech_context_evaluation(mech->xcode.context);
  int doweap = 0, doinfo = 0, doarmor = 0, doshort = 0, doheat = 0, loop;
  int i;
  int usex = 0;
  bool weird = false;
  char buf[LBUF_SIZE] = {0};
  char weird_buffer[LBUF_SIZE] = {0};

  cch(MECH_USUALSM);
  if (!buffer || !strlen(buffer))
    // No arguments, we'll go with our default 'status' output.
    doweap = doinfo = doarmor = doheat = 1;
  else {
    // Argument provided, only show certain parts.
    for (loop = 0; buffer[loop]; loop++) {
      switch (toupper(buffer[loop])) {
      case 'R':
        doweap = doinfo = doarmor = doheat = usex = 1;
        break;
      case 'A':
        // Armor status
        if (toupper(buffer[loop + 1]) == 'R')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        doarmor = 1;
        break;
      case 'I':
        // Speed/Heading/Heat
        doinfo = 1;
        if (toupper(buffer[loop + 1]) == 'N')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        break;
      case 'W':
        // Weapons list.
        doweap = 1;
        if (toupper(buffer[loop + 1]) == 'E')
          while (buffer[loop + 1] && buffer[loop + 1] != ' ')
            loop++;
        break;
      case 'N':
        // Really weird status display.
        weird = true;
        break;
      case 'S':
        // Very short one-line status.
        doshort = 1;
        break;
      case 'H':
        // Just the heat bar.
        doheat = 1;
        break;
      }
    }
  }

  // Very short one-line status.
  if (doshort) {
    PrintShortInfo(evaluation, player, mech);
    return;
  }

  // Really weird status display.
  if (weird) {
    snprintf(buf, sizeof(buf), "%s %s %d %d/%d/%d %d ", MechType_Ref(mech),
             MechType_Name(mech), MechTons(mech),
             (int)(MechMaxSpeed(mech) / MP1) * 2 / 3,
             (int)(MechMaxSpeed(mech) / MP1), (int)(MechJumpSpeed(mech) / MP1),
             MechActiveNumsinks(mech));
    memcpy(weird_buffer, buf, sizeof(weird_buffer));

  } else if (!doheat || (doarmor | doinfo | doweap))
    PrintGenericStatus(evaluation, player, mech, 1, usex);

  // Show our armor diagram.
  if (doarmor) {
    if (!weird) {
      PrintArmorStatus(evaluation, player, mech, 1);
      notify(evaluation, player, " ");
    } else {
      for (i = 0; i < NUM_SECTIONS; i++)
        if (GetSectOArmor(mech, i)) {
          if (GetSectORArmor(mech, i))
            snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "%d|%d|%d ",
                     GetSectOArmor(mech, i), GetSectOInt(mech, i),
                     GetSectORArmor(mech, i));
          else
            snprintf(buf + strlen(buf), sizeof(buf) - strlen(buf), "%d|%d ",
                     GetSectOArmor(mech, i), GetSectOInt(mech, i));
        }
    }
  }

  // Standard heat/heading/dive/etc.
  if (doinfo && !weird) {
    PrintInfoStatus(evaluation, player, mech, 1);
    // notify(evaluation, player, " ");
  }

  // Show our heat bar by itself.
  if (!doinfo && doheat && MechHasHeat(mech)) {
    PrintHeatBar(evaluation, player, mech);
  }

  // Weapons readout.
  if (doweap)
    print_weapon_status(evaluation, mech, player, weird, weird_buffer,
                        sizeof(weird_buffer));

  // Really strange, short status info.
  if (weird)
    notify(evaluation, player, weird_buffer);
}
