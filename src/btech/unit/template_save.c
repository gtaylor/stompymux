#include "template_internal.h"

void try_to_find_name(char *mechref, Mech *mech) {
  const char *c;

  if ((c = find_mechname_by_mechref(mechref)))
    strcpy(MechType_Name(mech), c);
}

int DefaultFuelByType(Mech *mech) {
  int mod = 2;

  switch (MechType(mech)) {
  case CLASS_VTOL:
    return 2000 * mod;
  case CLASS_AERO:
    return 1200 * mod;
  case CLASS_DS:
  case CLASS_SPHEROID_DS:
    return 3600 * mod;
  }
  return 0;
}

int save_template(DbRef player, Mech *mech, char *reference, char *filename) {
  FILE *fp;
  int x, x2, inf_x;
  char **locs;
  char *d, *c = ctime(&mech->xcode.context->clock->now);

  if (!MechComputer(mech))
    computer_conversion(mech);
  if (!MechType_Name(mech)[0])
    try_to_find_name(reference, mech);
  if (!(fp = fopen(filename, "w")))
    return -1;
  if (MechType_Name(mech)[0])
    fprintf(fp, "Name             { %s }\n", MechType_Name(mech));
  fprintf(fp, "Reference        { %s }\n", reference);
  fprintf(fp, "Type             { %s }\n", mech_types[(short)MechType(mech)]);
  fprintf(fp, "Unit_Era         { %s }\n", MechUnitEra(mech)),
      fprintf(fp, "Unit_TRO         { %s }\n", MechUnitTRO(mech)),
      fprintf(fp, "Move_Type        { %s }\n",
              move_types[(short)MechMove(mech)]);
  fprintf(fp, "Tons             { %d }\n", MechTons(mech));
  if ((d = strrchr(c, '\n')))
    *d = 0;
  fprintf(fp, "Comment          { Saved by: %s(#%ld) at %s }\n",
          game_object_name(mech->xcode.context->database, player), player, c);
#define SILLY_UTTERANCE(ran, cran, dran, name)                                 \
  if ((!MechComputer(mech) && ran != dran) ||                                  \
      (MechComputer(mech) && ran != cran))                                     \
  fprintf(fp, "%-16s { %d }\n", name, ran)

  SILLY_UTTERANCE(MechTacRange(mech), MechComputersTacRange(mech),
                  DEFAULT_TACRANGE, "Tac_Range");
  SILLY_UTTERANCE(MechLRSRange(mech), MechComputersLRSRange(mech),
                  DEFAULT_LRSRANGE, "LRS_Range");
  SILLY_UTTERANCE(MechScanRange(mech), MechComputersScanRange(mech),
                  DEFAULT_SCANRANGE, "Scan_Range");
  SILLY_UTTERANCE(MechRadioRange(mech), MechComputersRadioRange(mech),
                  DEFAULT_RADIORANGE, "Radio_Range");

#define SILLY_OUTPUT(def, now, name)                                           \
  if ((def) != (now))                                                          \
  fprintf(fp, "%-16s { %d }\n", name, now)

  SILLY_OUTPUT(DEFAULT_COMPUTER, MechComputer(mech), "Computer");
  SILLY_OUTPUT(DEFAULT_RADIO, MechRadio(mech), "Radio");
  SILLY_OUTPUT(0, MechHSEngOverRide(mech), "HSEngOverRide");
  SILLY_OUTPUT((MechSpecials(mech) & ICE_TECH) ? 0 : DEFAULT_HEATSINKS,
               MechRealNumsinks(mech), "Heat_Sinks");
  SILLY_OUTPUT(
      generic_radio_type(MechRadio(mech), MechSpecials(mech) & CLAN_TECH),
      MechRadioType(mech), "RadioType");
  SILLY_OUTPUT(2000, MechBV(mech), "Mech_BV");
  SILLY_OUTPUT(2000, CargoSpace(mech), "Cargo_Space");
  SILLY_OUTPUT(0, CarMaxTon(mech), "Max_Ton");
  SILLY_OUTPUT(2000, MechMaxSuits(mech), "Max_Suits");
  SILLY_OUTPUT(0, AeroSIOrig(mech), "SI");

  SILLY_OUTPUT(DefaultFuelByType(mech), AeroFuelOrig(mech), "Fuel");

  fprintf(fp, "Max_Speed        { %.2f }\n", MechMaxSpeed(mech));
  if (MechJumpSpeed(mech) > 0.0)
    fprintf(fp, "Jump_Speed       { %.2f }\n", MechJumpSpeed(mech));
  x = MechSpecials(mech);
  x2 = MechSpecials2(mech);
  /* Remove AMS'es, they're re-generated back on loadtime */
  x &= ~(CL_ANTI_MISSILE_TECH | IS_ANTI_MISSILE_TECH | SS_ABILITY);
  x &= /* Calculated at load-time */
      ~(BEAGLE_PROBE_TECH | TRIPLE_MYOMER_TECH | MASC_TECH | ECM_TECH |
        C3_SLAVE_TECH | C3_MASTER_TECH | ARTEMIS_IV_TECH | ES_TECH | FF_TECH |
        LIGHT_BAP_TECH);

  if (MechType(mech) == CLASS_MECH)
    x &= ~(XL_TECH | XXL_TECH | CE_TECH | LE_TECH);

  /* Get rid of our specials2 */
  x2 &= ~(STEALTH_ARMOR_TECH | NULLSIGSYS_TECH | ANGEL_ECM_TECH |
          HVY_FF_ARMOR_TECH | LT_FF_ARMOR_TECH | TAG_TECH | C3I_TECH |
          BLOODHOUND_PROBE_TECH | TCOMP_TECH);

  if (x || x2)
    fprintf(fp, "Specials         { %s }\n",
            build_bit_string2(specials, specials2, x, x2,
                              (char[BTECH_TEXT_CAPACITY]){0}));

  inf_x = MechInfantrySpecials(mech);

  if (inf_x)
    fprintf(fp, "InfantrySpecials { %s }\n",
            build_bit_string(infantry_specials, inf_x,
                             (char[BTECH_TEXT_CAPACITY]){0}));

  if ((locs = (char **)ProperSectionStringFromType(MechType(mech),
                                                   MechMove(mech)))) {
    dump_locations(fp, mech, (const char **)locs);
    fclose(fp);
    return 0;
  }
  fclose(fp);
  return -1;
}

