/* Implements BattleTech autopilot mechanics for autopilot core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_argument_list_api.h"
#include "autopilot_commands_api.h"
#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "coolmenu.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mycool.h"
#include "registry_api.h"
#include "special_object.h"

/*
 * Creates a new command_node for the AI's
 * command list
 */
static AutopilotCommand *auto_create_command_node() {

  AutopilotCommand *temp;

  temp = malloc(sizeof(AutopilotCommand));
  if (temp == NULL)
    return NULL;

  memset(temp, 0, sizeof(AutopilotCommand));
  autopilot_argument_list_initialize(&temp->arguments, AUTOPILOT_MAX_ARGS);
  temp->ai_command_function = NULL;

  return temp;
}

/*
 * Destroys a command_node
 */
void auto_destroy_command_node(AutopilotCommand *node) {

  autopilot_argument_list_destroy(&node->arguments);

  /* Free the node */
  free(node);

  return;
}

/*
   The Autopilot command interface

   addcommand <name> [args]
   delcommand <num>
   listcommands
   engage
   disengage
   jump

 */

/*
 * The commands that are on the XCODE Object along
 * with some helper commands for modifying the state
 * of the AI
 */

/*! \todo {See if we need this function and remove it if not} */
int auto_valid_progline(Autopilot *a, int p) { return 0; }

/*
 * Internal function to return a string that
 * displays a command from a command_node
 */
/*! \todo {Maybe re-write this so doesn't use a static buffer} */
typedef struct AutoCommandText {
  char text[MBUF_SIZE];
} AutoCommandText;

static AutoCommandText auto_command_text(AutopilotCommand *node) {
  AutoCommandText command = {0};
  char *buf = command.text;
  int i;
  size_t len;

  snprintf(buf, sizeof(command.text), "%-10s",
           autopilot_argument_list_get(&node->arguments, 0));

  /* Loop through the args and print the commands */
  for (i = 1; i < AUTOPILOT_MAX_ARGS; i++) {
    const char *argument =
        autopilot_argument_list_get(&node->arguments, (size_t)i);
    if (argument != nullptr) {
      len = strlen(buf);
      if (len < sizeof(command.text) - 1)
        strncat(buf, " ", sizeof(command.text) - len - 1);
      len = strlen(buf);
      if (len < sizeof(command.text) - 1)
        strncat(buf, argument, sizeof(command.text) - len - 1);
    }
  }

  return command;
}

/*
 * Removes a command from the AI's command list
 */
void auto_delcommand(DbRef player, void *data, const char *buffer) {

  int p;
  Autopilot *autopilot = (Autopilot *)data;
  int remove_all_commands = 0;
  AutopilotCommand *temp_command_node;
  char error_buf[MBUF_SIZE];

  /* Make sure they specified an argument */
  if (!*buffer) {
    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "No argument used : Usage delcommand [num]\n");
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "Must be within the range"
                  " 1 to %d or -1 for all\n",
                  doubly_linked_list_size(autopilot->commands));
    return;
  }

  /* Make sure its a number */
  if ((!((p) = atoi(buffer)) && strcmp((buffer), "0"))) {
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "Invalid Argument : Must be within the range"
                  " 1 to %d or -1 for all\n",
                  doubly_linked_list_size(autopilot->commands));
    return;
  }

  /* Check if its a valid command position
   * If its -1 means remove all */
  if (p == -1) {
    remove_all_commands = 1;
  } else if ((p > doubly_linked_list_size(autopilot->commands)) || (p < 1)) {
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "Invalid Argument : Must be within the range"
                  " 1 to %d or -1 for all\n",
                  doubly_linked_list_size(autopilot->commands));
    return;
  }

  /*! \todo {Add in check so they don't accidently remove a running command
   * without disengaging first} */

  /* Now remove the node(s) */
  if (!remove_all_commands) {

    /* Remove the node at pos */
    temp_command_node =
        (AutopilotCommand *)doubly_linked_list_remove_node_at_pos(
            autopilot->commands, p);

    if (!temp_command_node) {
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error: Trying to remove"
               " Command #%d from AI #%ld but the command node doesn't exist\n",
               p, autopilot->mynum);
      btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                         error_buf);
    }

    /* Destroy the command_node */
    auto_destroy_command_node(temp_command_node);

    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "Command #%d Successfully Removed\n", p);

  } else {

    /* Remove ALL the commands */
    while (doubly_linked_list_size(autopilot->commands)) {

      /* Remove the first node on the list and get the data
       * from it */
      temp_command_node = (AutopilotCommand *)doubly_linked_list_remove(
          autopilot->commands, doubly_linked_list_head(autopilot->commands));

      /* Make sure the command node exists */
      if (!temp_command_node) {

        snprintf(error_buf, MBUF_SIZE,
                 "Internal AI Error: Trying to remove"
                 " the first command from AI #%ld but the command node doesn't "
                 "exist\n",
                 autopilot->mynum);
        btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI,
                           "%s", error_buf);

      } else {

        /* Destroy the command node */
        auto_destroy_command_node(temp_command_node);
      }
    }

    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "All the commands have been removed.\n");
  }
}

