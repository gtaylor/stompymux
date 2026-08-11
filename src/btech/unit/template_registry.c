#include "mech_template_api.h"

#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include "btech/context.h"
#include "mux/support/array_sort.h"
#include "mux/support/checked_storage.h"

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

static TemplateDirectoryEntry *template_entry_at(MechTemplateRegistry *registry,
                                                 size_t index) {
  return checked_storage_at(registry->templates, registry->template_capacity,
                            sizeof(*registry->templates), index);
}

/*
 * The ordering function for the template name cache.  Used to sort and
 * search the cache.
 */
static int tmplcmp(const ArraySortComparison *comparison) {
  const TemplateDirectoryEntry *p1 = comparison->left;
  const TemplateDirectoryEntry *p2 = comparison->right;

  return strncasecmp(p1->name, p2->name, MECH_MAXREF);
}

/*
 * Add all the template names in a directory to the template cache.
 */
typedef struct TemplateDirectoryScanRequest {
  MechTemplateRegistry *registry;
  const char *directory_name;
  const char *parent_name;
} TemplateDirectoryScanRequest;

static int scan_template_dir(const TemplateDirectoryScanRequest *request) {
  MechTemplateRegistry *registry = request->registry;
  const char *dirname = request->directory_name;
  const char *parent = request->parent_name;
  char buf[1000] = {0};
  size_t dirnamelen = strlen(dirname);
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

    (void)snprintf(buf, sizeof(buf), "%s/%s", dirname, ent->d_name);
    if (stat(buf, &sb) == -1) {
      continue;
    }

    if (parent == nullptr && S_ISDIR(sb.st_mode) && ent->d_name[0] != '.' &&
        strlen(ent->d_name) <= CACHE_MAXNAME) {
      TemplateDirectory *link = calloc(1, sizeof(*link));
      if (link == nullptr)
        continue;

      strlcpy(link->name, ent->d_name, sizeof(link->name));
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

    TemplateDirectoryEntry *entry =
        template_entry_at(registry, registry->template_count);
    (void)snprintf(entry->name, sizeof(entry->name), "%s", ent->d_name);
    entry->dir = parent;
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

  if (scan_template_dir(&(TemplateDirectoryScanRequest){
          .registry = registry, .directory_name = dir}) == -1) {
    return -1;
  }

  p = registry->directories;
  while (p != nullptr) {
    (void)snprintf(buf, sizeof(buf), "%s/%s", dir, p->name);
    scan_template_dir(&(TemplateDirectoryScanRequest){
        .registry = registry, .directory_name = buf, .parent_name = p->name});
    p = p->next;
  }

  if (registry->templates != nullptr && registry->template_count > 1) {
    array_sort(&(ArraySortRequest){.items = registry->templates,
                                   .count = registry->template_count,
                                   .item_size = sizeof(registry->templates[0]),
                                   .compare = tmplcmp});
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

static const char *const SUBDIRS[] = {
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
    TemplateDirectoryEntry key;

    if (registry->templates == nullptr || registry->template_count == 0) {
      return nullptr;
    }
    strncpy(key.name, id, CACHE_MAXNAME);
    key.name[CACHE_MAXNAME] = '\0';

    ArraySearchResult search = array_search(
        &(ArraySearchRequest){.key = &key,
                              .items = registry->templates,
                              .count = registry->template_count,
                              .item_size = sizeof(registry->templates[0]),
                              .compare = tmplcmp});
    if (!search.found) {
      return nullptr;
    }
    TemplateDirectoryEntry *ent = template_entry_at(registry, search.index);
    if (ent->dir == nullptr) {
      (void)snprintf(registry->resolved_path, sizeof(registry->resolved_path),
                     "%s/%s", mech_path, ent->name);
    } else {
      (void)snprintf(registry->resolved_path, sizeof(registry->resolved_path),
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
  const size_t SUBDIR_COUNT = sizeof(SUBDIRS) / sizeof(*SUBDIRS) - 1;
  for (size_t subdir_index = 0; !fp && subdir_index < SUBDIR_COUNT;
       subdir_index++) {
    const char *const *subdir = (const char *const *)checked_storage_at_const(
        (const void *)SUBDIRS, SUBDIR_COUNT, sizeof(*SUBDIRS), subdir_index);
    (void)snprintf(registry->resolved_path, sizeof(registry->resolved_path),
                   "%s/%s/%s", mech_path, *subdir, id);
    fp = fopen(registry->resolved_path, "r");
  }
  if (fp) {
    if (fclose(fp) != 0)
      return nullptr;
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
