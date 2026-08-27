/* configuration.c - Configuration parsing and defaults */

#include <arpa/inet.h>
#include <ctype.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_internal.h"
#include "mux/server/configuration_interpreter.h"
#include "mux/server/configuration_registry.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/palette.h"
#include "mux/world/player.h"

int cf_bootstrap_objects_clear(const ConfigurationCall *call) {
  call->context->configuration->database.bootstrap_object_count = 0;
  return 0;
}

int cf_bootstrap_object(const ConfigurationCall *call) {
  DatabaseConfiguration *database = &call->context->configuration->database;
  BootstrapObjectConfiguration object;
  size_t text_length = strlen(call->text);
  char *type_separator = strchr(call->text, ' ');
  char *type;
  char *wizard_separator;
  char *name_separator;
  char *wizard;
  const char *name;

  if (!type_separator) {
    call->context->fatal_error = true;
    return -1;
  }
  *type_separator = '\0';
  type = checked_storage_at(call->text, text_length + 1, sizeof(char),
                            strlen(call->text) + 1);
  wizard_separator = strchr(type, ' ');
  if (!wizard_separator) {
    call->context->fatal_error = true;
    return -1;
  }
  *wizard_separator = '\0';
  wizard =
      checked_storage_at(type, text_length + 1 - (size_t)(type - call->text),
                         sizeof(char), strlen(type) + 1);
  name_separator = strchr(wizard, ' ');
  if (!name_separator) {
    call->context->fatal_error = true;
    return -1;
  }
  *name_separator = '\0';
  name = checked_storage_at_const(call->text, text_length + 1, sizeof(char),
                                  (size_t)(name_separator - call->text) + 1);
  if (!parse_long_checked(call->text, &object.dbref) || object.dbref < 0 ||
      !*name || database->bootstrap_object_count >= MAX_BOOTSTRAP_OBJECTS) {
    call->context->fatal_error = true;
    return -1;
  }
  if (!strcmp(type, "room")) {
    object.type = BOOTSTRAP_OBJECT_ROOM;
  } else if (!strcmp(type, "player")) {
    object.type = BOOTSTRAP_OBJECT_PLAYER;
  } else {
    call->context->fatal_error = true;
    return -1;
  }
  if (!strcmp(wizard, "true")) {
    object.wizard = true;
  } else if (!strcmp(wizard, "false")) {
    object.wizard = false;
  } else {
    call->context->fatal_error = true;
    return -1;
  }
  if (object.wizard && object.type != BOOTSTRAP_OBJECT_PLAYER) {
    call->context->fatal_error = true;
    return -1;
  }
  for (size_t index = 0; index < database->bootstrap_object_count; index++) {
    BootstrapObjectConfiguration *existing =
        checked_storage_at(database->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
                           sizeof(*database->bootstrap_objects), index);
    if (existing->dbref == object.dbref) {
      call->context->fatal_error = true;
      return -1;
    }
  }
  (void)snprintf(object.name, sizeof(object.name), "%s", name);
  BootstrapObjectConfiguration *slot = checked_storage_at(
      database->bootstrap_objects, MAX_BOOTSTRAP_OBJECTS,
      sizeof(*database->bootstrap_objects), database->bootstrap_object_count);
  *slot = object;
  database->bootstrap_object_count++;
  return 0;
}

int cf_ntab_access(const ConfigurationCall *call) {
  char *name = call->text;
  size_t length = strlen(name);
  size_t offset = 0;

  while (offset < length &&
         !(isspace)(*(const unsigned char *)checked_storage_at_const(
             name, length, sizeof(char), offset)))
    offset++;
  if (offset < length) {
    *(char *)checked_storage_at(name, length + 1, sizeof(char), offset) = '\0';
    offset++;
  }
  while (offset < length &&
         (isspace)(*(const unsigned char *)checked_storage_at_const(
             name, length, sizeof(char), offset)))
    offset++;
  char *access = checked_storage_at(name, length + 1, sizeof(char), offset);
  NameTable *entry = name_table_find_match(call->value, name);
  if (entry != nullptr) {
    ConfigurationCall modify_call = *call;
    modify_call.value = &entry->perm;
    modify_call.text = access;
    return configuration_modify_bits(&modify_call);
  }
  configuration_log_not_found(call->context, call->player, call->command,
                              "Entry", name);
  return -1;
}

