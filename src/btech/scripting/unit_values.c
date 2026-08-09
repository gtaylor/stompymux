#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_tech_api.h"
#include "mech_utils_api.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "template_api.h"
#include "values_internal.h"

#include "checked_conversion.h"

#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

char *mechIDfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  const MechUnitId id = mech_unit_id(mech);
  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 0) = id.first;
  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 1) = id.second;
  *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), 2) = '\0';
  return buffer;
}

static bool parse_damage_numbers(const char *token, const char *prefix,
                                 bool has_third, int *first, int *second,
                                 int *third) {
  const size_t token_length = strcspn(token, " ,");
  const size_t prefix_length = strlen(prefix);
  if (token_length >= 64 || token_length <= prefix_length ||
      strncmp(token, prefix, prefix_length) != 0) {
    return false;
  }

  char text[64];
  memcpy(text, token, token_length);
  *(char *)checked_storage_at(text, sizeof(text), sizeof(*text), token_length) =
      '\0';
  char *second_text =
      strchr(checked_mutable_string_suffix(text, prefix_length), '/');
  if (second_text == nullptr)
    return false;
  *second_text = '\0';
  second_text = checked_mutable_string_suffix(second_text, 1);
  if (!has_third)
    return parse_int_checked(checked_string_suffix(text, prefix_length),
                             first) &&
           parse_int_checked(second_text, second);

  char *third_text = strchr(second_text, '(');
  char *last = (char *)checked_storage_at(text, sizeof(text), sizeof(*text),
                                          token_length - 1);
  if (third_text == nullptr || *last != ')')
    return false;
  *third_text = '\0';
  *last = '\0';
  return parse_int_checked(checked_string_suffix(text, prefix_length), first) &&
         parse_int_checked(second_text, second) &&
         parse_int_checked(checked_string_suffix(third_text, 1), third);
}

char *mech_getset_ref(int mode, Mech *mech, char *data) {
  if (mode) {
    mech_model_reference_set(mech, data);
    return nullptr;
  }

  static char reference[LBUF_SIZE];
  snprintf(reference, sizeof(reference), "%s", mech_model_reference(mech));
  return reference;
}

extern char *mech_types[];
extern char *move_types[];

char *mechTypefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode) {
    const UnitClass unit_class = mech_class(mech);
    return template_unit_class_name((size_t)unit_class);
  }
  /* Should _alter_ mechtype.. weeeel. */
  if ((i = compare_array(mech_types, template_unit_class_count(), arg)) >= 0)
    mech_class_set(mech, (UnitClass)i);
  return nullptr;
}

char *mechMovefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode) {
    const MechMovementType movement_type = mech_movement_type(mech);
    return template_movement_type_name((size_t)movement_type);
  }
  if ((i = compare_array(move_types, template_movement_type_count(), arg)) >= 0)
    mech_movement_type_set(mech, (MechMovementType)i);
  return NULL;
}

char *mechTechTimefunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int n = figure_latest_tech_event(mech);

  snprintf(buffer, LBUF_SIZE, "%d", n);
  return buffer;
}

