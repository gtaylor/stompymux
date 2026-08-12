#include "btech_event.h" // IWYU pragma: keep
#include "command_registry.h"
#include "map.h" // IWYU pragma: keep
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/* Implements help output for BattleTech special objects. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btechstats_api.h"
#include "mech_lifecycle.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "special_object.h"

/*** #include all the prototype here! ****/
#include "btech/persistence.h"
#include "coolmenu.h"
#include "mech_api_types.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mycool.h"
#include "registry_internal.h"

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
  (void)snprintf(checked_storage_region(destination, destination_size, padding,
                                        destination_size - padding),
                 destination_size - padding, "%s", source);
}

static void help_color_initialize(const char *from, char *to) {
  char buf[LBUF_SIZE];
  char *tp = to;

  const size_t FIRST_WORD_LENGTH = strcspn(from, " ");
  if (*checked_string_suffix(from, FIRST_WORD_LENGTH) != '\0') {

    strncpy(buf, from, FIRST_WORD_LENGTH);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                FIRST_WORD_LENGTH) = '\0';
    safe_str("[fg=blue bold]", to, &tp);
    safe_str(buf, to, &tp);
    safe_str("[reset] ", to, &tp);
    safe_str(checked_string_suffix(from, FIRST_WORD_LENGTH + 1), to, &tp);

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
#define M_LEN CM_ONE
#else
#define MLen CM_TWO
#endif

typedef struct HelpLineRequest {
  CoolMenu **menu;
  const char *message;
  int width;
  int indentation;
} HelpLineRequest;

static const char *help_line_add(const HelpLineRequest *request) {
  CoolMenu **d = request->menu;
  const char *msg = request->message;
  const int LEN = request->width;
  const int INITIAL = request->indentation;
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
  if (msg_len > (size_t)LEN) {
    break_offset = (size_t)LEN - 1;
    while (break_offset > 0 && *checked_string_suffix(msg, break_offset) != ' ')
      --break_offset;
    if (break_offset == 0)
      break_offset = (size_t)LEN;
  }

  if (INITIAL > 0) {
    /* Colorize header line.  */
    help_color_initialize(msg, buf);
  } else if (INITIAL < 0) {
    /* Write indented line.  */
    const size_t INDENTATION = (size_t)(-INITIAL);
    text_length = break_offset;
    memset(buf, ' ', INDENTATION);
    memcpy(checked_storage_region(buf, sizeof(buf), INDENTATION, text_length),
           msg, text_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                text_length + INDENTATION) = '\0';
  } else {
    /* Write unindented line.  */
    text_length = break_offset;
    memcpy(buf, msg, text_length);
    *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char), text_length) =
        '\0';
  }

  cool_menu_add_with_flags(&c, buf, M_LEN);

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

typedef struct HelpTextRequest {
  CoolMenu **menu;
  const char *command;
  const char *description;
  int width;
  int initial_indentation;
} HelpTextRequest;