static void skip_template_whitespace(FILE *fp) {
  int c;

  while ((c = fgetc(fp)) != EOF && isspace((unsigned char)c))
    ;
  if (c != EOF)
    ungetc(c, fp);
}

char *read_desc(FILE *fp, char *data, char *buffer) {
  char keep[MAX_STRING_LENGTH + 500];
  char *t, *tmp;
  char *point;

  keep[0] = '\0';
  if (data && (tmp = strchr(data, '{'))) {
    skip_template_whitespace(fp);
    while (isspace(*(++tmp)))
      ;
    if ((t = strchr(tmp, '}'))) {
      while (isspace(*(t--)))
        ;
      *(t++) = '\0';
      strcpy(buffer, tmp);
      return buffer;
    } else {
      strcpy(keep, tmp);
      strcat(keep, "\r\n");
      t = tmp + strlen(tmp) - 1;
      while (fgets(data, 512, fp)) {
        skip_template_whitespace(fp);
        if ((tmp = strchr(data, '}')) != NULL) {
          *tmp = 0;
          strcat(keep, data);
          break;
        } else {
          point = data + strlen(data) - 1;
          *(point++) = '\r';
          *(point++) = '\n';
          *point = '\0';
          strcat(keep, data);
        }
      }
    }
  }
  strcpy(buffer, keep);
  return buffer;
}

int find_section(char *cmd, int type, int mtype) {
  char section[20];
  char *ch;
  char **locs;

  strcpy(section, cmd);
  for (ch = section; *ch; ch++)
    if (*ch == '_')
      *ch = ' ';
  locs = (char **)ProperSectionStringFromType(type, mtype);
  return compare_array((char **)locs, section);
  return -1; // Who's retarded? not me!
}

long BuildBitVector(char **list, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf);
    if ((temp = compare_array(list, buf)) == -1)
      return -1;
    bv |= 1U << temp;
  }
  return bv;
}

long BuildBitVectorWithDelim(char **list, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg_delim(line, buf);

    if ((temp = compare_array(list, buf)) == -1)
      return -1;

    bv |= 1U << temp;
  }

  return bv;
}

long BuildBitVectorNoErr(char **list, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf);

    if ((temp = compare_array(list, buf)) != -1)
      bv |= 1U << temp;
  }

  return bv;
}

