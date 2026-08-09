#include "btech_text_builder.h"
#include "mech_electronics_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_network_api.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mux/support/checked_storage.h"
#include "template_implementation.h"
#include "weapon_catalogue_api.h"

static int *template_integer_slot(int *values, size_t count, int index) {
  return checked_storage_at(values, count, sizeof(*values), (size_t)index);
}

void try_to_find_name(char *mechref, Mech *mech) {
  const char *c;

  if ((c = find_mechname_by_mechref(mechref)))
    strlcpy(((mech)->ud.mech_name), c, sizeof(((mech)->ud.mech_name)));
}

int DefaultFuelByType(Mech *mech) {
  int mod = 2;

  switch (((mech)->ud.type)) {
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

static void save_nondefault_range(FILE *fp, const Mech *mech, int current,
                                  int computer_default, int legacy_default,
                                  const char *name) {
  int expected =
      mech_computer_quality(mech) ? computer_default : legacy_default;

  if (current != expected)
    fprintf(fp, "%-16s { %d }\n", name, current);
}

static void save_nondefault_integer(FILE *fp, int expected, int current,
                                    const char *name) {
  if (expected != current)
    fprintf(fp, "%-16s { %d }\n", name, current);
}

int save_template(DbRef player, Mech *mech, char *reference, char *filename) {
  FILE *fp;
  int x, x2, inf_x;
  const char *const *locs;
  char *d, *c = ctime(&mech->xcode.context->clock->now);

  if (!mech_computer_quality(mech))
    computer_conversion(mech);
  if (!((mech)->ud.mech_name)[0])
    try_to_find_name(reference, mech);
  if (!(fp = fopen(filename, "w")))
    return -1;
  if (((mech)->ud.mech_name)[0])
    fprintf(fp, "Name             { %s }\n", ((mech)->ud.mech_name));
  fprintf(fp, "Reference        { %s }\n", reference);
  fprintf(fp, "Type             { %s }\n",
          template_unit_class_name((size_t)mech->ud.type));
  fprintf(fp, "Unit_Era         { %s }\n", ((mech)->ud.unit_era)),
      fprintf(fp, "Unit_TRO         { %s }\n", ((mech)->ud.unit_tro)),
      fprintf(fp, "Move_Type        { %s }\n",
              template_movement_type_name((size_t)mech->ud.move));
  fprintf(fp, "Tons             { %d }\n", ((mech)->ud.tons));
  if ((d = strrchr(c, '\n')))
    *d = 0;
  fprintf(fp, "Comment          { Saved by: %s(#%ld) at %s }\n",
          game_object_name(mech->xcode.context->database, player), player, c);
  save_nondefault_range(fp, mech, mech_tactical_range(mech),
                        mech_default_tactical_range(mech), DEFAULT_TACRANGE,
                        "Tac_Range");
  save_nondefault_range(fp, mech, mech_long_range_sensor_range(mech),
                        mech_default_long_range_sensor_range(mech),
                        DEFAULT_LRSRANGE, "LRS_Range");
  save_nondefault_range(fp, mech, mech_scanner_range(mech),
                        mech_default_scanner_range(mech), DEFAULT_SCANRANGE,
                        "Scan_Range");
  save_nondefault_range(fp, mech, mech_radio_range(mech),
                        mech_default_radio_range(mech), DEFAULT_RADIORANGE,
                        "Radio_Range");

  save_nondefault_integer(fp, DEFAULT_COMPUTER, mech_computer_quality(mech),
                          "Computer");
  save_nondefault_integer(fp, DEFAULT_RADIO, mech_radio_quality(mech), "Radio");
  save_nondefault_integer(fp, 0, ((mech)->ud.hsengoverride), "HSEngOverRide");
  save_nondefault_integer(
      fp, (((mech)->rd.specials) & ICE_TECH) ? 0 : DEFAULT_HEATSINKS,
      ((mech)->ud.numsinks), "Heat_Sinks");
  save_nondefault_integer(fp,
                          generic_radio_type(mech_radio_quality(mech),
                                             ((mech)->rd.specials) & CLAN_TECH),
                          mech_radio_configuration(mech), "RadioType");
  save_nondefault_integer(fp, 2000, ((mech)->ud.mechbv), "Mech_BV");
  save_nondefault_integer(fp, 2000, ((mech)->ud.cargospace), "Cargo_Space");
  save_nondefault_integer(fp, 0, ((mech)->ud.carmaxton), "Max_Ton");
  save_nondefault_integer(fp, 2000, ((mech)->rd.maxsuits), "Max_Suits");
  save_nondefault_integer(fp, 0, ((mech)->ud.si_orig), "SI");
  save_nondefault_integer(fp, DefaultFuelByType(mech), ((mech)->ud.fuel_orig),
                          "Fuel");

  fprintf(fp, "Max_Speed        { %.2f }\n", (double)((mech)->ud.maxspeed));
  if (((mech)->rd.jumpspeed) > 0.0F)
    fprintf(fp, "Jump_Speed       { %.2f }\n", (double)((mech)->rd.jumpspeed));
  x = ((mech)->rd.specials);
  x2 = ((mech)->rd.specials2);
  /* Remove AMS'es, they're re-generated back on loadtime */
  x &= ~(CL_ANTI_MISSILE_TECH | IS_ANTI_MISSILE_TECH | SS_ABILITY);
  x &= /* Calculated at load-time */
      ~(BEAGLE_PROBE_TECH | TRIPLE_MYOMER_TECH | MASC_TECH | ECM_TECH |
        C3_SLAVE_TECH | C3_MASTER_TECH | ARTEMIS_IV_TECH | ES_TECH | FF_TECH |
        LIGHT_BAP_TECH);

  if (((mech)->ud.type) == CLASS_MECH)
    x &= ~(XL_TECH | XXL_TECH | CE_TECH | LE_TECH);

  /* Get rid of our specials2 */
  x2 &= ~(STEALTH_ARMOR_TECH | NULLSIGSYS_TECH | ANGEL_ECM_TECH |
          HVY_FF_ARMOR_TECH | LT_FF_ARMOR_TECH | TAG_TECH | C3I_TECH |
          BLOODHOUND_PROBE_TECH | TCOMP_TECH);

  if (x || x2)
    fprintf(fp, "Specials         { %s }\n",
            build_bit_string2(specials, primary_technology_name_count(),
                              specials2, secondary_technology_name_count(), x,
                              x2, (char[BTECH_TEXT_CAPACITY]){0}));

  inf_x = ((mech)->rd.infantry_specials);

  if (inf_x)
    fprintf(fp, "InfantrySpecials { %s }\n",
            build_bit_string(infantry_specials,
                             infantry_technology_name_count(), inf_x,
                             (char[BTECH_TEXT_CAPACITY]){0}));

  int result = -1;
  if ((locs =
           ProperSectionStringFromType(((mech)->ud.type), ((mech)->ud.move)))) {
    dump_locations(fp, mech, locs,
                   mech_section_name_count(mech->ud.type, mech->ud.move));
    result = 0;
  }
  if (fclose(fp) != 0)
    return -1;
  return result;
}

static void skip_template_whitespace(FILE *fp) {
  int c;

  while ((c = fgetc(fp)) != EOF && (c == ' ' || c == '\t' || c == '\n' ||
                                    c == '\v' || c == '\f' || c == '\r'))
    ;
  if (c != EOF)
    ungetc(c, fp);
}

char *read_desc(FILE *fp, char *data, char *buffer) {
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  char *opening;

  if (data && (opening = strchr(data, '{'))) {
    skip_template_whitespace(fp);
    char *content = checked_mutable_string_suffix(opening, 1);
    content =
        checked_mutable_string_suffix(content, strspn(content, " \t\n\v\f\r"));
    char *closing = strchr(content, '}');
    if (closing) {
      size_t content_length = (size_t)(closing - content);
      while (content_length > 0) {
        const char *last = checked_storage_at_const(
            content, strlen(content) + 1, sizeof(*content), content_length - 1);
        if (!strchr(" \t\n\v\f\r", *last))
          break;
        content_length--;
      }
      btech_text_builder_append_count(&builder, content, content_length);
      return buffer;
    }
    btech_text_builder_append(&builder, content);
    btech_text_builder_append(&builder, "\r\n");
    while (fgets(data, 512, fp)) {
      skip_template_whitespace(fp);
      closing = strchr(data, '}');
      if (closing) {
        btech_text_builder_append_count(&builder, data,
                                        (size_t)(closing - data));
        break;
      }
      size_t length = strlen(data);
      while (length > 0) {
        const char *last = checked_storage_at_const(data, length + 1,
                                                    sizeof(*data), length - 1);
        if (*last != '\n' && *last != '\r')
          break;
        length--;
      }
      btech_text_builder_append_count(&builder, data, length);
      btech_text_builder_append(&builder, "\r\n");
    }
  }
  return buffer;
}

int find_section(char *cmd, int type, int mtype) {
  char section[20];
  const char *const *locs;

  strlcpy(section, cmd, sizeof(section));
  size_t section_length = strlen(section);
  for (size_t index = 0; index < section_length; index++) {
    char *character =
        checked_storage_at(section, sizeof(section), sizeof(*section), index);
    if (*character == '_')
      *character = ' ';
  }
  locs = ProperSectionStringFromType(type, mtype);
  return compare_const_array(locs, mech_section_name_count(type, mtype),
                             section);
}

long BuildBitVector(const char *const list[], size_t count, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf, sizeof(buf));
    if ((temp = compare_const_array(list, count, buf)) == -1)
      return -1;
    bv |= 1U << temp;
  }
  return bv;
}

