#include "values_internal.h"

char *mechIDfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  buffer[0] = MechID(mech)[0];
  buffer[1] = MechID(mech)[1];
  buffer[2] = '\0';
  return buffer;
}

char *mech_getset_ref(int mode, Mech *mech, char *data) {
  if (mode) {
    strncpy(MechType_Ref(mech), data, 24);
    MechType_Ref(mech)[24] = '\0';
    return NULL;
  } else
    return MechType_Ref(mech);
}

extern char *mech_types[];
extern char *move_types[];

char *mechTypefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode)
    return mech_types[(short)MechType(mech)];
  /* Should _alter_ mechtype.. weeeel. */
  if ((i = compare_array(mech_types, arg)) >= 0)
    MechType(mech) = i;
  return NULL;
}

char *mechMovefunc(int mode, Mech *mech, char *arg) {
  int i;

  if (!mode)
    return move_types[(short)MechMove(mech)];
  if ((i = compare_array(move_types, arg)) >= 0)
    MechMove(mech) = i;
  return NULL;
}

char *mechTechTimefunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int n = figure_latest_tech_event(mech);

  snprintf(buffer, LBUF_SIZE, "%d", n);
  return buffer;
}

void apply_mechDamage(Mech *omech, char *buf) {
  Mech mek;
  Mech *mech = &mek;
  int i, j, i1, i2, i3;
  char *s;
  int do_mag = 0;

  memcpy(mech, omech, sizeof(Mech));
  for (i = 0; i < NUM_SECTIONS; i++) {
    SetSectInt(mech, i, GetSectOInt(mech, i));
    SetSectArmor(mech, i, GetSectOArmor(mech, i));
    SetSectRArmor(mech, i, GetSectORArmor(mech, i));
    for (j = 0; j < NUM_CRITICALS; j++)
      if (GetPartType(mech, i, j) && !IsCrap(GetPartType(mech, i, j))) {
        if (PartIsDestroyed(mech, i, j))
          UnDestroyPart(mech, i, j);
        if (IsAmmo(GetPartType(mech, i, j)))
          SetPartData(mech, i, j, FullAmmo(mech, i, j));
        else
          SetPartTempNuke(mech, i, j, 0);
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
        SetSectArmor(mech, i1, GetSectOArmor(mech, i1) - i2);
    } else if (sscanf(s, "A(R):%d/%d", &i1, &i2) == 2) {
      /* Ordinary rear armor damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        SetSectRArmor(mech, i1, GetSectORArmor(mech, i1) - i2);
    } else if (sscanf(s, "I:%d/%d", &i1, &i2) == 2) {
      /* Ordinary int damage */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        SetSectInt(mech, i1, GetSectOInt(mech, i1) - i2);
    } else if (sscanf(s, "C:%d/%d", &i1, &i2) == 2) {
      /* Dest'ed crit */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        DestroyPart(mech, i1, i2);
    } else if (sscanf(s, "G:%d/%d(%d)", &i1, &i2, &i3) == 3) {
      /* Glitch */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          SetPartTempNuke(mech, i1, i2, i3);
    } else if (sscanf(s, "R:%d/%d(%d)", &i1, &i2, &i3) == 3) {
      /* Reload */
      if (i1 >= 0 && i1 < NUM_SECTIONS)
        if (i2 >= 0 && i2 < NUM_CRITICALS)
          SetPartData(mech, i1, i2, FullAmmo(mech, i1, i2) - i3);
    }
    while (*s && (*s != ' ' && *s != ','))
      s++;
  }
  for (i = 0; i < NUM_SECTIONS; i++) {
    if (GetSectInt(mech, i) != GetSectInt(omech, i))
      SetSectInt(omech, i, GetSectInt(mech, i));
    if (GetSectArmor(mech, i) != GetSectArmor(omech, i))
      SetSectArmor(omech, i, GetSectArmor(mech, i));
    if (GetSectRArmor(mech, i) != GetSectRArmor(omech, i))
      SetSectRArmor(omech, i, GetSectRArmor(mech, i));
    for (j = 0; j < NUM_CRITICALS; j++)
      if (GetPartType(mech, i, j) && !IsCrap(GetPartType(mech, i, j))) {
        if (PartIsDestroyed(mech, i, j) && !PartIsDestroyed(omech, i, j)) {
          /* Blast a part */
          DestroyPart(omech, i, j);
          do_mag = 1;
        } else if (!PartIsDestroyed(mech, i, j) &&
                   PartIsDestroyed(omech, i, j)) {
          mech_RepairPart(omech, i, j);
          SetPartTempNuke(omech, i, j, 0);
          do_mag = 1;
        }
        if (IsAmmo(GetPartType(mech, i, j))) {
          if (GetPartData(mech, i, j) != GetPartData(omech, i, j))
            SetPartData(omech, i, j, GetPartData(mech, i, j));
        } else {
          if (PartTempNuke(mech, i, j) != PartTempNuke(omech, i, j))
            SetPartTempNuke(omech, i, j, PartTempNuke(mech, i, j));
        }
      }
  }
  if (do_mag && MechType(omech) == CLASS_MECH)
    do_magic(omech);
}

