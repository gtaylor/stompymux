#include "mech_template_api.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_status_types.h"
#include "template_api.h"

void mech_template_clear(Mech *mech, bool clear_communications) {
  mech_template_state_reset(mech);

  MechSpotter(mech) = -1;
  MechTarget(mech) = -1;
  MechChargeTarget(mech) = -1;
  MechChargeTimer(mech) = 0;
  MechChargeDistance(mech) = 0;
  MechSwarmTarget(mech) = -1;
  MechSwarmer(mech) = -1;
  MechDFATarget(mech) = -1;
  MechTargX(mech) = -1;
  MechStatus(mech) = 0;
  MechTargY(mech) = -1;
  MechPilot(mech) = -1;
  MechAim(mech) = NUM_SECTIONS;
  mech_event_cancel(mech, EVENT_VEHICLEBURN);
  if (clear_communications)
    mech_communications_clear(mech);
}

static int template_load_modern(DbRef player, Mech *mech, const char *id) {
  FILE *fp = nullptr;
  char *filename;

  filename = mech_template_resolve_path(
      mech_context(mech), btech_context_mech_template_path(mech_context(mech)),
      id);

  if (!filename)
    return 0;
  if (!(fp = fopen(filename, "r")))
    return 0;
  fclose(fp);
  return load_template(player, mech, filename) >= 0 ? 1 : 0;
}

extern const int num_def_weapons;

static int template_part_type_is_invalid(int i) {
  if (!i)
    return 0;
  if (IsWeapon(i)) {
    if (i > (num_def_weapons))
      return 1;
  }
  if (IsAmmo(i)) {
    if ((Ammo2Weapon(i) + 1) > (num_def_weapons))
      return 1;
  }
  if (IsSpecial(i))
    if (Special2I(i) >= count_special_items())
      return 1;
  return 0;
}

static bool template_load_error(FILE *fp, Mech *mech, bool condition,
                                const char *format, ...)
    __attribute__((format(printf, 4, 5)));

static bool template_load_error(FILE *fp, Mech *mech, bool condition,
                                const char *format, ...) {
  if (!condition) {
    return false;
  }
#ifdef TEMPLATE_VERBOSE_ERRORS
  char message[LBUF_SIZE] = {0};
  va_list args;
  va_start(args, format);
  vsnprintf(message, sizeof(message), format, args);
  va_end(args);
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ERRORS, "%s",
                     message);
#else
  (void)mech;
  (void)format;
#endif
  if (fp) {
    fclose(fp);
  }
  return true;
}