/*
 * Jump to a specific command location in the AI's
 * command list
 */
void auto_jump(DbRef player, void *data, char *buffer) {
  Autopilot *autopilot = data;
  mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
               "jump has been temporarly disabled till I can figure out"
               " how I want to change it - Dany");
}

/*
 * Adds a command to the AI Command List
 */
void auto_addcommand(DbRef player, void *data, char *buffer) {

  Autopilot *autopilot = (Autopilot *)data;
  AutopilotArgumentList args;
  char *command; /* temp string to get the name of the command */
  int argc;
  int i;

  AutopilotCommand *temp_command_node;
  DoublyLinkedListNode *temp_dllist_node;

  autopilot_argument_list_initialize(&args, AUTOPILOT_MAX_ARGS);

  command = first_parseattribute(buffer);

  /* Look at the buffer and try and get the command */
  const AutopilotCommandDefinition *definition =
      autopilot_command_definition_at(0);
  for (i = 0; definition->name; i++) {
    if ((!strncmp(command, definition->name, strlen(command))) &&
        (!strncmp(definition->name, command, strlen(definition->name))))
      break;
    definition = autopilot_command_definition_at(i + 1);
  }

  /* Free the command string we dont need it anymore */
  free(command);

  /* Make sure its a valid command */
  if (!definition->name) {
    autopilot_argument_list_destroy(&args);
    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "Invalid Command!");
    return;
  }

  /* Get the arguments for the command */
  if (definition->argcount > 0) {

    /* Parse the buffer for commands
     * Its argcount + 1 because we are parsing the command + its
     * arguments */
    argc = proper_explodearguments(
        buffer, autopilot_argument_list_parser_storage(&args),
        definition->argcount + 1);

    if (argc != definition->argcount + 1) {

      autopilot_argument_list_destroy(&args);
      mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                   "Not the proper number of arguments!");
      return;
    }

  } else {

    /* Copy the command to the first arg */
    autopilot_argument_list_set(&args, 0, strdup(definition->name));
  }

  /* Build the command node */
  temp_command_node = auto_create_command_node();

  for (size_t index = 0; index < AUTOPILOT_MAX_ARGS; index++) {
    char *argument = autopilot_argument_list_take(&args, index);
    if (argument != nullptr)
      autopilot_argument_list_set(&temp_command_node->arguments, index,
                                  argument);
  }
  autopilot_argument_list_destroy(&args);

  temp_command_node->argcount = (unsigned char)definition->argcount;
  temp_command_node->command_enum = definition->command_enum;
  temp_command_node->ai_command_function = definition->ai_command_function;

  /* Add the command to the list */
  temp_dllist_node = doubly_linked_list_create_node(temp_command_node);
  doubly_linked_list_insert_end(autopilot->commands, temp_dllist_node);

  /* Let the player know it worked */
  notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                "Command Added: %s", auto_command_text(temp_command_node).text);
}

/*
 * Lists the various settings and commands currently on the AI
 */