#define ADD(...)                                                               \
  {                                                                            \
    if (count++) {                                                             \
      size_t len = strlen(buffer);                                             \
      if (len + 1 < LBUF_SIZE) {                                               \
        buffer[len] = ',';                                                     \
        buffer[len + 1] = '\0';                                                \
      }                                                                        \
    }                                                                          \
    snprintf(buffer + strlen(buffer), LBUF_SIZE - strlen(buffer),              \
             __VA_ARGS__);                                                     \
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
    if (GetSectOInt(mech, i)) {
      if (GetSectArmor(mech, i) != GetSectOArmor(mech, i))
        ADD("A:%d/%d", i, GetSectOArmor(mech, i) - GetSectArmor(mech, i));
      if (GetSectRArmor(mech, i) != GetSectORArmor(mech, i))
        ADD("A(R):%d/%d", i, GetSectORArmor(mech, i) - GetSectRArmor(mech, i));
    }
  for (i = 0; i < NUM_SECTIONS; i++)
    if (GetSectOInt(mech, i))
      if (GetSectInt(mech, i) != GetSectOInt(mech, i))
        ADD("I:%d/%d", i, GetSectOInt(mech, i) - GetSectInt(mech, i));
  for (i = 0; i < NUM_SECTIONS; i++)
    for (j = 0; j < CritsInLoc(mech, i); j++) {
      if (GetPartType(mech, i, j) && !IsCrap(GetPartType(mech, i, j))) {
        if (PartIsDestroyed(mech, i, j)) {
          ADD("C:%d/%d", i, j);
        } else {
          if (IsAmmo(GetPartType(mech, i, j))) {
            if (GetPartData(mech, i, j) != FullAmmo(mech, i, j))
              ADD("R:%d/%d(%d)", i, j,
                  FullAmmo(mech, i, j) - GetPartData(mech, i, j));
          } else if (PartTempNuke(mech, i, j))
            ADD("G:%d/%d(%d)", i, j, PartTempNuke(mech, i, j));
        }
      }
    }
  return buffer;
}

char *mechCentBearingfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int x = MechX(mech);
  int y = MechY(mech);
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  snprintf(buffer, LBUF_SIZE, "%d",
           FindBearing(MechFX(mech), MechFY(mech), fx, fy));
  return buffer;
}

char *mechCentDistfunc(Mech *mech, char buffer[static LBUF_SIZE]) {
  int x = MechX(mech);
  int y = MechY(mech);
  float fx, fy;

  MapCoordToRealCoord(x, y, &fx, &fy);
  snprintf(buffer, LBUF_SIZE, "%.2f",
           FindHexRange(fx, fy, MechFX(mech), MechFY(mech)));
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

  if (!Readnum(j, text))
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
