#include "btconfig.h"
#include "btech_channel.h"
#include "btech_text_builder.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mech_c3_api.h"
#include "mech_consistency_api.h"
#include "mech_electronics_api.h"
#include "mech_internal.h"
#include "mech_mechref_ident_api.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "section_types.h"
#include "template_api.h"
#include "template_implementation.h"
#include "weapon_catalogue_api.h"
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include <strings.h>
#include <time.h>

void try_to_find_name(const char *mechref, Mech *mech) {
  const char *c;

  c = find_mechname_by_mechref(mechref);
  if (c)
    (void)string_copy_bounded(((mech)->ud.mech_name),
                              sizeof(((mech)->ud.mech_name)), c);
}

int default_fuel_by_type(Mech *mech) {
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

typedef struct NondefaultRangeSaveRequest {
  FILE *file;
  const Mech *mech;
  int current;
  int computer_default;
  int legacy_default;
  const char *name;
} NondefaultRangeSaveRequest;

static void save_nondefault_range(const NondefaultRangeSaveRequest *request) {
  FILE *fp = request->file;
  const Mech *mech = request->mech;
  const int CURRENT = request->current;
  const int COMPUTER_DEFAULT = request->computer_default;
  const int LEGACY_DEFAULT = request->legacy_default;
  const char *name = request->name;
  int expected =
      mech_computer_quality(mech) ? COMPUTER_DEFAULT : LEGACY_DEFAULT;

  if (CURRENT != expected)
    (void)fprintf(fp, "%-16s { %d }\n", name, CURRENT);
}

static void save_nondefault_integer(FILE *fp, int expected, int current,
                                    const char *name) {
  if (expected != current)
    (void)fprintf(fp, "%-16s { %d }\n", name, current);
}

int template_save(const TemplateSaveRequest *request) {
  const DbRef PLAYER = request->player;
  Mech *mech = request->mech;
  const char *reference = request->reference;
  const char *filename = request->filename;
  FILE *fp;
  int x;
  int x2;
  int inf_x;
  const char *const *locs;
  char *d;
  char timestamp[26];
  const char *c = ctime_r(&mech->xcode.context->clock->now, timestamp);

  if (!mech_computer_quality(mech))
    computer_conversion(mech);
  if (!((mech)->ud.mech_name)[0])
    try_to_find_name(reference, mech);
  fp = fopen(filename, "w");
  if (!fp)
    return -1;
  if (((mech)->ud.mech_name)[0])
    (void)fprintf(fp, "Name             { %s }\n", ((mech)->ud.mech_name));
  (void)fprintf(fp, "Reference        { %s }\n", reference);
  (void)fprintf(fp, "Type             { %s }\n",
                template_unit_class_name((size_t)mech->ud.type));
  (void)fprintf(fp, "Unit_Era         { %s }\n", ((mech)->ud.unit_era));
  (void)fprintf(fp, "Unit_TRO         { %s }\n", ((mech)->ud.unit_tro));
  (void)fprintf(fp, "Move_Type        { %s }\n",
                template_movement_type_name((size_t)mech->ud.move));
  (void)fprintf(fp, "Tons             { %d }\n", ((mech)->ud.tons));
  if (c == nullptr) {
    c = "unknown time";
  } else {
    d = strrchr(timestamp, '\n');
    if (d)
      *d = 0;
  }
  (void)fprintf(fp, "Comment          { Saved by: %s(#%ld) at %s }\n",
                game_object_name(mech->xcode.context->database, PLAYER), PLAYER,
                c);
  save_nondefault_range(&(NondefaultRangeSaveRequest){
      .file = fp,
      .mech = mech,
      .current = mech_tactical_range(mech),
      .computer_default = mech_default_tactical_range(mech),
      .legacy_default = DEFAULT_TACRANGE,
      .name = "Tac_Range",
  });
  save_nondefault_range(&(NondefaultRangeSaveRequest){
      .file = fp,
      .mech = mech,
      .current = mech_long_range_sensor_range(mech),
      .computer_default = mech_default_long_range_sensor_range(mech),
      .legacy_default = DEFAULT_LRSRANGE,
      .name = "LRS_Range",
  });
  save_nondefault_range(&(NondefaultRangeSaveRequest){
      .file = fp,
      .mech = mech,
      .current = mech_scanner_range(mech),
      .computer_default = mech_default_scanner_range(mech),
      .legacy_default = DEFAULT_SCANRANGE,
      .name = "Scan_Range",
  });
  save_nondefault_range(&(NondefaultRangeSaveRequest){
      .file = fp,
      .mech = mech,
      .current = mech_radio_range(mech),
      .computer_default = mech_default_radio_range(mech),
      .legacy_default = DEFAULT_RADIORANGE,
      .name = "Radio_Range",
  });

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
  save_nondefault_integer(fp, default_fuel_by_type(mech),
                          ((mech)->ud.fuel_orig), "Fuel");

  (void)fprintf(fp, "Max_Speed        { %.2f }\n",
                (double)((mech)->ud.maxspeed));
  if (((mech)->rd.jumpspeed) > 0.0F)
    (void)fprintf(fp, "Jump_Speed       { %.2f }\n",
                  (double)((mech)->rd.jumpspeed));
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

  if (x || x2) {
    (void)fprintf(fp, "Specials         { %s }\n",
                  template_bit_string_build(&(TemplateBitStringRequest){
                      .sets =
                          (TemplateBitSet[]){
                              {.descriptions = primary_technology_names(),
                               .count = primary_technology_name_count(),
                               .bits = x},
                              {.descriptions = secondary_technology_names(),
                               .count = secondary_technology_name_count(),
                               .bits = x2}},
                      .set_count = 2,
                      .delimiter = ' ',
                      .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
  }

  inf_x = ((mech)->rd.infantry_specials);

  if (inf_x) {
    (void)fprintf(
        fp, "InfantrySpecials { %s }\n",
        template_bit_string_build(&(TemplateBitStringRequest){
            .sets =
                &(TemplateBitSet){.descriptions = infantry_technology_names(),
                                  .count = infantry_technology_name_count(),
                                  .bits = inf_x},
            .set_count = 1,
            .delimiter = ' ',
            .buffer = (char[BTECH_TEXT_CAPACITY]){0}}));
  }

  int result = -1;
  locs = proper_section_string_from_type(((mech)->ud.type), ((mech)->ud.move));
  if (locs) {
    dump_locations(
        fp, mech, locs,
        unit_section_name_count(&(UnitSectionCatalog){
            .unit_type = mech->ud.type, .movement_type = mech->ud.move}));
    result = 0;
  }
  if (fclose(fp) != 0)
    return -1;
  return result;
}

static void skip_template_whitespace(FILE *fp) {
  int c;

  for (;;) {
    c = fgetc(fp);
    if (c == EOF || (c != ' ' && c != '\t' && c != '\n' && c != '\v' &&
                     c != '\f' && c != '\r'))
      break;
  }
  if (c != EOF)
    DASSERT(ungetc(c, fp) != EOF);
}

char *template_description_read(const TemplateDescriptionRead *request) {
  FILE *fp = request->file;
  char *data = request->line;
  char *buffer = request->buffer;
  BtechTextBuilder builder;
  btech_text_builder_initialize(&builder, buffer, BTECH_TEXT_CAPACITY);
  char *opening;

  opening = data ? strchr(data, '{') : nullptr;
  if (opening) {
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

  (void)string_copy_bounded(section, sizeof(section), cmd);
  size_t section_length = strlen(section);
  for (size_t index = 0; index < section_length; index++) {
    char *character =
        checked_storage_at(section, sizeof(section), sizeof(*section), index);
    if (*character == '_')
      *character = ' ';
  }
  locs = proper_section_string_from_type(type, mtype);
  return compare_const_array(locs,
                             unit_section_name_count(&(UnitSectionCatalog){
                                 .unit_type = type, .movement_type = mtype}),
                             section);
}

long build_bit_vector(const char *const list[], size_t count, char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = template_token_parse(&(TemplateTokenRequest){
        .input = line, .output = buf, .output_capacity = sizeof(buf)});
    temp = compare_const_array(list, count, buf);
    if (temp == -1)
      return -1;
    bv |= 1U << temp;
  }
  return bv;
}

long build_bit_vector_with_delim(const char *const list[], size_t count,
                                 char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = template_token_parse(
        &(TemplateTokenRequest){.input = line,
                                .output = buf,
                                .output_capacity = sizeof(buf),
                                .pipe_delimited = true});

    temp = compare_const_array(list, count, buf);
    if (temp == -1)
      return -1;

    bv |= 1U << temp;
  }

  return bv;
}

long build_bit_vector_no_err(const char *const list[], size_t count,
                             char *line) {
  long bv = 0;
  int temp;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return 0;

  while (*line) {
    line = template_token_parse(&(TemplateTokenRequest){
        .input = line, .output = buf, .output_capacity = sizeof(buf)});

    temp = compare_const_array(list, count, buf);
    if (temp != -1)
      bv |= 1U << temp;
  }

  return bv;
}

bool check_specials_list(const char *const special_list[], size_t count,
                         const char *const special_list2[], size_t count2,
                         char *line) {
  int w_spec_check = -1;
  int w_spec2_check = -1;
  char buf[30];

  if (!strcasecmp(line, "-"))
    return false;

  while (*line) {
    line = template_token_parse(&(TemplateTokenRequest){
        .input = line, .output = buf, .output_capacity = sizeof(buf)});

    if (special_list)
      w_spec_check = compare_const_array(special_list, count, buf);

    if (special_list2)
      w_spec2_check = compare_const_array(special_list2, count2, buf);

    if ((w_spec_check == -1) && (w_spec2_check == -1))
      return false;
  }

  return true;
}

int weapon_i_from_string(char *data) {
  for (int x = 0; x < DEFAULT_WEAPON_COUNT; x++) {
    if (!strcasecmp(weapon_catalogue_name(x), data))
      return x + 1; /* weapons start at 1 not 0 */
  }
  return -1;
}

int ammo_i_from_string(char *data) {
  char *separator = strchr(data, '_');
  if (!separator)
    return -1;
  char *name = checked_mutable_string_suffix(separator, 1);
  for (int x = 0; x < DEFAULT_WEAPON_COUNT; x++) {
    if (!strcasecmp(weapon_catalogue_name(x), name))
      return x + 101;
  }
  return -1;
}

int update_oweight(Mech *mech, int value) {
  mech_crit_status_set(&mech->rd.critstatus, MECH_CRIT_STATUS_OWEIGHT_OK);

  /* Check to prevent silliness */
  if (!mech->xcode.context->configuration->btech_dynspeed ||
      (value == 1 && !mech_is_destroyed(mech)))
    value = ((mech)->ud.tons) * 1024;
  ((mech)->rd.row) = value;
  return value;
}

int mech_calculated_weight(Mech *mech) {
  if (mech_crit_status_has(mech->rd.critstatus, MECH_CRIT_STATUS_OWEIGHT_OK))
    return ((mech)->rd.row);
  return update_oweight(mech, mech_weight_sub(GOD, mech, -1));
}