void apply_mechDamage(Mech *omech, char *buf) {
  Mech *mech = mech_temporary_clone(omech);
  int i, j, i1, i2, i3;
  int do_mag = 0;

  if (mech == nullptr)
    return;
  for (i = 0; i < NUM_SECTIONS; i++) {
    mech_section_internal_set(mech, i, mech_section_original_internal(mech, i));
    mech_section_armor_set(mech, i, mech_section_original_armor(mech, i));
    mech_section_rear_armor_set(mech, i,
                                mech_section_original_rear_armor(mech, i));
    for (j = 0; j < NUM_CRITICALS; j++)
      if (mech_critical_part_type(mech, i, j) &&
          !mech_part_is_structural_placeholder(
              mech_critical_part_type(mech, i, j))) {
        if (mech_critical_is_destroyed(mech, i, j))
          mech_critical_destroyed_set(mech, i, j, false);
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, j)))
          mech_critical_data_set(mech, i, j,
                                 mech_critical_full_ammunition(mech, i, j));
        else
          mech_critical_temporary_failure_set(mech, i, j, 0);
      }
  }
  size_t offset = 0;
  const size_t input_length = strlen(buf);
  while (offset < input_length) {
    offset += strspn(checked_string_suffix(buf, offset), " ,");
    if (offset >= input_length)
      break;
    const char *token = checked_string_suffix(buf, offset);
    /* Parse the keyword ; it's one of the many known types */
    if (parse_damage_numbers(token, "A:", false, &i1, &i2, &i3)) {
      /* Ordinary armor damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_armor_set(mech, i1,
                               mech_section_original_armor(mech, i1) - i2);
    } else if (parse_damage_numbers(token, "A(R):", false, &i1, &i2, &i3)) {
      /* Ordinary rear armor damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_rear_armor_set(
            mech, i1, mech_section_original_rear_armor(mech, i1) - i2);
    } else if (parse_damage_numbers(token, "I:", false, &i1, &i2, &i3)) {
      /* Ordinary int damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_internal_set(
            mech, i1, mech_section_original_internal(mech, i1) - i2);
    } else if (parse_damage_numbers(token, "C:", false, &i1, &i2, &i3)) {
      /* Dest'ed crit */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_critical_destroyed_set(mech, i1, i2, true);
    } else if (parse_damage_numbers(token, "G:", true, &i1, &i2, &i3)) {
      /* Glitch */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          mech_critical_temporary_failure_set(mech, i1, i2, i3);
    } else if (parse_damage_numbers(token, "R:", true, &i1, &i2, &i3)) {
      /* Reload */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          mech_critical_data_set(
              mech, i1, i2, mech_critical_full_ammunition(mech, i1, i2) - i3);
    }
    offset += strcspn(token, " ,");
  }
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (mech_section_internal(mech, i) != mech_section_internal(omech, i))
      mech_section_internal_set(omech, i, mech_section_internal(mech, i));
    if (mech_section_armor(mech, i) != mech_section_armor(omech, i))
      mech_section_armor_set(omech, i, mech_section_armor(mech, i));
    if (mech_section_rear_armor(mech, i) != mech_section_rear_armor(omech, i))
      mech_section_rear_armor_set(omech, i, mech_section_rear_armor(mech, i));
    for (j = 0; j < NUM_CRITICALS; j++)
      if (mech_critical_part_type(mech, i, j) &&
          !mech_part_is_structural_placeholder(
              mech_critical_part_type(mech, i, j))) {
        if (mech_critical_is_destroyed(mech, i, j) &&
            !mech_critical_is_destroyed(omech, i, j)) {
          /* Blast a part */
          mech_critical_destroyed_set(omech, i, j, true);
          do_mag = 1;
        } else if (!mech_critical_is_destroyed(mech, i, j) &&
                   mech_critical_is_destroyed(omech, i, j)) {
          mech_RepairPart(omech, i, j);
          mech_critical_temporary_failure_set(omech, i, j, 0);
          do_mag = 1;
        }
        if (equipment_is_ammunition(mech_critical_part_type(mech, i, j))) {
          if (mech_critical_data(mech, i, j) != mech_critical_data(omech, i, j))
            mech_critical_data_set(omech, i, j, mech_critical_data(mech, i, j));
        } else {
          if (mech_critical_temporary_failure(mech, i, j) !=
              mech_critical_temporary_failure(omech, i, j))
            mech_critical_temporary_failure_set(
                omech, i, j, mech_critical_temporary_failure(mech, i, j));
        }
      }
  }
  if (do_mag && mech_class(omech) == CLASS_MECH)
    do_magic(omech);
  mech_temporary_destroy(mech);
}

static void damage_list_append(char buffer[static LBUF_SIZE], int *count,
                               const char *format, ...)
    __attribute__((format(printf, 3, 4)));

static void damage_list_append(char buffer[static LBUF_SIZE], int *count,
                               const char *format, ...) {
  if ((*count)++) {
    size_t length = strlen(buffer);
    if (length + 1 < LBUF_SIZE) {
      *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), length) =
          ',';
      *(char *)checked_storage_at(buffer, LBUF_SIZE, sizeof(char), length + 1) =
          '\0';
    }
  }
  size_t length = strlen(buffer);
  va_list arguments;
  va_start(arguments, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  vsnprintf(
      checked_storage_region(buffer, LBUF_SIZE, length, LBUF_SIZE - length),
      LBUF_SIZE - length, format, arguments);
  va_end(arguments);
}