void auto_listcommands(DbRef player, void *data, char *buffer) {

  Autopilot *autopilot = (Autopilot *)data;
  CoolMenu *c = NULL;
  char buf[MBUF_SIZE];
  int i;

  cool_menu_add_line(&c);

  snprintf(
      buf, MBUF_SIZE, "Autopilot data for %s",
      game_object_name(autopilot->xcode.context->database, autopilot->mynum));
  cool_menu_add_text(&c, buf);

  snprintf(
      buf, MBUF_SIZE, "Controling unit %s",
      game_object_name(autopilot->xcode.context->database,
                       game_object_location(autopilot->xcode.context->database,
                                            autopilot->mynum)));
  cool_menu_add_text(&c, buf);

  cool_menu_add_line(&c);

  snprintf(buf, MBUF_SIZE,
           "MyRef: #%ld  MechRef: #%ld  MapIndex: #%ld  "
           "FSpeed: %d %% (Flag:%d)",
           autopilot->mynum, autopilot->mymechnum, autopilot->mapindex,
           autopilot->speed, autopilot->flags);
  cool_menu_add_text(&c, buf);

  cool_menu_add_line(&c);

  if (doubly_linked_list_size(autopilot->commands)) {

    for (i = 1; i <= doubly_linked_list_size(autopilot->commands); i++) {
      snprintf(
          buf, MBUF_SIZE, "#%-3d %s", i,
          auto_command_text((AutopilotCommand *)doubly_linked_list_get_node(
                                autopilot->commands, i))
              .text);
      cool_menu_add_text(&c, buf);
    }

  } else {
    cool_menu_add_text(&c, "No commands have been queued to date.");
  }

  cool_menu_add_line(&c);
  ShowCoolMenu(btech_context_evaluation(autopilot->xcode.context), player, c);
  KillCoolMenu(c);
}

void auto_eventstats(DbRef player, void *data, char *buffer) {

  Autopilot *autopilot = (Autopilot *)data;
  int i, j, total;

  mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
               "Events by type: ");
  mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
               "-------------------------------");

  total = 0;

  for (i = FIRST_AUTO_EVENT; i <= LAST_AUTO_EVENT; i++) {

    if ((j = mux_event_count_type_data(autopilot->xcode.context->events, i,
                                       (void *)autopilot))) {
      notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                    "%-20s%d", btech_event_name(i), j);
      total += j;
    }
  }

  if (total) {
    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "-------------------------------");
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "%d total", total);
  }
}

/*
 * Turn the autopilot on
 */
static int auto_pilot_on(Autopilot *autopilot) {

  int i, j, count = 0;

  for (i = FIRST_AUTO_EVENT; i <= LAST_AUTO_EVENT; i++)
    if ((j = mux_event_count_type_data(autopilot->xcode.context->events, i,
                                       (void *)autopilot)))
      count += j;

  if (!count) {
    return autopilot->flags &
           (AUTOPILOT_AUTOGUN | AUTOPILOT_GUNZOMBIE | AUTOPILOT_PILZOMBIE);
  }

  return count;
}

/*
 * Stop whatever the autopilot is doing
 */
void auto_stop_pilot(Autopilot *autopilot) {

  int i;

  autopilot->flags &=
      ~(AUTOPILOT_AUTOGUN | AUTOPILOT_GUNZOMBIE | AUTOPILOT_PILZOMBIE);

  for (i = FIRST_AUTO_EVENT; i <= LAST_AUTO_EVENT; i++)
    mux_event_remove_type_data(autopilot->xcode.context->events, i,
                               (void *)autopilot);
}

/*
 * Set the comtitle for the autopilot's unit
 */
void auto_set_comtitle(Autopilot *autopilot, Mech *mech) {

  char buf[LBUF_SIZE];

  snprintf(buf, LBUF_SIZE, "a=%s/%s", mech_model_reference(mech),
           mech_id(mech, true).text);
  mech_set_channeltitle(autopilot->mynum, mech, buf);
}

/*
 * Set default parameters for the AI
 */
/*! \todo {Make this smarter and check some of these} */
void auto_init(Autopilot *autopilot, Mech *mech) {

  autopilot->ofsx = 0;       /* Positional - angle */
  autopilot->ofsy = 0;       /* Positional - distance */
  autopilot->auto_cmode = 1; /* CHARGE! */
  autopilot->auto_cdist = 2; /* Attempt to avoid kicking distance */
  autopilot->auto_nervous = 0;
  autopilot->auto_goweight = 44; /* We're mainly concentrating on fighting */
  autopilot->auto_fweight = 55;
  autopilot->speed = 100; /* Reset to full speed */
  autopilot->flags = 0;

  /* Target Stuff */
  autopilot->target = -2;
  autopilot->target_score = 0;
  autopilot->target_threshold = 50;
  autopilot->target_update_tick = AUTO_GUN_UPDATE_TICK;

  /* Follow & Chase target stuff */
  autopilot->chase_target = -10;
  autopilot->chasetarg_update_tick = AUTOPILOT_CHASETARG_UPDATE_TICK;
  autopilot->follow_update_tick = AUTOPILOT_FOLLOW_UPDATE_TICK;
}