int CheckSpecialsList(char **special_list, char **special_list2, char *line) {
  int wSpecCheck = -1, wSpec2Check = -1;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf);

    if (special_list)
      wSpecCheck = compare_array(special_list, buf);

    if (special_list2)
      wSpec2Check = compare_array(special_list2, buf);

    if ((wSpecCheck == -1) && (wSpec2Check == -1))
      return 0;
  }

  return 1;
}

int WeaponIFromString(char *data) {
  int x = 0;
  ;

  while (MechWeapons[x].name) {
    if (!strcasecmp(MechWeapons[x].name, data))
      return x + 1; /* weapons start at 1 not 0 */
    x++;
  }
  return -1;
}

int AmmoIFromString(char *data) {
  int x = 0;
  char *ptr;

  ptr = data;
  while (*ptr != '_')
    ptr++;
  ptr++;
  while (MechWeapons[x].name) {
    if (!strcasecmp(MechWeapons[x].name, ptr))
      return x + 101;
    x++;
  }
  return -1;
}

void update_specials(Mech *mech) {
  int x, y, t;
  int masc_count = 0;
  int c3_master_count = 0;
  int tsm_count = 0;
  int ff_count = 0;
  int es_count = 0;
  int tc_count = 0;
  int awcSthArmor[NUM_SECTIONS];
  int awcNSS[NUM_SECTIONS];
  int wcSthArmor = 0;
  int wcNSS = 0;
  int wcAngel = 0;
  int cl = MechSpecials(mech) & CLAN_TECH;
  int e_count = 0;
  int tTechOK = 1;
  int wcHvyFF = 0;
  int wcLtFF = 0;
  int wcC3i = 0;
  int wcBloodhound = 0;
  int awInfSpec[5];
  int wcSuits = 0;

  MechSpecials(mech) &=
      ~(BEAGLE_PROBE_TECH | TRIPLE_MYOMER_TECH | MASC_TECH | ECM_TECH |
        C3_SLAVE_TECH | C3_MASTER_TECH | ARTEMIS_IV_TECH | ES_TECH | FF_TECH |
        IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH | LIGHT_BAP_TECH);
  if (MechType(mech) == CLASS_MECH)
    MechSpecials(mech) &= ~(XL_TECH | XXL_TECH | CE_TECH | LE_TECH);

  MechSpecials2(mech) &=
      ~(STEALTH_ARMOR_TECH | NULLSIGSYS_TECH | ANGEL_ECM_TECH |
        HVY_FF_ARMOR_TECH | LT_FF_ARMOR_TECH | TAG_TECH | C3I_TECH |
        BLOODHOUND_PROBE_TECH | TCOMP_TECH);

  MechInfantrySpecials(mech) &=
      ~(CS_PURIFIER_STEALTH_TECH | DC_KAGE_STEALTH_TECH |
        FWL_ACHILEUS_STEALTH_TECH | FC_INFILTRATOR_STEALTH_TECH |
        FC_INFILTRATORII_STEALTH_TECH);

  for (x = 0; x < 5; x++)
    awInfSpec[x] = 0;

  for (x = 0; x < NUM_SECTIONS; x++) {
    e_count = 0;
    MechSections(mech)[x].config &= ~CASE_TECH;
    awcSthArmor[x] = 0;
    awcNSS[x] = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++)
      if ((t = GetPartType(mech, x, y))) {
        switch (Special2I(t)) {
#define TECHC(item, name)                                                      \
  case item:                                                                   \
    name++;                                                                    \
    break;
#define TECHCU(item, name)                                                     \
  case item:                                                                   \
    if (!PartIsNonfunctional(mech, x, y))                                      \
      name++;                                                                  \
    break;
#define TECH(item, name)                                                       \
  case item:                                                                   \
    MechSpecials(mech) |= name;                                                \
    break
#define TECH2(item, name)                                                      \
  case item:                                                                   \
    MechSpecials2(mech) |= name;                                               \
    break
#define TECHU(item, name)                                                      \
  case item:                                                                   \
    if (!PartIsNonfunctional(mech, x, y))                                      \
      MechSpecials(mech) |= name;                                              \
    break
          TECH(ARTEMIS_IV, ARTEMIS_IV_TECH);
          TECHU(BEAGLE_PROBE, BEAGLE_PROBE_TECH);
          TECHU(LIGHT_BAP, LIGHT_BAP_TECH);
          TECH(ECM, ECM_TECH);
          TECH2(TAG, TAG_TECH);
          TECHU(C3_SLAVE, C3_SLAVE_TECH);
          TECHCU(MASC, masc_count);
          TECHC(C3_MASTER, c3_master_count);
          TECHCU(C3I, wcC3i);
          TECHCU(ANGELECM, wcAngel);
          TECHC(TRIPLE_STRENGTH_MYOMER, tsm_count);
          TECHC(FERRO_FIBROUS, ff_count);
          TECHC(HVY_FERRO_FIBROUS, wcHvyFF);
          TECHC(LT_FERRO_FIBROUS, wcLtFF);
          TECHC(BLOODHOUND_PROBE, wcBloodhound);
          TECHCU(TARGETING_COMPUTER, tc_count);
          TECHC(ENDO_STEEL, es_count);
          TECHC(PURIFIER_ARMOR, awInfSpec[0]);
          TECHCU(KAGE_STEALTH_UNIT, awInfSpec[1]);
          TECHCU(ACHILEUS_STEALTH_UNIT, awInfSpec[2]);
          TECHCU(INFILTRATOR_STEALTH_UNIT, awInfSpec[3]);
          TECHCU(INFILTRATORII_STEALTH_UNIT, awInfSpec[4]);
        case ENGINE:
          e_count++;
          break;
        case CASE:
          MechSections(mech)[(MechType(mech) == CLASS_VEH_GROUND) ? BSIDE : x]
              .config |= CASE_TECH;
          break;
        case STEALTH_ARMOR:
          awcSthArmor[x]++;
          wcSthArmor++;
          break;
        case NULL_SIGNATURE_SYSTEM:
          awcNSS[x]++;
          wcNSS++;
          break;
        }
        if (IsWeapon(t) && IsAMS(Weapon2I(t))) {
          if (MechWeapons[Weapon2I(t)].special & CLAT)
            MechSpecials(mech) |= CL_ANTI_MISSILE_TECH;
          else
            MechSpecials(mech) |= IS_ANTI_MISSILE_TECH;
        }
      }
    if (x != CTORSO && e_count) {
      if (e_count > 3)
        MechSpecials(mech) |= XXL_TECH;

      else if (e_count == 2)
        if (cl)
          MechSpecials(mech) |= XL_TECH;

        else
          MechSpecials(mech) |= LE_TECH;

      else
        MechSpecials(mech) |= XL_TECH;
    } else {
      if (x == CTORSO && e_count < 4 && MechType(mech) == CLASS_MECH)
        MechSpecials(mech) |= CE_TECH;
    }
  }
  if ((MechSpecials(mech) & (XXL_TECH | XL_TECH | LE_TECH)) &&
      (MechSpecials(mech) & CE_TECH))
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("#%ld apparently is very weird: Compact engine AND XL/XXL?",
                mech->mynum));
  if (tc_count) {
    MechSpecials2(mech) |= TCOMP_TECH;
    for (x = 0; x < NUM_SECTIONS; x++)
      for (y = 0; y < CritsInLoc(mech, x); y++)
        if (IsWeapon((t = GetPartType(mech, x, y))))
          if (TCAble(t))
            GetPartFireMode(mech, x, y) |= ON_TC;
  }
  if (masc_count >= MAX(1, (MechTons(mech) / (cl ? 25 : 20))))
    MechSpecials(mech) |= MASC_TECH;
