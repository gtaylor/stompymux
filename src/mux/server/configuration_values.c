/* configuration.c - Configuration parsing and defaults */

#include <arpa/inet.h>
#include <ctype.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/objects/flags.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_internal.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/palette.h"
#include "mux/world/player.h"

int cf_int(int *vp, char *str, long extra, DbRef player, char *cmd,
           ConfigurationContext *context) {
  /*
   * Copy the numeric value to the parameter
   */

  if (parse_int_checked(str, vp))
    return 0;
  configuration_log_syntax(context, player, cmd, "Expected integer: ", str);
  return -1;
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
  if (extra <= 0)
    return -1;
  if (strlen(str) >= (size_t)extra) {
    *(char *)checked_storage_at(str, strlen(str) + 1, sizeof(char),
                                (size_t)extra - 1) = '\0';
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
      sp = checked_mutable_string_suffix(sp, 1);
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
  char *name;
  char *red_text;
  char *green_text;
  char *blue_text;
  char error[128];
  int red;
  int green;
  int blue;

  (void)vp;
  (void)extra;
  name = strtok(str, " \t");
  red_text = strtok(nullptr, " \t");
  green_text = strtok(nullptr, " \t");
  blue_text = strtok(nullptr, " \t");
  if (name == nullptr || strlen(name) > 60 || red_text == nullptr ||
      blue_text == nullptr || strtok(nullptr, " \t") != nullptr ||
      !parse_int_checked(red_text, &red) ||
      !parse_int_checked(green_text, &green) ||
      !parse_int_checked(blue_text, &blue)) {
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
  size_t length = strlen(str);
  size_t offset = 0;

  (void)vp;
  (void)extra;
  while (offset < length &&
         !(isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  if (offset == length) {
    configuration_log_syntax(context, player, cmd,
                             "Expected NAME DIRECTIVES: ", str);
    if (context->configuration->is_initializing)
      context->fatal_error = true;
    return -1;
  }
  *(char *)checked_storage_at(str, length + 1, sizeof(char), offset) = '\0';
  offset++;
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  directives = checked_storage_at(str, length + 1, sizeof(char), offset);
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
  char *ap;
  size_t length = strlen(str);
  size_t offset = 0;

  while (offset < length &&
         !(isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  if (offset < length) {
    *(char *)checked_storage_at(str, length + 1, sizeof(char), offset) = '\0';
    offset++;
  }
  ap = checked_storage_at(str, length + 1, sizeof(char), offset);

  for (size_t index = 0; index < configuration_entry_count(); index++) {
    CONF *tp = configuration_entry_at(index);
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
