/*
 * Last modified: Thu Aug 13 23:41:12 1998 fingon
 * Copyright (c) 1999-2005 Kevin Stevens
 *   All right reserved
 */

#include <ctype.h>
#include <dirent.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map_terrain.h" // IWYU pragma: keep
#include "mech_events.h"
#include "mech_lifecycle.h" // IWYU pragma: keep
#include "mux/network/mux_event.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define MECH_STAT_C /* want to use the POSIX stat() call. */

#include "mech.h"
#include "mech_build_api.h"
#include "mech_consistency_api.h"
#include "mech_restrict_api.h"
#include "mech_status_api.h"
#include "mech_utils_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/mux_event_alloc.h"
#include "template_api.h"

/* Selectors */
#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

extern char *strtok(char *s, const char *ct);

#define MECHREP_COMMON(a)                                                      \
  struct RepairFacility *rep = (struct RepairFacility *)data;                  \
  Mech *mech;                                                                  \
  DOCHECK_CONTEXT(rep->xcode.context,                                          \
                  !is_god(rep->xcode.context->database, player) &&             \
                      !is_wizard(rep->xcode.context->database, player),        \
                  "I'm sorry Dave, can't do that.");                           \
  if (a) {                                                                     \
    DOCHECK_CONTEXT(rep->xcode.context, rep->current_target == -1,             \
                    "You must set a target first!");                           \
    mech = btech_context_get_mech(rep->xcode.context, rep->current_target);    \
    DOCHECK_CONTEXT(rep->xcode.context, mech == nullptr,                       \
                    "The target's BTech data is not allocated.");              \
  }

/*--------------------------------------------------------------------------*/

/* Code Begins                                                              */

/*--------------------------------------------------------------------------*/

/* Alloc free function */

/* Alloc/free routine */

/*
 * Implement a name cache of a template names.  This allows differences
 * in case and characters past the 14th to be ignored in mech references
 * when loading templates.  Templates can also be stored in any subdirectory
 * of the main template directory instead of in just one of list of hard
 * coded subdirectories.
 *
 * CACHE_MAXNAME sets the limit on how long a template filename can be.
 * Any template with a filename longer than this is ignored and not stored
 * in the cache.
 *
 * MECH_MAXREF sets the number of signficant characters in a mechref when
 * searching the cache.  This should be equal to the length of the
 * 'mech_type' (minus one for the terminating '\0' character) field of
 * MECH structure.
 *
 */

enum { CACHE_MAXNAME = 34, MECH_MAXREF = 24 };

struct tmpldirent {
  char name[CACHE_MAXNAME + 1];
  char const *dir;
};

struct tmpldir {
  char name[CACHE_MAXNAME + 1];
  struct tmpldir *next;
};

struct MechTemplateRegistry {
  struct tmpldirent *templates;
  struct tmpldir *directories;
  size_t template_count;
  size_t template_capacity;
  char resolved_path[1024];
};

/*
 * The ordering function for the template name cache.  Used to sort and
 * search the cache.
 */
static int tmplcmp(void const *v1, void const *v2) {
  struct tmpldirent const *p1 = v1;
  struct tmpldirent const *p2 = v2;

  return strncasecmp(p1->name, p2->name, MECH_MAXREF);
}

/*
 * Add all the template names in a directory to the template cache.
 */
static int scan_template_dir(MechTemplateRegistry *registry,
                             char const *dirname, char const *parent) {
  char buf[1000] = {0};
  int dirnamelen = strlen(dirname);
  DIR *dir = opendir(dirname);

  if (dir == NULL) {
    return -1;
  }

  while (1) {
    struct stat sb;
    struct dirent *ent = readdir(dir);

    if (ent == NULL) {
      break;
    }

    if (dirnamelen + 1 + strlen(ent->d_name) + 1 > sizeof buf) {
      continue;
    }

    snprintf(buf, sizeof(buf), "%s/%s", dirname, ent->d_name);
    if (stat(buf, &sb) == -1) {
      continue;
    }

    if (parent == NULL && S_ISDIR(sb.st_mode) && ent->d_name[0] != '.' &&
        strlen(ent->d_name) <= CACHE_MAXNAME) {
      struct tmpldir *link;

      Create(link, struct tmpldir, 1);

      strcpy(link->name, ent->d_name);
      link->next = registry->directories;
      registry->directories = link;
      continue;
    }

    if (!S_ISREG(sb.st_mode)) {
      continue;
    }

    if (registry->template_count == registry->template_capacity) {
      if (registry->template_capacity == 0) {
        registry->template_capacity = 4;
        Create(registry->templates, struct tmpldirent,
               registry->template_capacity);
      } else {
        registry->template_capacity *= 2;
        ReCreate(registry->templates, struct tmpldirent,
                 registry->template_capacity);
      }
    }

    snprintf(registry->templates[registry->template_count].name,
             sizeof(registry->templates[registry->template_count].name), "%s",
             ent->d_name);
    registry->templates[registry->template_count].dir = parent;
    registry->template_count++;
  }

  closedir(dir);
  return 0;
}

