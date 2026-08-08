#include "values_internal.h"

#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"

char *mechIDfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  const MechUnitId id = mech_unit_id(mech);
  buffer[0] = id.first;
  buffer[1] = id.second;
  buffer[2] = '\0';
  return buffer;
}

char *mech_getset_ref(int mode, Mech *mech, char *data) {
  if (mode) {
    mech_model_reference_set(mech, data);
    return NULL;
  } else
    return (char *)mech_model_reference(mech);
}

extern char *mech_types[];
extern char *move_types[];

char *mechTypefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode)
    return mech_types[(short)mech_class(mech)];
  /* Should _alter_ mechtype.. weeeel. */
  if ((i = compare_array(mech_types, arg)) >= 0)
    mech_class_set(mech, (UnitClass)i);
  return NULL;
}

char *mechMovefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode)
    return move_types[(short)mech_movement_type(mech)];
  if ((i = compare_array(move_types, arg)) >= 0)
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
  char *s;
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
  s = buf;
  while (*s) {
    while (*s && (*s == ' ' || *s == ','))
      s++;
    if (!(*s))
      break;
    /* Parse the keyword ; it's one of the many known types */
    if (sscanf(s, "A:%d/%d", &i1, &i2) == 2) {
      /* Ordinary armor damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_armor_set(mech, i1,
                               mech_section_original_armor(mech, i1) - i2);
    } else if (sscanf(s, "A(R):%d/%d", &i1, &i2) == 2) {
      /* Ordinary rear armor damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_rear_armor_set(
            mech, i1, mech_section_original_rear_armor(mech, i1) - i2);
    } else if (sscanf(s, "I:%d/%d", &i1, &i2) == 2) {
      /* Ordinary int damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_section_internal_set(
            mech, i1, mech_section_original_internal(mech, i1) - i2);
    } else if (sscanf(s, "C:%d/%d", &i1, &i2) == 2) {
      /* Dest'ed crit */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        mech_critical_destroyed_set(mech, i1, i2, true);
    } else if (sscanf(s, "G:%d/%d(%d)", &i1, &i2, &i3) == 3) {
      /* Glitch */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          mech_critical_temporary_failure_set(mech, i1, i2, i3);
    } else if (sscanf(s, "R:%d/%d(%d)", &i1, &i2, &i3) == 3) {
      /* Reload */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          mech_critical_data_set(
              mech, i1, i2, mech_critical_full_ammunition(mech, i1, i2) - i3);
    }
    while (*s && (*s != ' ' && *s != ','))
      s++;
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
      buffer[length] = ',';
      buffer[length + 1] = '\0';
    }
  }
  size_t length = strlen(buffer);
  va_list arguments;
  va_start(arguments, format);
  vsnprintf(buffer + length, LBUF_SIZE - length, format, arguments);
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
           FindHexRange(fx, fy, mech_position_real_x(mech),
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

int text2bv(char *text) {
  char *c;
  int j = 0;
  int mode_not = 0;

  if (!(!((j) = atoi(text)) && strcmp((text), "0")))
    return j; /* Allow 'old style' as well */

  /* Valid bitvector letters are: a-z (=27), A-Z (=27 more) */
  for (c = text; *c; c++) {
    if (*c == '!') {
      mode_not = 1;
      c++;
    };
    if ((*c >= 'a' && *c <= 'z') || (*c >= 'A' && *c <= 'Z')) {
      int k = bv_val(*c, 0);

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
  char *c = buffer;

  while (i > 0) {
    if (i & 1)
      *(c++) = bv_val(p, 1);
    i >>= 1;
    p <<= 1;
  }
  if (c == buffer)
    *(c++) = '-';
  *c = 0;
  return buffer;
}

#undef offsetof