#define ITech(var, cnt, spec)                                                  \
  if (((var)) >= ((cnt)) || (MechType(mech) != CLASS_MECH && ((var) > 0)))     \
  MechSpecials(mech) |= spec

#define ITech2(var, cnt, spec)                                                 \
  if (((var)) >= ((cnt)) || (MechType(mech) != CLASS_MECH && ((var) > 0)))     \
  MechSpecials2(mech) |= spec

  ITech(ff_count, (cl ? 7 : 14), FF_TECH);
  ITech(es_count, (cl ? 7 : 14), ES_TECH);
  ITech(tsm_count, 6, TRIPLE_MYOMER_TECH);
  ITech2(wcAngel, 2, ANGEL_ECM_TECH);
  ITech2(wcHvyFF, 21, HVY_FF_ARMOR_TECH);
  ITech2(wcLtFF, 7, LT_FF_ARMOR_TECH);
  ITech2(wcC3i, 2, C3I_TECH);
  ITech2(wcBloodhound, 3, BLOODHOUND_PROBE_TECH);

  if (MechType(mech) == CLASS_MECH) {
    /* Be 'noisy' about some crits/techs */
    if ((ff_count > 0) && (ff_count < (cl ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing FF Crits %d/%d!",
                                 MechType_Ref(mech), mech->mynum, ff_count,
                                 (cl ? 7 : 14)));

    if ((es_count > 0) && (es_count < (cl ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing ES Crits %d/%d!",
                                 MechType_Ref(mech), mech->mynum, es_count,
                                 (cl ? 7 : 14)));

    if ((tsm_count > 0) && (tsm_count < 6))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing TSM Crits %d/6!",
                                 MechType_Ref(mech), mech->mynum, tsm_count));

    if ((wcHvyFF > 0) && (wcHvyFF < 21))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing HvyFF Crits %d/21!",
                                 MechType_Ref(mech), mech->mynum, wcHvyFF));

    if ((wcLtFF > 0) && (wcLtFF < 7))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing LtFF Crits %d/7!",
                                 MechType_Ref(mech), mech->mynum, wcLtFF));
  }

  /*
   * Check our NSS. Need 1 crit in each loc except H
   */
  tTechOK = 0;

  if (wcNSS > 0) {
    tTechOK = 1;

    if (MechType(mech) != CLASS_MECH) {
      if (wcNSS < 1)
        tTechOK = 0;
    } else {
      for (x = 0; x < NUM_SECTIONS; x++) {
        if (x != HEAD) {
          if (awcNSS[x] < 1) {
            tTechOK = 0;
            break;
          }
        }
      }
    }

    if (tTechOK)
      MechSpecials2(mech) |= NULLSIGSYS_TECH;
  }

  /*
   * Check our Stealth armor. Need 2 crits in each loc except H and CT
   */
  tTechOK = 0;

  if (wcSthArmor > 0) {
    tTechOK = 1;

    if (!(MechSpecials(mech) & ECM_TECH)) {
      tTechOK = 0;
    } else {
      if (MechType(mech) != CLASS_MECH) {
        if (wcSthArmor < 1)
          tTechOK = 0;
      } else {
        for (x = 0; x < NUM_SECTIONS; x++) {
          if ((x != HEAD) && (x != CTORSO)) {
            if (awcSthArmor[x] < 2) {
              tTechOK = 0;
              break;
            }
          }
        }
      }
    }

    if (tTechOK)
      MechSpecials2(mech) |= STEALTH_ARMOR_TECH;
  }

  /* Let's do our suit checks */
  if (MechType(mech) == CLASS_BSUIT) {
    wcSuits = bsuit_member_count(mech);

    if (awInfSpec[0] >= wcSuits)
      MechInfantrySpecials(mech) |= CS_PURIFIER_STEALTH_TECH;

    if (awInfSpec[1] >= wcSuits)
      MechInfantrySpecials(mech) |= DC_KAGE_STEALTH_TECH;

    if (awInfSpec[2] >= wcSuits)
      MechInfantrySpecials(mech) |= FWL_ACHILEUS_STEALTH_TECH;

    if (awInfSpec[3] >= wcSuits)
      MechInfantrySpecials(mech) |= FC_INFILTRATOR_STEALTH_TECH;

    if (awInfSpec[4] >= wcSuits)
      MechInfantrySpecials(mech) |= FC_INFILTRATORII_STEALTH_TECH;
  }

  /* New C3 Master code */
  if (c3_master_count > 0) {
    MechTotalC3Masters(mech) = mech_c3_total_master_count(mech);
    MechWorkingC3Masters(mech) = mech_c3_working_master_count(mech);

    if (MechTotalC3Masters(mech) > 0)
      MechSpecials(mech) |= C3_MASTER_TECH;

    if (MechWorkingC3Masters(mech) == 0)
      MechCritStatus(mech) |= C3_DESTROYED;
    else
      MechCritStatus(mech) &= ~C3_DESTROYED;
  }
}

int update_oweight(Mech *mech, int value) {
  MechCritStatus(mech) |= OWEIGHT_OK;

  /* Check to prevent silliness */
  if (!mech->xcode.context->configuration->btech_dynspeed ||
      (value == 1 && !Destroyed(mech)))
    value = MechTons(mech) * 1024;
  MechRTonsV(mech) = value;
  return value;
}

int mech_calculated_weight(Mech *mech) {
  if (MechCritStatus(mech) & OWEIGHT_OK)
    return MechRTonsV(mech);
  return update_oweight(mech, mech_weight_sub(GOD, mech, -1));
}