static int template_load_legacy(Mech *mech, const char *id) {
  FILE *fp = nullptr;
  int i, j, k, t;
  int i1, i2, i3, i4, i5, i6;
  char *filename;

  filename = mech_template_resolve_path(
      mech_context(mech), btech_context_mech_template_path(mech_context(mech)),
      id);
  if (template_load_error(fp, mech, filename == nullptr,
                          "No matching file for '%s'.", id)) {
    return -1;
  }
  if (filename == nullptr) {
    return -1;
  }
  fp = fopen(filename, "r");
  if (template_load_error(fp, mech, fp == nullptr,
                          "Unable to open file %s (%s)!", filename, id)) {
    return -1;
  }
  if (fp == nullptr) {
    return -1;
  }
  strncpy(MechType_Ref(mech), id, 25);
  MechType_Ref(mech)[24] = '\0';
  if (template_load_error(
          fp, mech,
          fscanf(fp, "%d %d %d %d %d %f %f %d\n", &i1, &i2, &i3, &i4, &i5,
                 &MechMaxSpeed(mech), &MechJumpSpeed(mech), &i6) < 8,
          "Old template loading system: %s is invalid template file.", id)) {
    return -1;
  }
  MechTons(mech) = i1;
  MechTacRange(mech) = i2;
  MechLRSRange(mech) = i3;
  MechScanRange(mech) = i4;
  MechRealNumsinks(mech) = i5;
#define DROP(a)                                                                \
  if (i6 & a)                                                                  \
  i6 &= ~a
  DROP(32768); /* Quad */
  DROP(16384); /* Salvagetech */
  DROP(8192);  /* Cargotech */
  DROP(4196);  /* Watergun */
  MechSpecials(mech) = i6;
  for (k = 0; k < NUM_SECTIONS; k++) {
    i = k;
    if (MechType(mech) == 4) {
      switch (k) {
      case 3:
        i = 4;
        break;
      case 4:
        i = 5;
        break;
      case 5:
        i = 3;
        break;
      }
    }
    if (template_load_error(fp, mech,
                            fscanf(fp, "%d %d %d %d\n", &i1, &i2, &i3, &i4) < 4,
                            "Insufficient data reading section %d!", i)) {
      return -1;
    }
    MechSections(mech)[i].recycle = 0;
    SetSectArmor(mech, i, i1);
    SetSectOArmor(mech, i, i1);
    SetSectInt(mech, i, i2);
    SetSectOInt(mech, i, i2);
    SetSectRArmor(mech, i, i3);
    SetSectORArmor(mech, i, i3);
    /* Remove all rampant AXEs from the arms themselves, we do
       things differently here */
    if (i4 & 4)
      i4 &= ~4;
    MechSections(mech)[i].config = i4;
    for (j = 0; j < NUM_CRITICALS; j++) {
      if (template_load_error(
              fp, mech, fscanf(fp, "%d %d %d\n", &i1, &i2, &i3) < 3,
              "Insufficient data reading critical %d/%d!", i, j)) {
        return -1;
      }
      MechSections(mech)[i].criticals[j].type = i1;
      if (template_load_error(
              fp, mech, template_part_type_is_invalid(GetPartType(mech, i, j)),
              "Invalid datatype at %d/%d!", i, j)) {
        return -1;
      }
      if (IsSpecial(i1))
        i1 += SPECIAL_BASE_INDEX - OSPECIAL_BASE_INDEX;
      if (IsWeapon(GetPartType(mech, i, j)) &&
          IsAMS((t = Weapon2I(GetPartType(mech, i, j))))) {
        if (MechWeapons[t].special & CLAT)
          MechSpecials(mech) |= CL_ANTI_MISSILE_TECH;
        else
          MechSpecials(mech) |= IS_ANTI_MISSILE_TECH;
      }
      MechSections(mech)[i].criticals[j].data = i2;
      MechSections(mech)[i].criticals[j].firemode = i3;
    }
  }
  if (fscanf(fp, "%d %d\n", &i1, &i2) == 2) {
    MechType(mech) = i1;
    if (template_load_error(fp, mech, MechType(mech) > CLASS_LAST,
                            "Invalid 'mech type!")) {
      return -1;
    }
    MechMove(mech) = i2;
    if (template_load_error(fp, mech, MechMove(mech) > MOVENEMENT_LAST,
                            "Invalid movenement type!")) {
      return -1;
    }
  }
  if (fscanf(fp, "%d\n", &i1) != 1)
    MechRadioRange(mech) = DEFAULT_RADIORANGE;
  else
    MechRadioRange(mech) = i1;
  fclose(fp);
  return 1;
}

#undef LOADNEW_LOADS_OLD_IF_FAIL
#define LOADNEW_LOADS_MUSE_FORMAT

int mech_template_load(DbRef player, Mech *mech, const char *id) {
  char mech_origid[100];

  strncpy(mech_origid, MechType_Ref(mech), 99);
  mech_origid[99] = '\0';

  if (!strcmp(mech_origid, id)) {
    mech_template_clear(mech, 0);
    if (template_load_modern(player, mech, id) <= 0)
      return template_load_legacy(mech, id) > 0;
    return 1;
  } else {
    mech_template_clear(mech, 1);
    if (template_load_modern(player, mech, id) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
      if (template_load_legacy(mech, id) < 1)
#endif
#ifdef LOADNEW_LOADS_OLD_IF_FAIL
        if (template_load_modern(player, mech, mech_origid) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
          if (template_load_legacy(mech, mech_origid) < 1)
#endif
#endif
            return 0;
  }
  return 1;
}