long BuildBitVectorWithDelim(const char *const list[], size_t count,
                             char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg_delim(line, buf, sizeof(buf));

    if ((temp = compare_const_array(list, count, buf)) == -1)
      return -1;

    bv |= 1U << temp;
  }

  return bv;
}

long BuildBitVectorNoErr(const char *const list[], size_t count, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf, sizeof(buf));

    if ((temp = compare_const_array(list, count, buf)) != -1)
      bv |= 1U << temp;
  }

  return bv;
}

int CheckSpecialsList(const char *const special_list[], size_t count,
                      const char *const special_list2[], size_t count2,
                      char *line) {
  int wSpecCheck = -1, wSpec2Check = -1;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = one_arg(line, buf, sizeof(buf));

    if (special_list)
      wSpecCheck = compare_const_array(special_list, count, buf);

    if (special_list2)
      wSpec2Check = compare_const_array(special_list2, count2, buf);

    if ((wSpecCheck == -1) && (wSpec2Check == -1))
      return 0;
  }

  return 1;
}

int WeaponIFromString(char *data) {
  for (int x = 0; x < num_def_weapons; x++) {
    if (!strcasecmp(weapon_catalogue_name(x), data))
      return x + 1; /* weapons start at 1 not 0 */
  }
  return -1;
}