/*
 * Setup all the flags and variables to current, then
 * start the AI's first command.
 */
void auto_engage(DbRef player, void *data, const char *buffer) {

  Autopilot *autopilot = (Autopilot *)data;
  Mech *mech;

  autopilot->mymech = mech = btech_context_get_mech(
      autopilot->xcode.context,
      (autopilot->mymechnum = game_object_location(
           autopilot->xcode.context->database, autopilot->mynum)));
  if (!autopilot) {
    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "Internal error! - Bad AI object!");
    return;
  }
  if (!mech) {
    mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
                 "Error: The autopilot isn't inside a 'mech!");
    return;
  }
  if (auto_pilot_on(autopilot)) {
    mecha_notify(
        btech_context_evaluation(autopilot->xcode.context), player,
        "The autopilot's already online! You have to disengage it first.");
    return;
  }

  if (mech_autopilot_dbref(mech) <= 0)
    auto_init(autopilot, mech);
  mech_autopilot_dbref_set(mech, autopilot->mynum);

  if (mech_autopilot_dbref(mech) > 0)
    auto_set_comtitle(autopilot, mech);

  autopilot->mapindex = mech_map_dbref(mech);

  mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
               "Engaging autopilot...");
  autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event,
                           AUTOPILOT_NC_DELAY, 0);

  return;
}

/*
 * Turn off the autopilot
 */
void auto_disengage(DbRef player, void *data, const char *buffer) {

  Autopilot *autopilot = (Autopilot *)data;

  if (!auto_pilot_on(autopilot)) {
    mecha_notify(
        btech_context_evaluation(autopilot->xcode.context), player,
        "The autopilot's already offline! You have to engage it first.");
    return;
  }

  auto_stop_pilot(autopilot);
  mecha_notify(btech_context_evaluation(autopilot->xcode.context), player,
               "Autopilot has been disengaged.");

  return;
}

/*
 * Remove the first command_node in the list and go to the next
 */
void auto_goto_next_command(Autopilot *autopilot, int time) {

  AutopilotCommand *temp_command_node;
  char error_buf[MBUF_SIZE];

  if (doubly_linked_list_size(autopilot->commands) < 0) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to remove"
             " the first command from AI #%ld but the command list is empty\n",
             autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  temp_command_node = (AutopilotCommand *)doubly_linked_list_remove(
      autopilot->commands, doubly_linked_list_head(autopilot->commands));

  if (!temp_command_node) {
    snprintf(
        error_buf, MBUF_SIZE,
        "Internal AI Error: Trying to remove"
        " the first command from AI #%ld but the command node doesn't exist\n",
        autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return;
  }

  auto_destroy_command_node(temp_command_node);

  /* Fire off the AUTO_COM event */
  autopilot_event_schedule(autopilot, EVENT_AUTOCOM, auto_com_event, time, 0);
}

/*
 * Get the argument for a given command position and argument number
 * Remember to free the string that this returns after use
 */
char *auto_get_command_arg(Autopilot *autopilot, int command_number,
                           int arg_number) {

  char *argument;
  AutopilotCommand *temp_command_node;
  char error_buf[MBUF_SIZE];

  if (command_number > doubly_linked_list_size(autopilot->commands)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Command #%d for AI #%ld but it doesn't exist",
             command_number, autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return NULL;
  }

  if (arg_number >= AUTOPILOT_MAX_ARGS) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Arg #%d for AI #%ld Command #%d but its greater"
             " then AUTOPILOT_MAX_ARGS (%d)",
             arg_number, autopilot->mynum, command_number, AUTOPILOT_MAX_ARGS);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return NULL;
  }

  temp_command_node = (AutopilotCommand *)doubly_linked_list_get_node(
      autopilot->commands, command_number);

  /*! \todo {Add in check incase the command node doesn't exist} */

  const char *stored_argument = autopilot_argument_list_get(
      &temp_command_node->arguments, (size_t)arg_number);
  if (!stored_argument) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Arg #%d for AI #%ld Command #%d but it doesn't exist",
             arg_number, autopilot->mynum, command_number);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return NULL;
  }

  argument = strndup(stored_argument, MBUF_SIZE);

  return argument;
}