int cf_int(const ConfigurationCall *call) {
  int *vp = call->value;
  /*
   * Copy the numeric value to the parameter
   */

  if (parse_int_checked(call->text, vp))
    return 0;
  configuration_log_syntax(call->context, call->player, call->command,
                           "Expected integer: ", call->text);
  return -1;
}

int cf_lua_error_reporting(const ConfigurationCall *call) {
  LuaErrorReporting *value = call->value;

  if (!strcmp(call->text, "off")) {
    *value = LUA_ERROR_REPORTING_OFF;
  } else if (!strcmp(call->text, "wizards")) {
    *value = LUA_ERROR_REPORTING_WIZARDS;
  } else if (!strcmp(call->text, "all")) {
    *value = LUA_ERROR_REPORTING_ALL;
  } else {
    configuration_log_syntax(call->context, call->player, call->command,
                             "Expected off, wizards, or all: ", call->text);
    return -1;
  }
  return 0;
}

int cf_player_name_length_limit(const ConfigurationCall *call) {
  char expectation[96];
  int *value = call->value;
  int parsed;

  if (parse_int_checked(call->text, &parsed) && parsed >= 2 &&
      parsed <= (int)PLAYER_NAME_STORAGE_LIMIT) {
    *value = parsed;
    return 0;
  }
  (void)snprintf(
      expectation, sizeof(expectation),
      "Expected an integer from 2 through %zu: ", PLAYER_NAME_STORAGE_LIMIT);
  configuration_log_syntax(call->context, call->player, call->command,
                           expectation, call->text);
  return -1;
}

int cf_techtime_multiplier(const ConfigurationCall *call) {
  double *value = call->value;
  float parsed;

  if (parse_float_checked(call->text, &parsed) && parsed >= 0.0F &&
      parsed <= 10.0F) {
    *value = (double)parsed;
    return 0;
  }
  configuration_log_syntax(
      call->context, call->player, call->command,
      "Expected a finite number from 0.0 through 10.0: ", call->text);
  return -1;
}
/* *INDENT-OFF* */

/* ---------------------------------------------------------------------------
 * cf_bool: Set boolean parameter.
 */

static const NameTable BOOL_NAMES[] = {
    {"true", 1, 0, 1}, {"false", 1, 0, 0}, {"yes", 1, 0, 1},  {"no", 1, 0, 0},
    {"1", 1, 0, 1},    {"0", 1, 0, 0},     {nullptr, 0, 0, 0}};

/* *INDENT-ON* */

