#include "mech_template_api.h"

#include <dirent.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btech/context.h"

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

typedef struct TemplateDirectoryEntry {
  char name[CACHE_MAXNAME + 1];
  char const *dir;
} TemplateDirectoryEntry;

typedef struct TemplateDirectory {
  char name[CACHE_MAXNAME + 1];
  struct TemplateDirectory *next;
} TemplateDirectory;

struct MechTemplateRegistry {
  TemplateDirectoryEntry *templates;
  TemplateDirectory *directories;
  size_t template_count;
  size_t template_capacity;
  char resolved_path[1024];
};

/*
 * The ordering function for the template name cache.  Used to sort and
 * search the cache.
 */
static int tmplcmp(void const *v1, void const *v2) {
  TemplateDirectoryEntry const *p1 = v1;
  TemplateDirectoryEntry const *p2 = v2;

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

  if (dir == nullptr) {
    return -1;
  }

  while (1) {
    struct stat sb;
    struct dirent *ent = readdir(dir);

    if (ent == nullptr) {
      break;
    }

    if (dirnamelen + 1 + strlen(ent->d_name) + 1 > sizeof buf) {
      continue;
    }

    snprintf(buf, sizeof(buf), "%s/%s", dirname, ent->d_name);
    if (stat(buf, &sb) == -1) {
      continue;
    }

    if (parent == nullptr && S_ISDIR(sb.st_mode) && ent->d_name[0] != '.' &&
        strlen(ent->d_name) <= CACHE_MAXNAME) {
      TemplateDirectory *link = calloc(1, sizeof(*link));
      if (link == nullptr)
        continue;

      strcpy(link->name, ent->d_name);
      link->next = registry->directories;
      registry->directories = link;
      continue;
    }

    if (!S_ISREG(sb.st_mode)) {
      continue;
    }

    if (registry->templates == nullptr ||
        registry->template_count >= registry->template_capacity) {
      size_t capacity = registry->template_capacity == 0
                            ? 4
                            : registry->template_capacity * 2;
      TemplateDirectoryEntry *templates =
          realloc(registry->templates, capacity * sizeof(*templates));

      if (templates == nullptr) {
        closedir(dir);
        return -1;
      }
      registry->templates = templates;
      registry->template_capacity = capacity;
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
  TemplateDirectory *p;

  if (scan_template_dir(registry, dir, nullptr) == -1) {
    return -1;
  }

  p = registry->directories;
  while (p != nullptr) {
    snprintf(buf, sizeof(buf), "%s/%s", dir, p->name);
    scan_template_dir(registry, buf, p->name);
    p = p->next;
  }

  if (registry->templates != nullptr && registry->template_count > 1) {
    qsort(registry->templates, registry->template_count,
          sizeof(registry->templates[0]), tmplcmp);
  }

  return 0;
}

/*
 * Free all the memory used by the template cache.  Sets the cache to
 * the empty state.
 */
static void template_registry_reset(MechTemplateRegistry *registry) {
  TemplateDirectory *p;

  if (registry == nullptr)
    return;
  free(registry->templates);

  p = registry->directories;
  while (p != nullptr) {
    TemplateDirectory *np = p->next;

    free(p);
    p = np;
  }

  *registry = (MechTemplateRegistry){0};
}

static char *const subdirs[] = {
    "3025",   "3050",    "3055",     "3058",         "3060",     "2750",
    "Aero",   "MISC",    "Clan",     "ClanVehicles", "Clan2nd",  "ClanAero",
    "Custom", "Solaris", "Vehicles", "MFNA",         "Infantry", nullptr};

char *mech_template_resolve_path(BtechContext *context, const char *mech_path,
                                 const char *id) {
  MechTemplateRegistry *registry =
      btech_context_mech_template_registry(context);
  FILE *fp;
  int i = 0; /* this int has double use... ugly, but effective */

  if (registry == nullptr) {
    registry = calloc(1, sizeof(*registry));
    if (registry == nullptr)
      return nullptr;
    btech_context_mech_template_registry_set(context, registry);
  }

  /*
   * If the template name doesn't have slash search for it in the
   * template name cache.
   */
redo:
  if (strchr(id, '/') == nullptr &&
      (registry->templates != nullptr ||
       scan_templates(registry, mech_path) != -1)) {
    TemplateDirectoryEntry *ent;
    TemplateDirectoryEntry key;

    if (registry->templates == nullptr || registry->template_count == 0) {
      return nullptr;
    }
    strncpy(key.name, id, CACHE_MAXNAME);
    key.name[CACHE_MAXNAME] = '\0';

    ent = bsearch(&key, registry->templates, registry->template_count,
                  sizeof(registry->templates[0]), tmplcmp);
    if (ent == nullptr) {
      return nullptr;
    }
    if (ent->dir == nullptr) {
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
        template_registry_reset(registry);
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
  if (btech_context_mech_template_registry(context) == nullptr)
    return;
  template_registry_reset(btech_context_mech_template_registry(context));
  free(btech_context_mech_template_registry(context));
  btech_context_mech_template_registry_set(context, nullptr);
}

void mech_template_registry_clear(BtechContext *context) {
  template_registry_reset(btech_context_mech_template_registry(context));
}