/*
 * Returns the command_enum value for the given command
 * from the AI command list
 */
int auto_get_command_enum(Autopilot *autopilot, int command_number) {

  int command_enum;
  AutopilotCommand *temp_command_node;
  char error_buf[MBUF_SIZE];

  /* Make sure there are commands */
  if (doubly_linked_list_size(autopilot->commands) <= 0) {
    return -1;
  }

  if (command_number <= 0) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access a command (%d) for AI #%ld that can't be on a list",
             command_number, autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return -1;
  }

  /* Make sure the command is on the list */
  if (command_number > doubly_linked_list_size(autopilot->commands)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Command #%d for AI #%ld but it doesn't exist",
             command_number, autopilot->mynum);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return -1;
  }

  temp_command_node = (AutopilotCommand *)doubly_linked_list_get_node(
      autopilot->commands, command_number);

  /*! \todo {Add in check incase the command node doesn't exist} */

  command_enum = temp_command_node->command_enum;

  /* If its a bad enum value we have a problem */
  if ((command_enum >= AUTO_NUM_COMMANDS) || (command_enum < 0)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Command ENUM for"
             " AI #%ld Command Number #%d doesn't exist\n",
             autopilot->mynum, command_number);
    btech_channel_send(autopilot->xcode.context, BTECH_CHANNEL_MECH_AI, "%s",
                       error_buf);
    return -1;
  }

  return command_enum;
}

/*
 * Called when either creating a new autopilot - SPECIAL_ALLOC
 * or when destroying an autopilot - SPECIAL_FREE
 */
void auto_newautopilot(DbRef key, void **data,
                       BtechSpecialLifecycleOperation selector) {

  Autopilot *autopilot = *data;
  Mech *mech;
  AutopilotCommand *temp;
  switch (selector) {
  case SPECIAL_ALLOC:
    autopilot->mynum = key;

    /* Allocate the command list */
    autopilot->commands = doubly_linked_list_create_list();

    /* Make sure certain things are set NULL */
    autopilot->astar_path = NULL;
    autopilot->weaplist = NULL;

    autopilot_weapon_profiles_initialize(autopilot);

    /* And some things not set null */
    autopilot->speed = 100;

    break;

  case SPECIAL_FREE:

    /* Make sure the AI is stopped */
    auto_stop_pilot(autopilot);

    /* Go through the list and remove any leftover nodes */
    while (doubly_linked_list_size(autopilot->commands)) {

      /* Remove the first node on the list and get the data
       * from it */
      temp = (AutopilotCommand *)doubly_linked_list_remove(
          autopilot->commands, doubly_linked_list_head(autopilot->commands));

      /* Destroy the command node */
      auto_destroy_command_node(temp);
    }

    /* Destroy the list */
    doubly_linked_list_destroy_list(autopilot->commands);
    autopilot->commands = NULL;

    /* Destroy any astar path list thats on the AI */
    auto_destroy_astar_path(autopilot);

    /* Destroy profile array */
    autopilot_weapon_profiles_clear(autopilot);

    /* Destroy weaponlist */
    auto_destroy_weaplist(autopilot);

    /* Finally reset the AI value on its unit if
     * it needs to */
    if ((mech = btech_context_get_mech(autopilot->xcode.context,
                                       autopilot->mymechnum))) {

      /* Just incase another AI has taken over */
      if (mech_autopilot_dbref(mech) == autopilot->mynum) {
        mech_autopilot_dbref_set(mech, -1);
      }
    }

    break;
  }
}

// XXX: put in a header file
void auto_heartbeat(Autopilot *autopilot) {
  if (!autopilot->mymech)
    return;
  auto_sensor_event(autopilot);
  if (autopilot->weaplist == NULL ||
      autopilot->xcode.context->tick % AUTO_PROFILE_TICK == 0)
    auto_update_profile_event(autopilot);
  auto_gun_event(autopilot);
}
