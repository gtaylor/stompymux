#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/*
 * $Id: registry.c,v 1.4 2005/08/08 09:43:09 murrayma Exp $
 *
 * Original author: unknown
 *
 * Copyright (c) 1996-2002 Markus Stenberg
 * Copyright (c) 1998-2002 Thomas Wouters
 * Copyright (c) 2000-2002 Cord Awtry
 *
 * Last modified: Thu Jul  9 02:40:16 1998 fingon
 *
 * This includes the basic code to allow objects to have hardcoded
 * commands / properties.
 *
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "ds_turret_api.h"
#include "map_dynamic_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "coolmenu.h"
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_internal.h"
#include "turret.h"

static const char *command_help_message(int special_type, int command) {
  const BtechCommandDefinition *definition =
      btech_special_command_definition(special_type, (size_t)command);
  const char *message = definition->helpmsg;
  return checked_string_suffix(message, *message == '@' ? 1 : 0);
}

void center_string(char *destination, size_t destination_size,
                   const char *source, int width) {
  if (destination == nullptr || destination_size == 0)
    return;

  size_t source_length = strlen(source);
  size_t padding = 0;
  if (width > 0 && (size_t)width > source_length)
    padding = ((size_t)width - source_length) / 2;
  if (padding > destination_size - 1)
    padding = destination_size - 1;
  memset(destination, ' ', padding);
  snprintf(checked_storage_region(destination, destination_size, padding,
                                  destination_size - padding),
           destination_size - padding, "%s", source);
}

static void help_color_initialize(const char *from, char *to) {
  char buf[LBUF_SIZE];
  char *tp = to;

  const size_t first_word_length = strcspn(from, " ");
  if (*checked_string_suffix(from, first_word_length) != '\0') {

    strncpy(buf, from, first_word_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                first_word_length) = '\0';
    safe_str("[fg=blue bold]", to, &tp);
    safe_str(buf, to, &tp);
    safe_str("[reset] ", to, &tp);
    safe_str(checked_string_suffix(from, first_word_length + 1), to, &tp);

    /*      from[i]=' '; */
  } else {
    safe_str("[fg=cyan]", to, &tp);
    safe_str(from, to, &tp);
    safe_str("[reset]", to, &tp);
  }
  *tp = '\0';
}

#define ONE_LINE_TEXTS

#ifdef ONE_LINE_TEXTS
#define MLen CM_ONE
#else
#define MLen CM_TWO
#endif

static const char *do_ugly_things(CoolMenu **d, const char *msg, int len,
                                  int initial) {
  CoolMenu *c = *d;
  size_t msg_len;
  char buf[LBUF_SIZE];
  size_t text_length;

  /* XXX: Not entirely sure what this is for.  */
#ifndef ONE_LINE_TEXTS
  if (!msg) {
    cool_menu_add_with_flags(&c, " ", MLen);
    *d = c;
    return NULL;
  }
#endif

  /*
   * Split off at last space on a line, taking into account initial
   * indentation, etc.  Help messages are strings of words, separated by
   * at most one space, with no word longer than len.
   *
   * All of these assumptions are necessary for this code to be safe.
   * Basically, the code needs to find the breaking space.
   *
   * FIXME: All of this code really needs more cleanup and fixing.
   */
  msg_len = strlen(msg);

  size_t break_offset = msg_len;
  if (msg_len > (size_t)len) {
    break_offset = (size_t)len - 1;
    while (break_offset > 0 && *checked_string_suffix(msg, break_offset) != ' ')
      --break_offset;
    if (break_offset == 0)
      break_offset = (size_t)len;
  }

  if (initial > 0) {
    /* Colorize header line.  */
    help_color_initialize(msg, buf);
  } else if (initial < 0) {
    /* Write indented line.  */
    const size_t indentation = (size_t)(-initial);
    text_length = break_offset;
    memset(buf, ' ', indentation);
    memcpy(checked_storage_region(buf, sizeof(buf), indentation, text_length),
           msg, text_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                text_length + indentation) = '\0';
  } else {
    /* Write unindented line.  */
    text_length = break_offset;
    memcpy(buf, msg, text_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char), text_length) =
        '\0';
  }

  cool_menu_add_with_flags(&c, buf, MLen);

  /* Move pointer to start of next line.  */
  if (*checked_string_suffix(msg, break_offset) == ' ')
    ++break_offset;

  *d = c;
  const char *remainder = checked_string_suffix(msg, break_offset);
  return *remainder ? remainder : NULL;
}

static int help_text_length(const char *text) {
  return text == nullptr || *text == '\0' ? 0 : (int)strlen(text);
}

static constexpr int TAB = 3;

static void cut_apart_helpmsgs(CoolMenu **d, const char *msg1, const char *msg2,
                               int len, int initial) {
  int l1 = help_text_length(msg1);
  int l2 = help_text_length(msg2);
  int nl1, nl2;

#ifndef ONE_LINE_TEXTS

  msg1 = do_ugly_things(d, msg1, len, initial);
  msg2 =
      do_ugly_things(d, msg2, initial ? len : len - TAB, initial ? 0 : 0 - TAB);
  if (!msg1 && !msg2)
    return;
  nl1 = help_text_length(msg1);
  nl2 = help_text_length(msg2);
  if (nl1 != l1 || nl2 != l2) /* To prevent infinite loops */
    cut_apart_helpmsgs(d, msg1, msg2, len, 0);
#else
  int first = 1;

  while (msg1 && *msg1) {
    msg1 = do_ugly_things(d, msg1, len * 2 - 1, first);
    nl1 = help_text_length(msg1);
    if (nl1 == l1)
      break;
    l1 = nl1;
    first = 0;
  }
  while (msg2 && *msg2) {
    msg2 = do_ugly_things(d, msg2, len * 2 - TAB, 0 - TAB);
    nl2 = help_text_length(msg2);
    if (nl2 == l2)
      break;
    l2 = nl2;
  }

#endif
}

