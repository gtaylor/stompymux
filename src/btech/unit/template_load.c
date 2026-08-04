#include "template_internal.h"

static bool template_load_error(FILE *fp, Mech *mech, DbRef player,
                                bool condition, bool global, const char *format,
                                ...) __attribute__((format(printf, 6, 7)));

static bool template_load_error(FILE *fp, Mech *mech, DbRef player,
                                bool condition, bool global, const char *format,
                                ...) {
  if (!condition) {
    return false;
  }
#ifdef TEMPLATE_VERBOSE_ERRORS
  char message[LBUF_SIZE] = {0};
  va_list args;
  va_start(args, format);
  vsnprintf(message, sizeof(message), format, args);
  va_end(args);
  if (global) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                       message);
  } else {
    notify(btech_context_evaluation(mech->xcode.context), player, message);
  }
#else
  (void)mech;
  (void)player;
  (void)global;
  (void)format;
#endif
  if (fp) {
    fclose(fp);
  }
  return true;
}

int load_template(DbRef player, Mech *mech, char *filename) {
  char line[MAX_STRING_LENGTH], buf[MAX_STRING_LENGTH];
  int x, y, value, i;
  char cmd[MAX_STRING_LENGTH];
  char *ptr, *j, *k, *line2;
  int section = 0, critical, selection, type, brand;
  FILE *fp = fopen(filename, "r");
  char *tmpc;
  int lpos, hpos;
  int ok_count = 0;
  int isClan = 0;
  int t;
  int wFireModes, wAmmoModes;
  BattleMap *map;

  if (!fp)
    return -1;

  ptr = strrchr(filename, '/');
  if (ptr == NULL) {
    ptr = filename;
  } else {
    ptr++;
  }
  strncpy(MechType_Ref(mech), ptr, 25);
  MechType_Ref(mech)[24] = '\0';

  silly_atr_set_in(mech->xcode.context->database, mech->mynum, A_MECHREF,
                   MechType_Ref(mech));
  MechRadioType(mech) = 0;
  while (fgets(line, 512, fp)) {
    line[strlen(line) - 1] = '\0';
    j = line;
    while (isspace(*j))
      j++;
    if (j != line)
      memmove(line, j, strlen(j) + 1);
    if ((ptr = strchr(line, ' '))) {
      if ((tmpc = strchr(line, '\t')) < ptr)
        if (tmpc)
          ptr = tmpc;
      j = line;
      k = cmd;
      while (j != ptr)
        *(k++) = *(j++);
      *k = 0;
      for (ptr++; isspace(*ptr); ptr++)
        ;
    } else {
      strcpy(cmd, line);
      strcpy(line, "");
      ptr = NULL;
    }
    if (!strncasecmp(cmd, "CRIT_", 5))
      selection = 9999;
    else if ((selection = compare_array(load_cmds, cmd)) == -1) {
      /* Initial premise: we will have a mech type before we get to this */
      section = find_section(cmd, MechType(mech), MechMove(mech));
      if (template_load_error(
              fp, mech, player, section == -1 && !ok_count, false,
              "New template loading system: %s is invalid template file.",
              filename)) {
        return -1;
      }
      if (template_load_error(fp, mech, player, section == -1, true,
                              "Error while loading: Section %s not found.",
                              cmd)) {
        return -1;
      }
      MechSections(mech)[section].recycle = 0;
      ok_count++;
      continue;
    }
    ok_count++;
    switch (selection) {
    case 0: /* Reference */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strcmp(tmpc, MechType_Ref(mech))) {
        btech_channel_send(
            mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
            tprintf("Template %s has Reference <-> Filename mismatch : %s <-> "
                    "%s - It is automatically fixed by saving again.",
                    filename, tmpc, MechType_Ref(mech)));
        tmpc = MechType_Ref(mech);
      }
      silly_atr_set_in(mech->xcode.context->database, mech->mynum, A_MECHREF,
                       tmpc);
      strcpy(MechType_Ref(mech), tmpc);
      break;
    case 1: /* Type */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      MechType(mech) = compare_array(mech_types, tmpc);
      if (template_load_error(fp, mech, player, MechType(mech) == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      AeroFuel(mech) = AeroFuelOrig(mech) = DefaultFuelByType(mech);
      break;
    case 2: /* Movement Type */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      MechMove(mech) = compare_array(move_types, tmpc);
      if (template_load_error(fp, mech, player, MechMove(mech) == -1, true,
                              "Error while loading: Type %s not found.",
                              tmpc)) {
        return -1;
      }
      break;
    case 3: /* Tons */
      MechTons(mech) = atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 4: /* Tac_Range */
      MechTacRange(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 5: /* LRS_Range */
      MechLRSRange(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 6: /* Radio Range */
      MechRadioRange(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 7: /* Scan Range */
      MechScanRange(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 8: /* Heat Sinks */
      MechRealNumsinks(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 9: /* Max Speed */
      mech_max_speed_set(
          mech, atof(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      TemplateMaxSpeed(mech) = MechMaxSpeed(mech);
      break;
    case 10: /* Specials */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});

      if (CheckSpecialsList(specials, specials2, tmpc)) {
        MechSpecials(mech) |= BuildBitVectorNoErr(specials, tmpc);
        MechSpecials2(mech) |= BuildBitVectorNoErr(specials2, tmpc);
      } else if (template_load_error(
                     fp, mech, player, MechSpecials(mech) == -1, true,
                     "Error while loading: Invalid specials - %s.", tmpc)) {
        return -1;
      }
      break;
    case 11: /* Armor */
      SetSectOArmor(mech, section,
                    atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      SetSectArmor(mech, section, GetSectOArmor(mech, section));
      break;
    case 12: /* Internals */
      SetSectOInt(mech, section,
                  atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      SetSectInt(mech, section, GetSectOInt(mech, section));
      break;
    case 13: /* Rear */
      SetSectORArmor(mech, section,
                     atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0})));
      SetSectRArmor(mech, section, GetSectORArmor(mech, section));
      break;
    case 14: /* Config */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      MechSections(mech)[section].config =
          BuildBitVector(section_configs, tmpc) &
          ~(CASE_TECH | SECTION_DESTROYED);
      if (template_load_error(
              fp, mech, player, MechSections(mech)[section].config == -1, true,
              "Error while loading: Invalid location config: %s.", tmpc)) {
        return -1;
      }
      break;
    case 9999:
      if ((sscanf(cmd, "CRIT_%d-%d", &x, &y)) == 2) {
        lpos = x - 1;
        hpos = y - 1;
      } else if ((sscanf(cmd, "CRIT_%d", &x)) == 1) {
        lpos = x - 1;
        hpos = x - 1;
      } else
        break;
      critical = lpos;
      line2 = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      line2 = one_arg(line2, buf);
      if (!strncasecmp(buf, "CL.", 3))
        isClan = 1;
      if (template_load_error(fp, mech, player,
                              !find_matching_vlong_part(mech->xcode.context,
                                                        buf, NULL, &type,
                                                        &brand),
                              true, "Unable to find %s", buf)) {
        return -1;
      }
      SetPartType(mech, section, critical, type);
      if (!mech->xcode.context->configuration->btech_parts)
        brand = 0;
      SetPartBrand(mech, section, critical, brand);
      SetPartDesiredAmmoLoc(mech, section, critical, -1);

      if (IsWeapon(type)) {
        /* Thanks to legacy of past, we _do_ have to do this.. sniff */
        if (IsAMS(Weapon2I(type))) {
          if (MechWeapons[Weapon2I(type)].special & CLAT)
            MechSpecials(mech) |= CL_ANTI_MISSILE_TECH;
          else
            MechSpecials(mech) |= IS_ANTI_MISSILE_TECH;
        }
        SetPartData(mech, section, critical, 0);
        line2 = one_arg(line2, buf); /* Don't need the '-' */
        line2 = one_arg(line2, buf);

        /*              wFireModes = BuildBitVector(crit_fire_modes, buf); */

        /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */

        wFireModes = BuildBitVectorWithDelim(crit_fire_modes, buf);
        wAmmoModes = BuildBitVectorWithDelim(crit_ammo_modes, buf);

        if (template_load_error(
                fp, mech, player, wFireModes < 0 && wAmmoModes < 0, true,
                "Error while loading: Invalid crit modes for weapon: %s.",
                buf)) {
          return -1;
        }

        if (wFireModes < 0)
          wFireModes = 0;

        if (wAmmoModes < 0)
          wAmmoModes = 0;

        GetPartFireMode(mech, section, critical) = wFireModes;
        GetPartAmmoMode(mech, section, critical) = wAmmoModes;

        line2 = one_arg(line2, buf);
        if (mech->xcode.context->configuration->btech_parts)
          if (atoi(buf)) {
            SetPartBrand(mech, section, critical, atoi(buf));
          }
      } else if (IsAmmo(type)) {
        line2 = one_arg(line2, buf);
        GetPartData(mech, section, critical) = atoi(buf);
        line2 = one_arg(line2, buf);

        /*              wFireModes = BuildBitVector(crit_fire_modes, buf); */

        /*              wAmmoModes = BuildBitVector(crit_ammo_modes, buf); */

        wFireModes = BuildBitVectorWithDelim(crit_fire_modes, buf);
        wAmmoModes = BuildBitVectorWithDelim(crit_ammo_modes, buf);

        if (template_load_error(
                fp, mech, player, wFireModes < 0 && wAmmoModes < 0, true,
                "Error while loading: Invalid crit modes for ammo: %s.", buf)) {
          return -1;
        }

        if (wFireModes < 0)
          wFireModes = 0;

        if (wAmmoModes < 0)
          wAmmoModes = 0;

        GetPartFireMode(mech, section, critical) = wFireModes;
        GetPartAmmoMode(mech, section, critical) = wAmmoModes;

        if (GetPartData(mech, section, critical) <
            FullAmmo(mech, section, critical)) {
          GetPartFireMode(mech, section, critical) |= HALFTON_MODE;
          if (GetPartData(mech, section, critical) >
              FullAmmo(mech, section, critical))
            GetPartFireMode(mech, section, critical) &= ~HALFTON_MODE;
        }

        if (GetPartData(mech, section, critical) !=
                FullAmmo(mech, section, critical) &&
            MechType(mech) != CLASS_MW && MechType(mech) != CLASS_BSUIT) {
          btech_channel_send(
              mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
              tprintf("Invalid ammo crit for %s in #%ld %s (%d/%d)",
                      MechWeapons[Ammo2I(type)].name, mech->mynum, filename,
                      GetPartData(mech, section, critical),
                      FullAmmo(mech, section, critical)));
          SetPartData(mech, section, critical,
                      FullAmmo(mech, section, critical));
        }
      } else {
        if ((line2 = one_arg(line2, buf)))
          GetPartData(mech, section, critical) = atoi(buf);
        else
          GetPartData(mech, section, critical) = 0;
        GetPartFireMode(mech, section, critical) = 0;
        GetPartAmmoMode(mech, section, critical) = 0;
        if ((line2 = one_arg(line2, buf)))
          if ((line2 = one_arg(line2, buf))) {
            if (mech->xcode.context->configuration->btech_parts)
              if (atoi(buf)) {
                SetPartBrand(mech, section, critical, atoi(buf));
              }
          }
      }
      for (x = (lpos + 1); x <= hpos; x++) {
        SetPartType(mech, section, x, GetPartType(mech, section, lpos));
        SetPartData(mech, section, x, GetPartData(mech, section, lpos));
        SetPartFireMode(mech, section, x, GetPartFireMode(mech, section, lpos));
        SetPartAmmoMode(mech, section, x, GetPartAmmoMode(mech, section, lpos));
        SetPartBrand(mech, section, x, GetPartBrand(mech, section, lpos));
      }
      break;
    case 15: /* Mech's Computer level */
      MechComputer(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 16: /* Name of the mech */
      strcpy(MechType_Name(mech),
             read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 17: /* Jj's */
      MechJumpSpeed(mech) =
          atof(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 18: /* Radio */
      MechRadio(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 19: /* SI */
      AeroSI(mech) = AeroSIOrig(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 20: /* Fuel */
      AeroFuel(mech) = AeroFuelOrig(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 21: /* Comment */
      break;
    case 22: /* Radio_freqs */
      MechRadioType(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 23: /* Mech battle value */
      MechBV(mech) = atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 24: /* Cargospace */
      CargoSpace(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 25: /* Maxsuits */
      MechMaxSuits(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 26: /* Specials */
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});

      if (CheckSpecialsList(infantry_specials, 0, tmpc))
        MechInfantrySpecials(mech) |=
            BuildBitVectorNoErr(infantry_specials, tmpc);

      break;
    case 27: /* Carmaxton */
      CarMaxTon(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 28:
      MechHSEngOverRide(mech) =
          atoi(read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0}));
      break;
    case 29:
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strlen(tmpc) == 1) /* just the \0 */
        tmpc = "Undefined";
      strcpy(MechUnitEra(mech), tmpc);
      break;
    case 30:
      tmpc = read_desc(fp, ptr, (char[BTECH_TEXT_CAPACITY]){0});
      if (strlen(tmpc) == 1) /* just the \0 */
        tmpc = "Undefined";
      strcpy(MechUnitTRO(mech), tmpc);
      break;
    }
  }
  fclose(fp);
  MechEngineSizeV(mech) = MechEngineSizeC(mech);
  /* So we're not getting 'blank' ERA/TRO values, we'll default to 'Undefined'
   */
  if (strlen(MechUnitEra(mech)) == 0) {
    tmpc = "Undefined";
    strcpy(MechUnitEra(mech), tmpc);
  }
  if (strlen(MechUnitTRO(mech)) == 0) {
    tmpc = "Undefined";
    strcpy(MechUnitTRO(mech), tmpc);
  }
#define Set(a, b)                                                              \
  if (!(a))                                                                    \
  a = b
  if (!(MechSpecials(mech) & ICE_TECH))
    Set(MechRealNumsinks(mech), DEFAULT_HEATSINKS);
  if (MechType(mech) == CLASS_MECH)
    do_sub_magic(mech, 1);
  if (MechType(mech) == CLASS_MW)
    mech_power_up(mech);

  if (MechType(mech) == CLASS_MECH)
    value = 8;
  else
    value = 6;

  if (mech->xcode.context->configuration->btech_parts)
    for (x = 0; x < value; x++)
      for (y = 0; y < CritsInLoc(mech, x); y++)
        if ((t = GetPartType(mech, x, y))) {
          if (GetPartBrand(mech, x, y))
            continue;
          if (IsAmmo(t))
            continue;
          if (IsBomb(t))
            continue;
          SetPartBrand(mech, x, y,
                       isClan ? DEFAULT_CLPART_LEVEL : DEFAULT_PART_LEVEL);
        }
  if (isClan) {
    Set(MechComputer(mech), DEFAULT_CLCOMPUTER);
    Set(MechRadio(mech), DEFAULT_CLRADIO);
  } else {
    Set(MechComputer(mech), DEFAULT_COMPUTER);
    Set(MechRadio(mech), DEFAULT_RADIO);
  }
  if (!MechRadioType(mech))
    MechRadioType(mech) = generic_radio_type(MechRadio(mech), isClan);
  if (!MechComputer(mech)) {
    Set(MechScanRange(mech), DEFAULT_SCANRANGE);
    Set(MechLRSRange(mech), DEFAULT_LRSRANGE);
    Set(MechRadioRange(mech), DEFAULT_RADIORANGE);
    Set(MechTacRange(mech), DEFAULT_TACRANGE);
  } else {
    Set(MechScanRange(mech), MechComputersScanRange(mech));
    Set(MechLRSRange(mech), MechComputersLRSRange(mech));
    Set(MechRadioRange(mech), MechComputersRadioRange(mech));
    Set(MechTacRange(mech), MechComputersTacRange(mech));
  }
#if 1 /* Don't know if we're ready for this yet - aw, what the hell :) */
  MechSpecials(mech) &= ~FLIPABLE_ARMS;
  if (MechType(mech) == CLASS_MECH)
    if ((GetPartType(mech, LARM, 2) != Special(LOWER_ACTUATOR)) &&
        (GetPartType(mech, RARM, 2) != Special(LOWER_ACTUATOR)) &&
        (GetPartType(mech, LARM, 3) != Special(HAND_OR_FOOT_ACTUATOR)) &&
        (GetPartType(mech, RARM, 3) != Special(HAND_OR_FOOT_ACTUATOR)))
      MechSpecials(mech) |= FLIPABLE_ARMS;
#endif
  update_specials(mech);
  MechXPMod(mech) = 1.0;     /* Default it to 1 (no mod effect at all) */
  MechUnitsKilled(mech) = 0; /* Clear the mechs killed */
  mech_int_check(mech, 1);
  x = mech_weight_sub(GOD, mech, 0);
  y = MechTons(mech) * 1024;
  /* While we're at it, lets report those that are overweight */
  if ((x - y) > 40)
    if (MechType(mech) != CLASS_BSUIT && MechMove(mech) != MOVE_NONE)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf(
              "Error in %s template: %.1f tons of 'stuff', yet %d ton frame.",
              MechType_Ref(mech), x / 1024.0, y / 1024));
  update_oweight(mech, x);
  if ((map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
    UpdateConditions(mech, map);
  /* To prevent certain funny occurences.. */
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (!(GetSectOInt(mech, i))) {
      SetSectDestroyed(mech, i);
    }
  }
  return 0;
}