/*
 * Scan all the template names in the mech template directory.  Only looks
 * in the mech template directory and it immediate subdirectories.
 * It doesn't recursively look any further down the tree.
 */

static int scan_templates(MechTemplateRegistry *registry, char const *dir) {
  char buf[1000] = {0};
  struct tmpldir *p;

  if (scan_template_dir(registry, dir, nullptr) == -1) {
    return -1;
  }

  p = registry->directories;
  while (p != nullptr) {
    snprintf(buf, sizeof(buf), "%s/%s", dir, p->name);
    scan_template_dir(registry, buf, p->name);
    p = p->next;
  }

  qsort(registry->templates, registry->template_count,
        sizeof(registry->templates[0]), tmplcmp);

  return 0;
}

/*
 * Free all the memory used by the template cache.  Sets the cache to
 * the empty state.
 */
static void template_registry_clear(MechTemplateRegistry *registry) {
  struct tmpldir *p;

  if (registry == nullptr)
    return;
  free(registry->templates);

  p = registry->directories;
  while (p != nullptr) {
    struct tmpldir *np = p->next;

    free(p);
    p = np;
  }

  *registry = (MechTemplateRegistry){0};
}

static char *const subdirs[] = {
    "3025",   "3050",    "3055",     "3058",         "3060",     "2750",
    "Aero",   "MISC",    "Clan",     "ClanVehicles", "Clan2nd",  "ClanAero",
    "Custom", "Solaris", "Vehicles", "MFNA",         "Infantry", NULL};

void mechrep_Rloadnew(DbRef player, void *data, char *buffer) {
  char *args[1];

  MECHREP_COMMON(1);
  if (mech_parseattributes(buffer, args, 1) == 1)
    if (mech_loadnew(player, mech, args[0]) == 1) {
      mux_event_remove_data(mech->xcode.context->events, mech);
      clear_mech_from_LOS(mech);
      notify(btech_context_evaluation(rep->xcode.context), player,
             "Template loaded.");
      return;
    }
  notify(btech_context_evaluation(rep->xcode.context), player,
         "Unable to read that template.");
}

void clear_mech(Mech *mech, int flag) {
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
  if (flag)
    mech_communications_clear(mech);
}

char *mechref_path(BtechContext *context, const char *mech_path, char *id) {
  MechTemplateRegistry *registry = context->templates;
  FILE *fp;
  int i = 0; /* this int has double use... ugly, but effective */

  if (registry == nullptr) {
    registry = calloc(1, sizeof(*registry));
    if (registry == nullptr)
      return nullptr;
    context->templates = registry;
  }

  /*
   * If the template name doesn't have slash search for it in the
   * template name cache.
   */
redo:
  if (strchr(id, '/') == NULL && (registry->templates != nullptr ||
                                  scan_templates(registry, mech_path) != -1)) {
    struct tmpldirent *ent;
    struct tmpldirent key;

    strncpy(key.name, id, CACHE_MAXNAME);
    key.name[CACHE_MAXNAME] = '\0';

    ent = bsearch(&key, registry->templates, registry->template_count,
                  sizeof(registry->templates[0]), tmplcmp);
    if (ent == NULL) {
      return NULL;
    }
    if (ent->dir == NULL) {
      snprintf(registry->resolved_path, sizeof(registry->resolved_path),
               "%s/%s", mech_path, ent->name);
    } else {
      snprintf(registry->resolved_path, sizeof(registry->resolved_path),
               "%s/%s/%s", mech_path, ent->dir, ent->name);
    }
    if (access(registry->resolved_path, R_OK) != 0) {
      /* The file is missing (or unreadable)
         invalidate the cache and try again,
         if *that* fails, fall back to the old version. */
      if (!i) {
        i = 1;
        template_registry_clear(registry);
        goto redo;
      } else
        goto oldstyle;
    }
    return registry->resolved_path;
  }
oldstyle:
  /*
   * Look up a template name the old way...
   */
  snprintf(registry->resolved_path, sizeof(registry->resolved_path), "%s/%s",
           mech_path, id);
  fp = fopen(registry->resolved_path, "r");
  for (i = 0; !fp && subdirs[i]; i++) {
    snprintf(registry->resolved_path, sizeof(registry->resolved_path),
             "%s/%s/%s", mech_path, subdirs[i], id);
    fp = fopen(registry->resolved_path, "r");
  }
  if (fp) {
    fclose(fp);
    return registry->resolved_path;
  }
  return nullptr;
}