typedef struct HelpSection {
  int start;
  int length;
} HelpSection;

static HelpSection *help_section(HelpSection *sections, int index) {
  return checked_storage_at(sections, 100, sizeof(*sections), (size_t)index);
}

void btech_special_object_help(BtechContext *context, DbRef player,
                               const char *type, int id, DbRef loc,
                               PowerId powerneeded, DbRef objid, char *arg) {
  int i, j;
  Mech *mech = NULL;
  HelpSection sections[100];
  int count = 0, csho = 0;
  CoolMenu *c = NULL;
  char buf[LBUF_SIZE];
  int dc;

  if (id == GTYPE_MECH)
    mech = btech_context_get_mech(context, loc);
  bzero(sections, sizeof(sections));
  const int command_count = (int)btech_special_command_count(id);
  for (i = 0; i < command_count; i++) {
    const BtechCommandDefinition *command =
        btech_special_command_definition(id, (size_t)i);
    if (!btech_command_definition_has_handler(command) &&
        (*command->helpmsg != '@' ||
         btech_special_command_access(context, player, powerneeded)))
      if (id != GTYPE_MECH ||
          btech_command_allowed_for_mech(mech, command->flag)) {
        if (count)
          help_section(sections, count - 1)->length =
              i - help_section(sections, count - 1)->start;
        help_section(sections, count)->start = i;
        count++;
      }
  }
  if (count)
    help_section(sections, count - 1)->length =
        i - help_section(sections, count - 1)->start;
  else {
    help_section(sections, 0)->start = 0;
    help_section(sections, 0)->length = i;
    count = 1;
  }
  cool_menu_add_with_flags(&c, NULL, CM_ONE | CM_LINE);
  if (!arg || !*arg) {
    for (i = 0; i < count; i++) {
      if (count > 1) {
        center_string(
            buf, sizeof(buf),
            command_help_message(id, help_section(sections, i)->start), 70);
        cool_menu_add_with_flags(
            &c, tprintf("%s%s%s", "[fg=green]", buf, "[reset]"), CM_ONE);
      } else
        cool_menu_add_with_flags(&c, tprintf("%s command listing: ", type),
                                 CM_ONE | CM_CENTER);
      const HelpSection *section = help_section(sections, i);
      for (j = section->start + (count == 1 ? 0 : 1);
           j < section->start + section->length; j++) {
        const BtechCommandDefinition *command =
            btech_special_command_definition(id, (size_t)j);
        if (*command->helpmsg != '@' ||
            btech_special_command_access(context, player, powerneeded))
          if (id != GTYPE_MECH ||
              btech_command_allowed_for_mech(mech, command->flag)) {
            strlcpy(buf, command->name, sizeof(buf));
            const size_t name_length = strcspn(buf, " ");
            *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                        name_length) = '\0';
            cool_menu_add_with_flags(&c, buf, CM_FOUR);
            csho++;
          }
      }
    }
    if (!csho)
      cool_menu_add_text(
          &c, tprintf("There are no commands you are authorized to use here."));
    else {
      cool_menu_add_with_flags(&c, NULL, CM_ONE | CM_LINE);
      if (count > 1)
        cool_menu_add_text(&c,
                           "Additional info available with 'HELP SUBTOPIC'");
      else
        cool_menu_add_text(&c, "Additional info available with 'HELP ALL'");
    }
  } else {
    /* Try to find matching subtopic, or ALL */
    if (!strcasecmp(arg, "all")) {
      if (count > 1) {
        cool_menu_add_text(&c,
                           "ALL not available for objects with subcategories.");
        dc = -2;
      } else
        dc = -1;
    } else {
      if (count == 1) {
        cool_menu_add_text(
            &c,
            "This object doesn't have any other detailed help than 'HELP ALL'");
        dc = -2;
      } else {
        for (i = 0; i < count; i++)
          if (!strcasecmp(arg, command_help_message(
                                   id, help_section(sections, i)->start)))
            break;
        if (i == count) {
          cool_menu_add_text(&c, "Subcategory not found.");
          dc = -2;
        } else
          dc = i;
      }
    }
    if (dc > -2) {
      for (i = 0; i < count; i++)
        if (dc == -1 || i == dc) {
          if (count > 1) {
            center_string(
                buf, sizeof(buf),
                command_help_message(id, help_section(sections, i)->start), 70);
            cool_menu_add_text(&c,
                               tprintf("%s%s%s", "[fg=green]", buf, "[reset]"));
          }
          const HelpSection *section = help_section(sections, i);
          for (j = section->start + (count == 1 ? 0 : 1);
               j < section->start + section->length; j++) {
            const BtechCommandDefinition *command =
                btech_special_command_definition(id, (size_t)j);
            if (*command->helpmsg != '@' ||
                btech_special_command_access(context, player, powerneeded))
              if (id != GTYPE_MECH ||
                  btech_command_allowed_for_mech(mech, command->flag))
                cut_apart_helpmsgs(&c, command->name,
                                   command_help_message(id, j), 37, 1);
          }
        }
    }
  }
  cool_menu_add_with_flags(&c, NULL, CM_ONE | CM_LINE);
  ShowCoolMenu(btech_context_evaluation(context), player, c);
  KillCoolMenu(c);
}
