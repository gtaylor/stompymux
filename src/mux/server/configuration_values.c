/* configuration.c - Configuration parsing and defaults */

#include "mux/server/configuration.h"
#include "mux/server/game.h"
#include "mux/world/player.h"

#include "mux/server/configuration_context.h"
#include "mux/server/configuration_toml.h"
#include "mux/server/platform.h"

#include <arpa/inet.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_runtime.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/configuration_internal.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "mux/server/server_registries.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/styled_text/palette.h"
#include "mux/world/world_context.h"
int cf_int(int *vp, char *str, long extra, DbRef player, char *cmd,
           ConfigurationContext *context) {
  /*
   * Copy the numeric value to the parameter
   */

  sscanf(str, "%d", vp);
  return 0;
}
/* *INDENT-OFF* */

/* ---------------------------------------------------------------------------
 * cf_bool: Set boolean parameter.
 */

NameTable bool_names[] = {
    {"true", 1, 0, 1}, {"false", 1, 0, 0}, {"yes", 1, 0, 1},  {"no", 1, 0, 0},
    {"1", 1, 0, 1},    {"0", 1, 0, 0},     {nullptr, 0, 0, 0}};

/* *INDENT-ON* */

int cf_bool(int *vp, char *str, long extra, DbRef player, char *cmd,
            ConfigurationContext *context) {
  *vp = (int)name_table_search(context->database, context->configuration, GOD,
                               bool_names, str);
  if (*vp < 0)
    *vp = (long)0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * cf_bool_bit: Set or clear one bit in a configuration bitmask from a boolean
 * parameter.
 */

int cf_bool_bit(int *vp, char *str, long extra, DbRef player, char *cmd,
                ConfigurationContext *context) {
  int value;

  value = (int)name_table_search(context->database, context->configuration, GOD,
                                 bool_names, str);
  if (value > 0)
    *vp |= (int)extra;
  else
    *vp &= ~(int)extra;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_string: Set string parameter.
 */

int cf_string(int *vp, char *str, long extra, DbRef player, char *cmd,
              ConfigurationContext *context) {
  int retval;

  /*
   * Copy the string to the buffer if it is not too big
   */

  retval = 0;
  if (strlen(str) >= (size_t)extra) {
    str[extra - 1] = '\0';
    if (context->configuration->is_initializing) {
      log_error(context->log, LOG_STARTUP, "CNF", "NFND",
                "%s: String truncated", cmd);
    } else {
      notify_checked(&context->command->evaluation, player, player,
                     "String truncated", MSG_ME_ALL | MSG_F_DOWN);
    }
    retval = 1;
  }
  StringCopy((char *)vp, str);
  return retval;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_flagalias: define a flag alias.
 */

int cf_flagalias(int *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context) {
  char *alias, *orig;
  int *cp, success;

  success = 0;
  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");

  cp = hash_table_find(orig, &context->world_indexes->flags);
  if (cp != nullptr) {
    hash_table_add(alias, cp, &context->world_indexes->flags);
    success++;
  }
  if (!success)
    configuration_log_not_found(context, player, cmd, "Flag", orig);
  return ((success > 0) ? 0 : -1);
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_modify_bits: set or clear bits in a flag word from a
 * namelist.
 */
int configuration_modify_bits(int *vp, char *str, long extra, DbRef player,
                              char *cmd, ConfigurationContext *context) {
  char *sp;
  int f, negate, success, failure;

  /*
   * Walk through the tokens
   */

  success = failure = 0;
  sp = strtok(str, " \t");
  while (sp != nullptr) {

    /*
     * Check for negation
     */

    negate = 0;
    if (*sp == '!') {
      negate = 1;
      sp++;
    }
    /*
     * Set or clear the appropriate bit
     */

    f = name_table_search(context->database, context->configuration, GOD,
                          (NameTable *)extra, sp);
    if (f > 0) {
      if (negate)
        *vp &= ~f;
      else
        *vp |= f;
      success++;
    } else {
      configuration_log_not_found(context, player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(nullptr, " \t");
  }
  return configuration_status_from_succfail(player, cmd, success, failure,
                                            context);
}

/*
 * ---------------------------------------------------------------------------
 * * cf_set_flags: Clear flag word and then set from a flags htab.
 */

int cf_set_flags(void *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context) {
  char *sp;
  FlagEntry *fp;
  ObjectFlagSet *fset;

  int success, failure;

  /*
   * Walk through the tokens
   */

  success = failure = 0;
  sp = strtok(str, " \t");
  fset = (ObjectFlagSet *)vp;

  while (sp != nullptr) {

    /*
     * Set the appropriate bit
     */

    fp = (FlagEntry *)hash_table_find(sp, &context->world_indexes->flags);
    if (fp != nullptr) {
      if (success == 0)
        *fset = (ObjectFlagSet){0};
      object_flag_set_set(fset, fp->id, true);
      success++;
    } else {
      configuration_log_not_found(context, player, cmd, "Entry", sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok(nullptr, " \t");
  }
  if ((success == 0) && (failure == 0)) {
    *fset = (ObjectFlagSet){0};
    return 0;
  }
  if (success > 0)
    return ((failure == 0) ? 0 : 1);
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_badname: Disallow use of player name/alias.
 */

int cf_badname(int *vp, char *str, long extra, DbRef player, char *cmd,
               ConfigurationContext *context) {
  if (extra)
    badname_remove(context->world, str);
  else
    badname_add(context->world, str);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_site: Update site information
 */

int cf_site(long **vp, char *str, long extra, DbRef player, char *cmd,
            ConfigurationContext *context) {
  SiteData *site, *last, *head;
  char *addr_txt, *mask_txt;
  struct in_addr addr_num, mask_num;

  addr_txt = strtok(str, " \t=,");
  mask_txt = nullptr;
  if (addr_txt)
    mask_txt = strtok(nullptr, " \t=,");
  if (!addr_txt || !*addr_txt || !mask_txt || !*mask_txt) {
    configuration_log_syntax(context, player, cmd,
                             "Missing host address or mask.", "");
    return -1;
  }

  addr_num.s_addr = inet_addr(addr_txt);
  mask_num.s_addr = inet_addr(mask_txt);

  if (addr_num.s_addr == INADDR_NONE) {
    configuration_log_syntax(context, player, cmd,
                             "Bad host address: ", addr_txt);
    return -1;
  }
  head = (SiteData *)*vp;
  /*
   * Parse the access entry and allocate space for it
   */

  site = malloc(sizeof(SiteData));

  /*
   * Initialize the site entry
   */

  site->address.s_addr = addr_num.s_addr;
  site->mask.s_addr = mask_num.s_addr;
  site->flag = (int)extra;
  site->next = nullptr;

  /*
   * Link in the entry.  Link it at the start if not initializing, at *
   *
   * *  * *  * *  * *  * * the end if initializing.  This is so that
   * entries  * in * the * config * * * file are processed as you would
   * think they * * would be, * while * entries * * made while running
   * are processed * * first.
   */

  if (context->configuration->is_initializing) {
    if (head == nullptr) {
      *vp = (long *)site;
    } else {
      for (last = head; last->next; last = last->next)
        ;
      last->next = site;
    }
  } else {
    site->next = head;
    *vp = (long *)site;
  }
  return 0;
}

int cf_named_color(void *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context) {
  char name[61];
  char trailing;
  char error[128];
  int red;
  int green;
  int blue;

  (void)vp;
  (void)extra;
  if (sscanf(str, "%60s %d %d %d %c", name, &red, &green, &blue, &trailing) !=
      4) {
    configuration_log_syntax(context, player, cmd,
                             "Expected NAME RED GREEN BLUE: ", str);
    return -1;
  }
  if (!styled_text_palette_set_rgb(context->world->styled_text_palette, name,
                                   red, green, blue, error, sizeof(error))) {
    configuration_log_syntax(context, player, cmd, error, "");
    if (context->configuration->is_initializing)
      context->fatal_error = true;
    return -1;
  }
  return 0;
}

int cf_osc8_preset(void *vp, char *str, long extra, DbRef player, char *cmd,
                   ConfigurationContext *context) {
  char *directives;
  char error[256];

  (void)vp;
  (void)extra;
  directives = str;
  while (*directives && !isspace((unsigned char)*directives))
    directives++;
  if (!*directives) {
    configuration_log_syntax(context, player, cmd,
                             "Expected NAME DIRECTIVES: ", str);
    if (context->configuration->is_initializing)
      context->fatal_error = true;
    return -1;
  }
  *directives++ = '\0';
  while (isspace((unsigned char)*directives))
    directives++;
  if (!styled_text_palette_set_preset(context->world->styled_text_palette, str,
                                      directives, error, sizeof(error))) {
    configuration_log_syntax(context, player, cmd, error, "");
    if (context->configuration->is_initializing)
      context->fatal_error = true;
    return -1;
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_cf_access: Set access on config directives
 */

int cf_cf_access(int *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context) {
  CONF *tp;
  char *ap;

  for (ap = str; *ap && !isspace((unsigned char)*ap); ap++)
    ;
  if (*ap)
    *ap++ = '\0';

  for (tp = conftable; tp->pname; tp++) {
    if (!strcmp(tp->pname, str)) {
      return configuration_modify_bits(&tp->flags, ap, extra, player, cmd,
                                       context);
    }
  }
  configuration_log_not_found(context, player, cmd, "Config directive", str);
  return -1;
}

/* ---------------------------------------------------------------------------
 * conftable: Table for parsing the configuration file.
 */