int cf_bool(const ConfigurationCall *call) {
  int *vp = call->value;
  *vp = name_table_search(call->context->database, call->context->configuration,
                          GOD, BOOL_NAMES, call->text);
  if (*vp < 0)
    *vp = (long)0;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * cf_bool_bit: Set or clear one bit in a configuration bitmask from a boolean
 * parameter.
 */

int cf_bool_bit(const ConfigurationCall *call) {
  int *vp = call->value;
  int value;

  value =
      name_table_search(call->context->database, call->context->configuration,
                        GOD, BOOL_NAMES, call->text);
  if (value > 0)
    *vp |= (int)call->extra;
  else
    *vp &= ~(int)call->extra;
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_string: Set string parameter.
 */

int cf_string(const ConfigurationCall *call) {
  char *destination = call->value;
  char *str = call->text;
  int retval;

  /*
   * Copy the string to the buffer if it is not too big
   */

  retval = 0;
  if (call->extra <= 0)
    return -1;
  if (strlen(str) >= (size_t)call->extra) {
    *(char *)checked_storage_at(str, strlen(str) + 1, sizeof(char),
                                (size_t)call->extra - 1) = '\0';
    if (call->context->configuration->is_initializing) {
      log_error((LogEntry){.log = call->context->log,
                           .key = LOG_STARTUP,
                           .primary = "CNF",
                           .secondary = "NFND"},
                "%s: String truncated", call->command);
    } else {
      notify_checked(&call->context->command->evaluation, call->player,
                     call->player, "String truncated", MSG_ME_ALL | MSG_F_DOWN);
    }
    retval = 1;
  }
  (void)string_copy_bounded(destination, (size_t)call->extra, str);
  return retval;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_flagalias: define a flag alias.
 */

int cf_flagalias(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *alias;
  char *orig;
  const FlagEntry *flag;
  int success;
  char *token_context = nullptr;

  success = 0;
  alias = strtok_r(str, " \t=,", &token_context);
  orig = strtok_r(nullptr, " \t=,", &token_context);

  flag = find_flag(context->world_indexes, NOTHING, orig);
  if (flag == nullptr) {
    configuration_log_not_found(context, call->player, call->command, "Flag",
                                orig != nullptr ? orig : "");
  } else if (!flag_alias_add(context->world_indexes, alias, flag)) {
    configuration_log_syntax(
        context, call->player, call->command,
        "Invalid or conflicting flag alias: ", alias != nullptr ? alias : "");
  } else {
    success++;
  }
  return ((success > 0) ? 0 : -1);
}

/*
 * ---------------------------------------------------------------------------
 * * configuration_modify_bits: set or clear bits in a flag word from a
 * namelist.
 */
int configuration_modify_bits(const ConfigurationCall *call) {
  int *vp = call->value;
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *sp;
  int f;
  int negate;
  int success;
  int failure;
  const NameTable *table;

  switch ((ConfigurationNameTableId)call->extra) {
  case CONFIGURATION_NAMETAB_ACCESS:
    table = ACCESS_NAMETAB;
    break;
  case CONFIGURATION_NAMETAB_LOGDATA:
    table = LOGDATA_NAMETAB;
    break;
  default:
    configuration_log_not_found(context, call->player, call->command,
                                "Name table", "internal identifier");
    return -1;
  }

  /*
   * Walk through the tokens
   */

  char *token_context = nullptr;
  success = failure = 0;
  sp = strtok_r(str, " \t", &token_context);
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

    f = name_table_search(context->database, context->configuration, GOD, table,
                          sp);
    if (f > 0) {
      if (negate)
        *vp &= ~f;
      else
        *vp |= f;
      success++;
    } else {
      configuration_log_not_found(context, call->player, call->command, "Entry",
                                  sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok_r(nullptr, " \t", &token_context);
  }
  return configuration_status_from_counts(
      call, (ConfigurationParseCounts){.success = success, .failure = failure});
}

/*
 * ---------------------------------------------------------------------------
 * * cf_set_flags: Clear flag word and then set from a flags htab.
 */

int cf_set_flags(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *sp;
  const FlagEntry *fp;
  ObjectFlagSet *fset;

  int success;
  int failure;

  /*
   * Walk through the tokens
   */

  char *token_context = nullptr;
  success = failure = 0;
  sp = strtok_r(str, " \t", &token_context);
  fset = call->value;

  while (sp != nullptr) {

    /*
     * Set the appropriate bit
     */

    fp = find_flag(context->world_indexes, NOTHING, sp);
    if (fp != nullptr) {
      if (success == 0)
        *fset = (ObjectFlagSet){};
      object_flag_set_set(fset, fp->id, true);
      success++;
    } else {
      configuration_log_not_found(context, call->player, call->command, "Entry",
                                  sp);
      failure++;
    }

    /*
     * Get the next token
     */

    sp = strtok_r(nullptr, " \t", &token_context);
  }
  if ((success == 0) && (failure == 0)) {
    *fset = (ObjectFlagSet){};
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

int cf_badname(const ConfigurationCall *call) {
  if (call->extra)
    badname_remove(call->context->world, call->text);
  else
    badname_add(call->context->world, call->text);
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_site: Update site information
 */

int cf_site(const ConfigurationCall *call) {
  long **vp = (long **)call->value;
  char *str = call->text;
  ConfigurationContext *context = call->context;
  SiteData *site;
  SiteData *last;
  SiteData *head;
  char *addr_txt;
  char *mask_txt;
  struct in_addr addr_num;
  struct in_addr mask_num;
  char *token_context = nullptr;

  addr_txt = strtok_r(str, " \t=,", &token_context);
  mask_txt = nullptr;
  if (addr_txt)
    mask_txt = strtok_r(nullptr, " \t=,", &token_context);
  if (!addr_txt || !*addr_txt || !mask_txt || !*mask_txt) {
    configuration_log_syntax(context, call->player, call->command,
                             "Missing host address or mask.", "");
    return -1;
  }

  addr_num.s_addr = inet_addr(addr_txt);
  mask_num.s_addr = inet_addr(mask_txt);

  if (addr_num.s_addr == INADDR_NONE) {
    configuration_log_syntax(context, call->player, call->command,
                             "Bad host address: ", addr_txt);
    return -1;
  }
  head = (SiteData *)*vp;
  /*
   * Parse the access entry and allocate space for it
   */

  site = checked_storage_allocate(sizeof(SiteData));

  /*
   * Initialize the site entry
   */

  site->address.s_addr = addr_num.s_addr;
  site->mask.s_addr = mask_num.s_addr;
  site->flag = (int)call->extra;
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

int cf_named_color(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *name;
  char *red_text;
  char *green_text;
  char *blue_text;
  char error[128];
  int red;
  int green;
  int blue;
  char *token_context = nullptr;

  name = strtok_r(str, " \t", &token_context);
  red_text = strtok_r(nullptr, " \t", &token_context);
  green_text = strtok_r(nullptr, " \t", &token_context);
  blue_text = strtok_r(nullptr, " \t", &token_context);
  if (name == nullptr || strlen(name) > 60 || red_text == nullptr ||
      blue_text == nullptr ||
      strtok_r(nullptr, " \t", &token_context) != nullptr ||
      !parse_int_checked(red_text, &red) ||
      !parse_int_checked(green_text, &green) ||
      !parse_int_checked(blue_text, &blue)) {
    configuration_log_syntax(context, call->player, call->command,
                             "Expected NAME RED GREEN BLUE: ", str);
    return -1;
  }
  if (!styled_text_palette_set_rgb(context->world->styled_text_palette, name,
                                   red, green, blue, error, sizeof(error))) {
    configuration_log_syntax(context, call->player, call->command, error, "");
    if (context->configuration->is_initializing)
      context->fatal_error = true;
    return -1;
  }
  return 0;
}

int cf_osc8_preset(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  char *directives;
  char error[256];
  size_t length = strlen(str);
  size_t offset = 0;

  while (offset < length &&
         !(isspace)(*(const unsigned char *)checked_storage_at_const(
             str, length, sizeof(char), offset)))
    offset++;
  if (offset == length) {
    configuration_log_syntax(context, call->player, call->command,
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
  if (!styled_text_palette_set_preset(
          context->world->styled_text_palette,
          &(StyledPresetDefinition){.name = str,
                                    .directives = directives,
                                    .error = error,
                                    .error_size = sizeof(error)})) {
    configuration_log_syntax(context, call->player, call->command, error, "");
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

int cf_cf_access(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
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

  for (size_t index = 0; index < configuration_registry_entry_count(
                                     context->configuration_registry);
       index++) {
    CONF *tp =
        configuration_registry_entry_at(context->configuration_registry, index);
    if (!strcmp(tp->pname, str)) {
      ConfigurationCall modify_call = *call;
      modify_call.value = &tp->flags;
      modify_call.text = ap;
      return configuration_modify_bits(&modify_call);
    }
  }
  configuration_log_not_found(context, call->player, call->command,
                              "Config directive", str);
  return -1;
}