int AmmoIFromString(char *data) {
  char *separator = strchr(data, '_');
  if (!separator)
    return -1;
  char *name = checked_mutable_string_suffix(separator, 1);
  for (int x = 0; x < num_def_weapons; x++) {
    if (!strcasecmp(weapon_catalogue_name(x), name))
      return x + 101;
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
  int cl = ((mech)->rd.specials) & CLAN_TECH;
  int e_count = 0;
  int tTechOK = 1;
  int wcHvyFF = 0;
  int wcLtFF = 0;
  int wcC3i = 0;
  int wcBloodhound = 0;
  int awInfSpec[5];
  int wcSuits = 0;

  ((mech)->rd.specials) &=
      ~(BEAGLE_PROBE_TECH | TRIPLE_MYOMER_TECH | MASC_TECH | ECM_TECH |
        C3_SLAVE_TECH | C3_MASTER_TECH | ARTEMIS_IV_TECH | ES_TECH | FF_TECH |
        IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH | LIGHT_BAP_TECH);
  if (((mech)->ud.type) == CLASS_MECH)
    ((mech)->rd.specials) &= ~(XL_TECH | XXL_TECH | CE_TECH | LE_TECH);

  ((mech)->rd.specials2) &=
      ~(STEALTH_ARMOR_TECH | NULLSIGSYS_TECH | ANGEL_ECM_TECH |
        HVY_FF_ARMOR_TECH | LT_FF_ARMOR_TECH | TAG_TECH | C3I_TECH |
        BLOODHOUND_PROBE_TECH | TCOMP_TECH);

  ((mech)->rd.infantry_specials) &=
      ~(CS_PURIFIER_STEALTH_TECH | DC_KAGE_STEALTH_TECH |
        FWL_ACHILEUS_STEALTH_TECH | FC_INFILTRATOR_STEALTH_TECH |
        FC_INFILTRATORII_STEALTH_TECH);

  for (x = 0; x < 5; x++)
    *template_integer_slot(awInfSpec, 5, x) = 0;

  for (x = 0; x < NUM_SECTIONS; x++) {
    e_count = 0;
    mech_section_configuration_remove(mech, x, CASE_TECH);
    *template_integer_slot(awcSthArmor, NUM_SECTIONS, x) = 0;
    *template_integer_slot(awcNSS, NUM_SECTIONS, x) = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++)
      if ((t = mech_critical_part_type(mech, x, y))) {
        switch (special_from_equipment_index(t)) {
        case ARTEMIS_IV:
          ((mech)->rd.specials) |= ARTEMIS_IV_TECH;
          break;
        case BEAGLE_PROBE:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            ((mech)->rd.specials) |= BEAGLE_PROBE_TECH;
          break;
        case LIGHT_BAP:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            ((mech)->rd.specials) |= LIGHT_BAP_TECH;
          break;
        case ECM:
          ((mech)->rd.specials) |= ECM_TECH;
          break;
        case TAG:
          ((mech)->rd.specials2) |= TAG_TECH;
          break;
        case C3_SLAVE:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            ((mech)->rd.specials) |= C3_SLAVE_TECH;
          break;
        case MASC:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            masc_count++;
          break;
        case C3_MASTER:
          c3_master_count++;
          break;
        case C3I:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            wcC3i++;
          break;
        case ANGELECM:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            wcAngel++;
          break;
        case TRIPLE_STRENGTH_MYOMER:
          tsm_count++;
          break;
        case FERRO_FIBROUS:
          ff_count++;
          break;
        case HVY_FERRO_FIBROUS:
          wcHvyFF++;
          break;
        case LT_FERRO_FIBROUS:
          wcLtFF++;
          break;
        case BLOODHOUND_PROBE:
          wcBloodhound++;
          break;
        case TARGETING_COMPUTER:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            tc_count++;
          break;
        case ENDO_STEEL:
          es_count++;
          break;
        case PURIFIER_ARMOR:
          (*template_integer_slot(awInfSpec, 5, 0))++;
          break;
        case KAGE_STEALTH_UNIT:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            (*template_integer_slot(awInfSpec, 5, 1))++;
          break;
        case ACHILEUS_STEALTH_UNIT:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            (*template_integer_slot(awInfSpec, 5, 2))++;
          break;
        case INFILTRATOR_STEALTH_UNIT:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            (*template_integer_slot(awInfSpec, 5, 3))++;
          break;
        case INFILTRATORII_STEALTH_UNIT:
          if (!mech_critical_is_nonfunctional(mech, x, y))
            (*template_integer_slot(awInfSpec, 5, 4))++;
          break;
        case ENGINE:
          e_count++;
          break;
        case CASE:
          mech_section_configuration_add(
              mech, ((mech)->ud.type) == CLASS_VEH_GROUND ? BSIDE : x,
              CASE_TECH);
          break;
        case STEALTH_ARMOR:
          (*template_integer_slot(awcSthArmor, NUM_SECTIONS, x))++;
          wcSthArmor++;
          break;
        case NULL_SIGNATURE_SYSTEM:
          (*template_integer_slot(awcNSS, NUM_SECTIONS, x))++;
          wcNSS++;
          break;
        }
        if (equipment_is_weapon(t) &&
            weapon_catalogue_is_anti_missile(weapon_from_equipment_index(t))) {
          if (weapon_catalogue_has_special(weapon_from_equipment_index(t),
                                           CLAT))
            ((mech)->rd.specials) |= CL_ANTI_MISSILE_TECH;
          else
            ((mech)->rd.specials) |= IS_ANTI_MISSILE_TECH;
        }
      }
    if (x != CTORSO && e_count) {
      if (e_count > 3)
        ((mech)->rd.specials) |= XXL_TECH;

      else if (e_count == 2)
        if (cl)
          ((mech)->rd.specials) |= XL_TECH;

        else
          ((mech)->rd.specials) |= LE_TECH;

      else
        ((mech)->rd.specials) |= XL_TECH;
    } else {
      if (x == CTORSO && e_count < 4 && ((mech)->ud.type) == CLASS_MECH)
        ((mech)->rd.specials) |= CE_TECH;
    }
  }
  if ((((mech)->rd.specials) & (XXL_TECH | XL_TECH | LE_TECH)) &&
      (((mech)->rd.specials) & CE_TECH))
    btech_channel_send(
        mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("#%ld apparently is very weird: Compact engine AND XL/XXL?",
                mech->mynum));
  if (tc_count) {
    ((mech)->rd.specials2) |= TCOMP_TECH;
    for (x = 0; x < NUM_SECTIONS; x++)
      for (y = 0; y < CritsInLoc(mech, x); y++)
        if (equipment_is_weapon((t = mech_critical_part_type(mech, x, y))))
          if (equipment_can_use_targeting_computer(t))
            mech_critical_fire_mode_add(mech, x, y, ON_TC);
  }
  if (masc_count >= MAX(1, (((mech)->ud.tons) / (cl ? 25 : 20))))
    ((mech)->rd.specials) |= MASC_TECH;
  if (ff_count >= (cl ? 7 : 14) ||
      (((mech)->ud.type) != CLASS_MECH && ff_count > 0))
    ((mech)->rd.specials) |= FF_TECH;
  if (es_count >= (cl ? 7 : 14) ||
      (((mech)->ud.type) != CLASS_MECH && es_count > 0))
    ((mech)->rd.specials) |= ES_TECH;
  if (tsm_count >= 6 || (((mech)->ud.type) != CLASS_MECH && tsm_count > 0))
    ((mech)->rd.specials) |= TRIPLE_MYOMER_TECH;
  if (wcAngel >= 2 || (((mech)->ud.type) != CLASS_MECH && wcAngel > 0))
    ((mech)->rd.specials2) |= ANGEL_ECM_TECH;
  if (wcHvyFF >= 21 || (((mech)->ud.type) != CLASS_MECH && wcHvyFF > 0))
    ((mech)->rd.specials2) |= HVY_FF_ARMOR_TECH;
  if (wcLtFF >= 7 || (((mech)->ud.type) != CLASS_MECH && wcLtFF > 0))
    ((mech)->rd.specials2) |= LT_FF_ARMOR_TECH;
  if (wcC3i >= 2 || (((mech)->ud.type) != CLASS_MECH && wcC3i > 0))
    ((mech)->rd.specials2) |= C3I_TECH;
  if (wcBloodhound >= 3 ||
      (((mech)->ud.type) != CLASS_MECH && wcBloodhound > 0))
    ((mech)->rd.specials2) |= BLOODHOUND_PROBE_TECH;

  if (((mech)->ud.type) == CLASS_MECH) {
    /* Be 'noisy' about some crits/techs */
    if ((ff_count > 0) && (ff_count < (cl ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing FF Crits %d/%d!",
                                 ((mech)->ud.mech_type), mech->mynum, ff_count,
                                 (cl ? 7 : 14)));

    if ((es_count > 0) && (es_count < (cl ? 7 : 14)))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing ES Crits %d/%d!",
                                 ((mech)->ud.mech_type), mech->mynum, es_count,
                                 (cl ? 7 : 14)));

    if ((tsm_count > 0) && (tsm_count < 6))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing TSM Crits %d/6!",
                                 ((mech)->ud.mech_type), mech->mynum,
                                 tsm_count));

    if ((wcHvyFF > 0) && (wcHvyFF < 21))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing HvyFF Crits %d/21!",
                                 ((mech)->ud.mech_type), mech->mynum, wcHvyFF));

    if ((wcLtFF > 0) && (wcLtFF < 7))
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("%s (#%ld) is missing LtFF Crits %d/7!",
                                 ((mech)->ud.mech_type), mech->mynum, wcLtFF));
  }

  /*
   * Check our NSS. Need 1 crit in each loc except H
   */
  tTechOK = 0;

  if (wcNSS > 0) {
    tTechOK = 1;

    if (((mech)->ud.type) != CLASS_MECH) {
      if (wcNSS < 1)
        tTechOK = 0;
    } else {
      for (x = 0; x < NUM_SECTIONS; x++) {
        if (x != HEAD) {
          if (*template_integer_slot(awcNSS, NUM_SECTIONS, x) < 1) {
            tTechOK = 0;
            break;
          }
        }
      }
    }

    if (tTechOK)
      ((mech)->rd.specials2) |= NULLSIGSYS_TECH;
  }

  /*
   * Check our Stealth armor. Need 2 crits in each loc except H and CT
   */
  tTechOK = 0;

  if (wcSthArmor > 0) {
    tTechOK = 1;

    if (!(((mech)->rd.specials) & ECM_TECH)) {
      tTechOK = 0;
    } else {
      if (((mech)->ud.type) != CLASS_MECH) {
        if (wcSthArmor < 1)
          tTechOK = 0;
      } else {
        for (x = 0; x < NUM_SECTIONS; x++) {
          if ((x != HEAD) && (x != CTORSO)) {
            if (*template_integer_slot(awcSthArmor, NUM_SECTIONS, x) < 2) {
              tTechOK = 0;
              break;
            }
          }
        }
      }
    }

    if (tTechOK)
      ((mech)->rd.specials2) |= STEALTH_ARMOR_TECH;
  }

  /* Let's do our suit checks */
  if (((mech)->ud.type) == CLASS_BSUIT) {
    wcSuits = bsuit_member_count(mech);

    if (*template_integer_slot(awInfSpec, 5, 0) >= wcSuits)
      ((mech)->rd.infantry_specials) |= CS_PURIFIER_STEALTH_TECH;

    if (*template_integer_slot(awInfSpec, 5, 1) >= wcSuits)
      ((mech)->rd.infantry_specials) |= DC_KAGE_STEALTH_TECH;

    if (*template_integer_slot(awInfSpec, 5, 2) >= wcSuits)
      ((mech)->rd.infantry_specials) |= FWL_ACHILEUS_STEALTH_TECH;

    if (*template_integer_slot(awInfSpec, 5, 3) >= wcSuits)
      ((mech)->rd.infantry_specials) |= FC_INFILTRATOR_STEALTH_TECH;

    if (*template_integer_slot(awInfSpec, 5, 4) >= wcSuits)
      ((mech)->rd.infantry_specials) |= FC_INFILTRATORII_STEALTH_TECH;
  }

  /* New C3 Master code */
  if (c3_master_count > 0) {
    mech_c3_total_masters_set(mech, mech_c3_total_master_count(mech));
    mech_c3_working_masters_set(mech, mech_c3_working_master_count(mech));

    if (mech_c3_total_masters(mech) > 0)
      ((mech)->rd.specials) |= C3_MASTER_TECH;

    if (mech_c3_working_masters(mech) == 0)
      ((mech)->rd.critstatus) |= C3_DESTROYED;
    else
      ((mech)->rd.critstatus) &= ~C3_DESTROYED;
  }
}

int update_oweight(Mech *mech, int value) {
  ((mech)->rd.critstatus) |= OWEIGHT_OK;

  /* Check to prevent silliness */
  if (!mech->xcode.context->configuration->btech_dynspeed ||
      (value == 1 && !mech_is_destroyed(mech)))
    value = ((mech)->ud.tons) * 1024;
  ((mech)->rd.row) = value;
  return value;
}

int mech_calculated_weight(Mech *mech) {
  if (((mech)->rd.critstatus) & OWEIGHT_OK)
    return ((mech)->rd.row);
  return update_oweight(mech, mech_weight_sub(GOD, mech, -1));
}