static void help_text_add(const HelpTextRequest *request) {
  CoolMenu **d = request->menu;
  const char *msg1 = request->command;
  const char *msg2 = request->description;
  const int LEN = request->width;
  [[maybe_unused]] const int INITIAL = request->initial_indentation;
  int l1 = help_text_length(msg1);
  int l2 = help_text_length(msg2);
  int nl1, nl2;

#ifndef ONE_LINE_TEXTS

  msg1 = help_line_add(&(HelpLineRequest){
      .menu = d, .message = msg1, .width = len, .indentation = initial});
  msg2 =
      help_line_add(&(HelpLineRequest){.menu = d,
                                       .message = msg2,
                                       .width = initial ? len : len - TAB,
                                       .indentation = initial ? 0 : 0 - TAB});
  if (!msg1 && !msg2)
    return;
  nl1 = help_text_length(msg1);
  nl2 = help_text_length(msg2);
  if (nl1 != l1 || nl2 != l2) /* To prevent infinite loops */
    help_text_add(&(HelpTextRequest){.menu = d,
                                     .command = msg1,
                                     .description = msg2,
                                     .width = len,
                                     .initial_indentation = 0});
#else
  int first = 1;

  while (msg1 && *msg1) {
    msg1 = help_line_add(&(HelpLineRequest){.menu = d,
                                            .message = msg1,
                                            .width = LEN * 2 - 1,
                                            .indentation = first});
    nl1 = help_text_length(msg1);
    if (nl1 == l1)
      break;
    l1 = nl1;
    first = 0;
  }
  while (msg2 && *msg2) {
    msg2 = help_line_add(&(HelpLineRequest){.menu = d,
                                            .message = msg2,
                                            .width = LEN * 2 - TAB,
                                            .indentation = 0 - TAB});
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

void btech_special_object_help(const SpecialObjectHelpRequest *request) {
  BtechContext *context = request->context;
  const DbRef PLAYER = request->player;
  const char *type = request->type;
  const int ID = request->special_type;
  const DbRef LOC = request->location;
  const PowerId POWERNEEDED = request->power_needed;
  char *arg = request->argument;
  int i, j;
  Mech *mech = NULL;
  HelpSection sections[100];
  int count = 0, csho = 0;
  CoolMenu *c = NULL;
  char buf[LBUF_SIZE];
  int dc;

  if (ID == GTYPE_MECH)
    mech = btech_context_get_mech(context, LOC);
  memset(sections, 0, sizeof(sections));
  const int COMMAND_COUNT = (int)btech_special_command_count(ID);
  for (i = 0; i < COMMAND_COUNT; i++) {
    const BtechCommandDefinition *command =
        btech_special_command_definition(ID, (size_t)i);
    if (!btech_command_definition_has_handler(command) &&
        (*command->helpmsg != '@' ||
         btech_special_command_access(context, PLAYER, POWERNEEDED)))
      if (ID != GTYPE_MECH ||
          btech_command_allowed_for_mech(mech, command->flag)) {
        if (count)
          help_section(sections, count - 1)->length =
              i - help_section(sections, count - 1)->start;
        help_section(sections, count)->start = i;
        count++;
      }
  }
  if (count) {
    help_section(sections, count - 1)->length =
        i - help_section(sections, count - 1)->start;
  } else {
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
            command_help_message(ID, help_section(sections, i)->start), 70);
        cool_menu_add_with_flags(
            &c, tprintf("%s%s%s", "[fg=green]", buf, "[reset]"), CM_ONE);
      } else {
        cool_menu_add_with_flags(&c, tprintf("%s command listing: ", type),
                                 CM_ONE | CM_CENTER);
      }
      const HelpSection *section = help_section(sections, i);
      for (j = section->start + (count == 1 ? 0 : 1);
           j < section->start + section->length; j++) {
        const BtechCommandDefinition *command =
            btech_special_command_definition(ID, (size_t)j);
        if (*command->helpmsg != '@' ||
            btech_special_command_access(context, PLAYER, POWERNEEDED))
          if (ID != GTYPE_MECH ||
              btech_command_allowed_for_mech(mech, command->flag)) {
            strlcpy(buf, command->name, sizeof(buf));
            const size_t NAME_LENGTH = strcspn(buf, " ");
            *(char *)checked_storage_at(buf, sizeof(buf), sizeof(char),
                                        NAME_LENGTH) = '\0';
            cool_menu_add_with_flags(&c, buf, CM_FOUR);
            csho++;
          }
      }
    }
    if (!csho) {
      cool_menu_add_text(
          &c, tprintf("There are no commands you are authorized to use here."));
    } else {
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
      } else {
        dc = -1;
      }
    } else {
      if (count == 1) {
        cool_menu_add_text(
            &c,
            "This object doesn't have any other detailed help than 'HELP ALL'");
        dc = -2;
      } else {
        for (i = 0; i < count; i++)
          if (!strcasecmp(arg, command_help_message(
                                   ID, help_section(sections, i)->start)))
            break;
        if (i == count) {
          cool_menu_add_text(&c, "Subcategory not found.");
          dc = -2;
        } else {
          dc = i;
        }
      }
    }
    if (dc > -2) {
      for (i = 0; i < count; i++)
        if (dc == -1 || i == dc) {
          if (count > 1) {
            center_string(
                buf, sizeof(buf),
                command_help_message(ID, help_section(sections, i)->start), 70);
            cool_menu_add_text(&c,
                               tprintf("%s%s%s", "[fg=green]", buf, "[reset]"));
          }
          const HelpSection *section = help_section(sections, i);
          for (j = section->start + (count == 1 ? 0 : 1);
               j < section->start + section->length; j++) {
            const BtechCommandDefinition *command =
                btech_special_command_definition(ID, (size_t)j);
            if (*command->helpmsg != '@' ||
                btech_special_command_access(context, PLAYER, POWERNEEDED))
              if (ID != GTYPE_MECH ||
                  btech_command_allowed_for_mech(mech, command->flag))
                help_text_add(&(HelpTextRequest){
                    .menu = &c,
                    .command = command->name,
                    .description = command_help_message(ID, j),
                    .width = 37,
                    .initial_indentation = 1});
          }
        }
    }
  }
  cool_menu_add_with_flags(&c, NULL, CM_ONE | CM_LINE);
  show_cool_menu(btech_context_evaluation(context), PLAYER, c);
  kill_cool_menu(c);
}