char *mechDamagefunc(int mode, Mech *mech, char *arg,
                     char buffer[static LBUF_SIZE]) {
  /* Lists damage in form:
     A:LOC/num[,LOC/num[,LOC(R)/num]],I:LOC/num
     C:LOC/num,R:LOC/num(num),G:LOC/num(num) */
  int i, j;
  int count = 0;

  if (mode) {
    apply_mechDamage(mech, arg);
    snprintf(buffer, LBUF_SIZE, "?");
    return buffer;
  };
  buffer[0] = '\0';
  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_original_internal(mech, i)) {
      if (mech_section_armor(mech, i) != mech_section_original_armor(mech, i))
        damage_list_append(buffer, &count, "A:%d/%d", i,
                           mech_section_original_armor(mech, i) -
                               mech_section_armor(mech, i));
      if (mech_section_rear_armor(mech, i) !=
          mech_section_original_rear_armor(mech, i))
        damage_list_append(buffer, &count, "A(R):%d/%d", i,
                           mech_section_original_rear_armor(mech, i) -
                               mech_section_rear_armor(mech, i));
    }
  for (i = 0; i < NUM_SECTIONS; i++)
    if (mech_section_original_internal(mech, i))
      if (mech_section_internal(mech, i) !=
          mech_section_original_internal(mech, i))
        damage_list_append(buffer, &count, "I:%d/%d", i,
                           mech_section_original_internal(mech, i) -
                               mech_section_internal(mech, i));
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (mech_critical_part_type(mech, i, j) &&
          !mech_part_is_structural_placeholder(
              mech_critical_part_type(mech, i, j))) {
        if (mech_critical_is_destroyed(mech, i, j)) {
          damage_list_append(buffer, &count, "C:%d/%d", i, j);
        } else {
          if (equipment_is_ammunition(mech_critical_part_type(mech, i, j))) {
            if (mech_critical_data(mech, i, j) !=
                mech_critical_full_ammunition(mech, i, j))
              damage_list_append(buffer, &count, "R:%d/%d(%d)", i, j,
                                 mech_critical_full_ammunition(mech, i, j) -
                                     mech_critical_data(mech, i, j));
          } else if (mech_critical_temporary_failure(mech, i, j))
            damage_list_append(buffer, &count, "G:%d/%d(%d)", i, j,
                               mech_critical_temporary_failure(mech, i, j));
        }
      }
    }
  return buffer;
}

char *mechCentBearingfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int x = mech_position_x(mech);
  int y = mech_position_y(mech);
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  snprintf(buffer, LBUF_SIZE, "%d",
           FindBearing(mech_position_real_x(mech), mech_position_real_y(mech),
                       fx, fy));
  return buffer;
}

char *mechCentDistfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int x = mech_position_x(mech);
  int y = mech_position_y(mech);
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  snprintf(buffer, LBUF_SIZE, "%.2f",
           (double)FindHexRange(fx, fy, mech_position_real_x(mech),
                                mech_position_real_y(mech)));
  return buffer;
}

/* Mode:
   0 = char -> bit field
   1 = bit field -> char
   */

static int bv_val(int in, int mode) {
  int p = 0;

  if (mode == 0) {
    if (in >= 'a' && in <= 'z')
      return 1 << (in - 'a');
    return 1 << ('z' - 'a' + 1 + (in - 'A'));
  }
  while (in > 0) {
    p++;
    in >>= 1;
  }
  /* Hmm. */
  p--;
  if (p > ('z' - 'a'))
    return 'A' + (p - ('z' - 'a' + 1));
  return 'a' + p;
}

int text2bv(const char *text) {
  int j = 0;
  int mode_not = 0;

  if (parse_int_checked(text, &j))
    return j; /* Allow 'old style' as well */

  /* Valid bitvector letters are: a-z (=27), A-Z (=27 more) */
  const size_t text_length = strlen(text);
  for (size_t index = 0; index < text_length; ++index) {
    char current = *checked_string_suffix(text, index);
    if (current == '!') {
      mode_not = 1;
      ++index;
      if (index >= text_length)
        break;
      current = *checked_string_suffix(text, index);
    };
    if ((current >= 'a' && current <= 'z') ||
        (current >= 'A' && current <= 'Z')) {
      int k = bv_val(current, 0);

      if (k) {
        if (mode_not)
          j &= ~k;
        else
          j |= k;
      }
    }
    mode_not = 0;
  }
  return j;
}

char *bv2text(int i, char *buffer) {
  int p = 1;
  size_t output = 0;

  while (i > 0) {
    if (i & 1) {
      *(char *)checked_storage_at(buffer, SBUF_SIZE, sizeof(char), output) =
          clamp_int_to_char(bv_val(p, 1));
      ++output;
    }
    i >>= 1;
    p <<= 1;
  }
  if (output == 0) {
    *(char *)checked_storage_at(buffer, SBUF_SIZE, sizeof(char), output) = '-';
    ++output;
  }
  *(char *)checked_storage_at(buffer, SBUF_SIZE, sizeof(char), output) = '\0';
  return buffer;
}

#undef offsetof