void mech_template_registry_destroy(BtechContext *context) {
  if (context->templates == nullptr)
    return;
  template_registry_clear(context->templates);
  free(context->templates);
  context->templates = nullptr;
}

int load_mechdata2(DbRef player, Mech *mech, char *id) {
  FILE *fp = NULL;
  char *filename;

  filename =
      mechref_path(mech->xcode.context,
                   mech->xcode.context->configuration->database.mech_db, id);

  if (!filename)
    return 0;
  if (!(fp = fopen(filename, "r")))
    return 0;
  fclose(fp);
  return load_template(player, mech, filename) >= 0 ? 1 : 0;
}

extern const int num_def_weapons;

int unable_to_find_proper_type(int i) {
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

static bool mechdata_load_error(FILE *fp, Mech *mech, bool condition,
                                const char *format, ...)
    __attribute__((format(printf, 4, 5)));

static bool mechdata_load_error(FILE *fp, Mech *mech, bool condition,
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
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
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

int load_mechdata(Mech *mech, char *id) {
  FILE *fp = NULL;
  int i, j, k, t;
  int i1, i2, i3, i4, i5, i6;
  char *filename;

  filename =
      mechref_path(mech->xcode.context,
                   mech->xcode.context->configuration->database.mech_db, id);
  if (mechdata_load_error(fp, mech, filename == NULL,
                          "No matching file for '%s'.", id)) {
    return -1;
  }
  if (filename)
    fp = fopen(filename, "r");
  if (mechdata_load_error(fp, mech, !fp, "Unable to open file %s (%s)!",
                          filename, id)) {
    return -1;
  }
  strncpy(MechType_Ref(mech), id, 25);
  MechType_Ref(mech)[24] = '\0';
  if (mechdata_load_error(
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
    if (mechdata_load_error(fp, mech,
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
      if (mechdata_load_error(
              fp, mech, fscanf(fp, "%d %d %d\n", &i1, &i2, &i3) < 3,
              "Insufficient data reading critical %d/%d!", i, j)) {
        return -1;
      }
      MechSections(mech)[i].criticals[j].type = i1;
      if (mechdata_load_error(
              fp, mech, unable_to_find_proper_type(GetPartType(mech, i, j)),
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
    if (mechdata_load_error(fp, mech, MechType(mech) > CLASS_LAST,
                            "Invalid 'mech type!")) {
      return -1;
    }
    MechMove(mech) = i2;
    if (mechdata_load_error(fp, mech, MechMove(mech) > MOVENEMENT_LAST,
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

int mech_loadnew(DbRef player, Mech *mech, char *id) {
  char mech_origid[100];

  strncpy(mech_origid, MechType_Ref(mech), 99);
  mech_origid[99] = '\0';

  if (!strcmp(mech_origid, id)) {
    clear_mech(mech, 0);
    if (load_mechdata2(player, mech, id) <= 0)
      return load_mechdata(mech, id) > 0;
    return 1;
  } else {
    clear_mech(mech, 1);
    if (load_mechdata2(player, mech, id) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
      if (load_mechdata(mech, id) < 1)
#endif
#ifdef LOADNEW_LOADS_OLD_IF_FAIL
        if (load_mechdata2(player, mech, mech_origid) < 1)
#ifdef LOADNEW_LOADS_MUSE_FORMAT
          if (load_mechdata(mech, mech_origid) < 1)
#endif
#endif
            return 0;
  }
  return 1;
}

void mechrep_Rrestore(DbRef player, void *data, char *buffer) {
  char *c;

  MECHREP_COMMON(1);
  c = btech_attribute_read(mech->xcode.context->database, mech->mynum,
                           A_MECHREF, (char[LBUF_SIZE]){0});
  DOCHECK_CONTEXT(rep->xcode.context, !c || !*c,
                  "Sorry, I don't know what type of mech this is");
  DOCHECK_CONTEXT(rep->xcode.context, mech_loadnew(player, mech, c) == 1,
                  "Restoration complete!");
  notify(btech_context_evaluation(rep->xcode.context), player,
         "Unable to restore this mech!.");
}

void mechrep_Rsavetemp(DbRef player, void *data, char *buffer) {
  char *args[1];
  FILE *fp;
  char openfile[512] = {0};
  int i, j;

  MECHREP_COMMON(1);

  template_registry_clear(mech->xcode.context->templates);

  DOCHECK_CONTEXT(rep->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "You must specify a template name!");
  DOCHECK_CONTEXT(rep->xcode.context, strstr(args[0], "/"),
                  "Invalid file name!");
  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Saving %s...", args[0]);
  snprintf(openfile, sizeof(openfile), "%s/",
           mech->xcode.context->configuration->database.mech_db);
  strcat(openfile, args[0]);
  DOCHECK_CONTEXT(rep->xcode.context, !(fp = fopen(openfile, "w")),
                  "Unable to open/create mech file! Sorry.");
  fprintf(fp, "%d %d %d %d %d %.2f %.2f %d\n", MechTons(mech),
          MechTacRange(mech), MechLRSRange(mech), MechScanRange(mech),
          MechRealNumsinks(mech), MechMaxSpeed(mech), MechJumpSpeed(mech),
          MechSpecials(mech));
  for (i = 0; i < NUM_SECTIONS; i++) {
    fprintf(fp, "%d %d %d %d\n", GetSectArmor(mech, i), GetSectInt(mech, i),
            GetSectRArmor(mech, i), MechSections(mech)[i].config);
    for (j = 0; j < NUM_CRITICALS; j++) {
      fprintf(fp, "%d %d %d\n", MechSections(mech)[i].criticals[j].type,
              MechSections(mech)[i].criticals[j].data,
              MechSections(mech)[i].criticals[j].firemode);
    }
  }
  fprintf(fp, "%d %d\n", MechType(mech), MechMove(mech));
  fprintf(fp, "%d\n", MechRadioRange(mech));
  fclose(fp);
  notify(btech_context_evaluation(rep->xcode.context), player,
         "Saving complete!");
}

/*
 * Template saving routines and logic.
 */
void mechrep_Rsavetemp2(DbRef player, void *data, char *buffer) {
  char *args[1];
  char openfile[512] = {0};

  MECHREP_COMMON(1);

  template_registry_clear(mech->xcode.context->templates);

  // No template name given.
  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "You must specify a template name!");
    return;
  }

  // Anti-twink measure. Don't allow directory saving... yet
  if (strstr(args[0], "/")) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Invalid file name!");
    return;
  }

  notify_printf(btech_context_evaluation(rep->xcode.context), player,
                "Saving %s", args[0]);
  snprintf(openfile, sizeof(openfile), "%s/",
           mech->xcode.context->configuration->database.mech_db);
  strcat(openfile, args[0]);

  // Just warn on overweight.
  if (mech_weight_sub(GOD, mech, -1) > (MechTons(mech) * 1024))
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Warning: Template Overweight, see @weight.");

  // I/O or Permissions error.
  if (save_template(player, mech, args[0], openfile) < 0) {
    notify(btech_context_evaluation(rep->xcode.context), player,
           "Error saving the template file!");
    return;
  }

  notify(btech_context_evaluation(rep->xcode.context), player,
         "Saving complete!");
} // end mechrep_Rsavetemp2

/*
 * Emits the valid sections when a player tries to setarmor/addsp/reload an
 * invalid section
 */
