/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include "autopilot.h"
#include "coolmenu.h"
#include "glue.h"
#include "mech.events.h"
#include "mech.h"
#include "mycool.h"
#include "p.mech.utils.h"

extern const ACOM acom[AUTO_NUM_COMMANDS + 1];
extern char *mux_event_names[];

/*
 * Creates a new command_node for the AI's
 * command list
 */
static command_node *auto_create_command_node() {

  command_node *temp;

  temp = malloc(sizeof(command_node));
  if (temp == NULL)
    return NULL;

  memset(temp, 0, sizeof(command_node));
  temp->ai_command_function = NULL;

  return temp;
}

/*
 * Destroys a command_node
 */
void auto_destroy_command_node(command_node *node) {

  int i;

  /* Free the args */
  for (i = 0; i < AUTOPILOT_MAX_ARGS; i++) {
    if (node->args[i]) {
      free(node->args[i]);
      node->args[i] = NULL;
    }
  }

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
int auto_valid_progline(AUTO *a, int p) {
#if 0
	for(i = 0; i < a->first_free; i += (acom[a->commands[i]].argcount + 1))
		if(i == p)
			return 1;
#endif
  return 0;
}

/*
 * Internal function to return a string that
 * displays a command from a command_node
 */
/*! \todo {Maybe re-write this so doesn't use a static buffer} */
typedef struct AutoCommandText {
  char text[MBUF_SIZE];
} AutoCommandText;

static AutoCommandText auto_command_text(command_node *node) {
  AutoCommandText command = {0};
  char *buf = command.text;
  int i;
  size_t len;

  snprintf(buf, sizeof(command.text), "%-10s", node->args[0]);

  /* Loop through the args and print the commands */
  for (i = 1; i < AUTOPILOT_MAX_ARGS; i++)
    if (node->args[i]) {
      len = strlen(buf);
      if (len < sizeof(command.text) - 1)
        strncat(buf, " ", sizeof(command.text) - len - 1);
      len = strlen(buf);
      if (len < sizeof(command.text) - 1)
        strncat(buf, node->args[i], sizeof(command.text) - len - 1);
    }

  return command;
}

/*
 * Removes a command from the AI's command list
 */
void auto_delcommand(DbRef player, void *data, char *buffer) {

  int p;
  AUTO *autopilot = (AUTO *)data;
  int remove_all_commands = 0;
  command_node *temp_command_node;
  char error_buf[MBUF_SIZE];

  /* Make sure they specified an argument */
  if (!*buffer) {
    notify(btech_context_evaluation(autopilot->xcode.context), player,
           "No argument used : Usage delcommand [num]\n");
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "Must be within the range"
                  " 1 to %d or -1 for all\n",
                  doubly_linked_list_size(autopilot->commands));
    return;
  }

  /* Make sure its a number */
  if (Readnum(p, buffer)) {
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
    temp_command_node = (command_node *)doubly_linked_list_remove_node_at_pos(
        autopilot->commands, p);

    if (!temp_command_node) {
      snprintf(error_buf, MBUF_SIZE,
               "Internal AI Error: Trying to remove"
               " Command #%d from AI #%ld but the command node doesn't exist\n",
               p, autopilot->mynum);
      SendAI(autopilot->xcode.context, error_buf);
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
      temp_command_node = (command_node *)doubly_linked_list_remove(
          autopilot->commands, doubly_linked_list_head(autopilot->commands));

      /* Make sure the command node exists */
      if (!temp_command_node) {

        snprintf(error_buf, MBUF_SIZE,
                 "Internal AI Error: Trying to remove"
                 " the first command from AI #%ld but the command node doesn't "
                 "exist\n",
                 autopilot->mynum);
        SendAI(autopilot->xcode.context, error_buf);

      } else {

        /* Destroy the command node */
        auto_destroy_command_node(temp_command_node);
      }
    }

    notify(btech_context_evaluation(autopilot->xcode.context), player,
           "All the commands have been removed.\n");
  }
}

/*
 * Jump to a specific command location in the AI's
 * command list
 */
void auto_jump(DbRef player, void *data, char *buffer) {
  AUTO *autopilot = data;
  notify(btech_context_evaluation(autopilot->xcode.context), player,
         "jump has been temporarly disabled till I can figure out"
         " how I want to change it - Dany");
#if 0
	skipws(buffer);
	DOCHECK_CONTEXT(autopilot->xcode.context, !*buffer, "Argument expected!");
	DOCHECK_CONTEXT(autopilot->xcode.context, Readnum(p, buffer), "Invalid argument - single number expected.");
	/* Find out if it's valid position */
	DOCHECK_CONTEXT(autopilot->xcode.context, !auto_valid_progline(a, p),
			"Invalid : Argument out of range, or argument, not command.");
	PG(a) = p;
	notify_printf(btech_context_evaluation(autopilot->xcode.context), player, "Program Counter set to #%d.", p);
#endif
}

/*
 * Adds a command to the AI Command List
 */
void auto_addcommand(DbRef player, void *data, char *buffer) {

  AUTO *autopilot = (AUTO *)data;
  char *args[AUTOPILOT_MAX_ARGS]; /* args[0] is the command the rest are
                                                                     args for
                                     the command */
  char *command; /* temp string to get the name of the command */
  int argc;
  int i, j;

  command_node *temp_command_node;
  DoublyLinkedListNode *temp_dllist_node;

  /* Clear the Args */
  memset(args, 0, sizeof(char *) * AUTOPILOT_MAX_ARGS);

  command = first_parseattribute(buffer);

  /* Look at the buffer and try and get the command */
  for (i = 0; acom[i].name; i++) {
    if ((!strncmp(command, acom[i].name, strlen(command))) &&
        (!strncmp(acom[i].name, command, strlen(acom[i].name))))
      break;
  }

  /* Free the command string we dont need it anymore */
  free(command);

  /* Make sure its a valid command */
  DOCHECK_CONTEXT(autopilot->xcode.context, !acom[i].name, "Invalid Command!");

  /* Get the arguments for the command */
  if (acom[i].argcount > 0) {

    /* Parse the buffer for commands
     * Its argcount + 1 because we are parsing the command + its
     * arguments */
    argc = proper_explodearguments(buffer, args, acom[i].argcount + 1);

    if (argc != acom[i].argcount + 1) {

      /* Free the args before we quit */
      for (j = 0; j < AUTOPILOT_MAX_ARGS; j++) {
        if (args[j])
          free(args[j]);
      }
      notify(btech_context_evaluation(autopilot->xcode.context), player,
             "Not the proper number of arguments!");
      return;
    }

  } else {

    /* Copy the command to the first arg */
    args[0] = strdup(acom[i].name);
  }

  /* Build the command node */
  temp_command_node = auto_create_command_node();

  for (j = 0; j < AUTOPILOT_MAX_ARGS; j++) {
    if (args[j])
      temp_command_node->args[j] = args[j];
  }

  temp_command_node->argcount = acom[i].argcount;
  temp_command_node->command_enum = acom[i].command_enum;
  temp_command_node->ai_command_function = acom[i].ai_command_function;

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

  AUTO *autopilot = (AUTO *)data;
  coolmenu *c = NULL;
  char buf[MBUF_SIZE];
  int i;

  addline();

  snprintf(
      buf, MBUF_SIZE, "Autopilot data for %s",
      game_object_name(autopilot->xcode.context->database, autopilot->mynum));
  vsi(buf);

  snprintf(
      buf, MBUF_SIZE, "Controling unit %s",
      game_object_name(autopilot->xcode.context->database,
                       game_object_location(autopilot->xcode.context->database,
                                            autopilot->mynum)));
  vsi(buf);

  addline();

  snprintf(buf, MBUF_SIZE,
           "MyRef: #%ld  MechRef: #%ld  MapIndex: #%d  "
           "FSpeed: %d %% (Flag:%d)",
           autopilot->mynum, autopilot->mymechnum, autopilot->mapindex,
           autopilot->speed, autopilot->flags);
  vsi(buf);

  addline();

  if (doubly_linked_list_size(autopilot->commands)) {

    for (i = 1; i <= doubly_linked_list_size(autopilot->commands); i++) {
      snprintf(buf, MBUF_SIZE, "#%-3d %s", i,
               auto_command_text((command_node *)doubly_linked_list_get_node(
                                     autopilot->commands, i))
                   .text);
      vsi(buf);
    }

  } else {
    vsi("No commands have been queued to date.");
  }

  addline();
  ShowCoolMenu(btech_context_evaluation(autopilot->xcode.context), player, c);
  KillCoolMenu(c);
}

void auto_eventstats(DbRef player, void *data, char *buffer) {

  AUTO *autopilot = (AUTO *)data;
  int i, j, total;

  notify(btech_context_evaluation(autopilot->xcode.context), player,
         "Events by type: ");
  notify(btech_context_evaluation(autopilot->xcode.context), player,
         "-------------------------------");

  total = 0;

  for (i = FIRST_AUTO_EVENT; i <= LAST_AUTO_EVENT; i++) {

    if ((j = mux_event_count_type_data(autopilot->xcode.context->events, i,
                                       (void *)autopilot))) {
      notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                    "%-20s%d", mux_event_names[i], j);
      total += j;
    }
  }

  if (total) {
    notify(btech_context_evaluation(autopilot->xcode.context), player,
           "-------------------------------");
    notify_printf(btech_context_evaluation(autopilot->xcode.context), player,
                  "%d total", total);
  }
}

/*
 * Turn the autopilot on
 */
static int auto_pilot_on(AUTO *autopilot) {

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
extern void auto_stop_pilot(AUTO *autopilot) {

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
void auto_set_comtitle(AUTO *autopilot, MECH *mech) {

  char buf[LBUF_SIZE];

  snprintf(buf, LBUF_SIZE, "a=%s/%s", MechType_Ref(mech),
           mech_id(mech, true).text);
  mech_set_channeltitle(autopilot->mynum, mech, buf);
}

/*
 * Set default parameters for the AI
 */
/*! \todo {Make this smarter and check some of these} */
void auto_init(AUTO *autopilot, MECH *mech) {

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
void auto_engage(DbRef player, void *data, char *buffer) {

  AUTO *autopilot = (AUTO *)data;
  MECH *mech;

  autopilot->mymech = mech = btech_context_get_mech(
      autopilot->xcode.context,
      (autopilot->mymechnum = game_object_location(
           autopilot->xcode.context->database, autopilot->mynum)));
  DOCHECK_CONTEXT(autopilot->xcode.context, !autopilot,
                  "Internal error! - Bad AI object!");
  DOCHECK_CONTEXT(autopilot->xcode.context, !mech,
                  "Error: The autopilot isn't inside a 'mech!");
  DOCHECK_CONTEXT(
      autopilot->xcode.context, auto_pilot_on(autopilot),
      "The autopilot's already online! You have to disengage it first.");

  if (MechAuto(mech) <= 0)
    auto_init(autopilot, mech);
  MechAuto(mech) = autopilot->mynum;

  if (MechAuto(mech) > 0)
    auto_set_comtitle(autopilot, mech);

  autopilot->mapindex = mech->mapindex;

  notify(btech_context_evaluation(autopilot->xcode.context), player,
         "Engaging autopilot...");
  AUTOEVENT(autopilot, EVENT_AUTOCOM, auto_com_event, AUTOPILOT_NC_DELAY, 0);

  return;
}

/*
 * Turn off the autopilot
 */
void auto_disengage(DbRef player, void *data, char *buffer) {

  AUTO *autopilot = (AUTO *)data;

  DOCHECK_CONTEXT(
      autopilot->xcode.context, !auto_pilot_on(autopilot),
      "The autopilot's already offline! You have to engage it first.");

  auto_stop_pilot(autopilot);
  notify(btech_context_evaluation(autopilot->xcode.context), player,
         "Autopilot has been disengaged.");

  return;
}

/*
 * Remove the first command_node in the list and go to the next
 */
void auto_goto_next_command(AUTO *autopilot, int time) {

  command_node *temp_command_node;
  char error_buf[MBUF_SIZE];

  if (doubly_linked_list_size(autopilot->commands) < 0) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to remove"
             " the first command from AI #%ld but the command list is empty\n",
             autopilot->mynum);
    SendAI(autopilot->xcode.context, error_buf);
    return;
  }

  temp_command_node = (command_node *)doubly_linked_list_remove(
      autopilot->commands, doubly_linked_list_head(autopilot->commands));

  if (!temp_command_node) {
    snprintf(
        error_buf, MBUF_SIZE,
        "Internal AI Error: Trying to remove"
        " the first command from AI #%ld but the command node doesn't exist\n",
        autopilot->mynum);
    SendAI(autopilot->xcode.context, error_buf);
    return;
  }

  auto_destroy_command_node(temp_command_node);

  /* Fire off the AUTO_COM event */
  AUTO_COM(autopilot, time);
}

/*
 * Get the argument for a given command position and argument number
 * Remember to free the string that this returns after use
 */
char *auto_get_command_arg(AUTO *autopilot, int command_number,
                           int arg_number) {

  char *argument;
  command_node *temp_command_node;
  char error_buf[MBUF_SIZE];

  if (command_number > doubly_linked_list_size(autopilot->commands)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Command #%d for AI #%ld but it doesn't exist",
             command_number, autopilot->mynum);
    SendAI(autopilot->xcode.context, error_buf);
    return NULL;
  }

  if (arg_number >= AUTOPILOT_MAX_ARGS) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Arg #%d for AI #%ld Command #%d but its greater"
             " then AUTOPILOT_MAX_ARGS (%d)",
             arg_number, autopilot->mynum, command_number, AUTOPILOT_MAX_ARGS);
    SendAI(autopilot->xcode.context, error_buf);
    return NULL;
  }

  temp_command_node = (command_node *)doubly_linked_list_get_node(
      autopilot->commands, command_number);

  /*! \todo {Add in check incase the command node doesn't exist} */

  if (!temp_command_node->args[arg_number]) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Arg #%d for AI #%ld Command #%d but it doesn't exist",
             arg_number, autopilot->mynum, command_number);
    SendAI(autopilot->xcode.context, error_buf);
    return NULL;
  }

  argument = strndup(temp_command_node->args[arg_number], MBUF_SIZE);

  return argument;
}

/*
 * Returns the command_enum value for the given command
 * from the AI command list
 */
int auto_get_command_enum(AUTO *autopilot, int command_number) {

  int command_enum;
  command_node *temp_command_node;
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
    SendAI(autopilot->xcode.context, error_buf);
    return -1;
  }

  /* Make sure the command is on the list */
  if (command_number > doubly_linked_list_size(autopilot->commands)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Trying to "
             "access Command #%d for AI #%ld but it doesn't exist",
             command_number, autopilot->mynum);
    SendAI(autopilot->xcode.context, error_buf);
    return -1;
  }

  temp_command_node = (command_node *)doubly_linked_list_get_node(
      autopilot->commands, command_number);

  /*! \todo {Add in check incase the command node doesn't exist} */

  command_enum = temp_command_node->command_enum;

  /* If its a bad enum value we have a problem */
  if ((command_enum >= AUTO_NUM_COMMANDS) || (command_enum < 0)) {
    snprintf(error_buf, MBUF_SIZE,
             "Internal AI Error: Command ENUM for"
             " AI #%ld Command Number #%d doesn't exist\n",
             autopilot->mynum, command_number);
    SendAI(autopilot->xcode.context, error_buf);
    return -1;
  }

  return command_enum;
}

#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

/*
 * Called when either creating a new autopilot - SPECIAL_ALLOC
 * or when destroying an autopilot - SPECIAL_FREE
 */
void auto_newautopilot(DbRef key, void **data, int selector) {

  AUTO *autopilot = *data;
  MECH *mech;
  command_node *temp;
  int i;

  switch (selector) {
  case SPECIAL_ALLOC:
    autopilot->mynum = key;

    /* Allocate the command list */
    autopilot->commands = doubly_linked_list_create_list();

    /* Make sure certain things are set NULL */
    autopilot->astar_path = NULL;
    autopilot->weaplist = NULL;

    for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
      autopilot->profile[i] = NULL;
    }

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
      temp = (command_node *)doubly_linked_list_remove(
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
    for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
      if (autopilot->profile[i]) {
        red_black_tree_destroy(autopilot->profile[i]);
      }
      autopilot->profile[i] = NULL;
    }

    /* Destroy weaponlist */
    auto_destroy_weaplist(autopilot);

    /* Finally reset the AI value on its unit if
     * it needs to */
    if ((mech = btech_context_get_mech(autopilot->xcode.context,
                                       autopilot->mymechnum))) {

      /* Just incase another AI has taken over */
      if (MechAuto(mech) == autopilot->mynum) {
        MechAuto(mech) = -1;
      }
    }

    break;
  }
}

// XXX: put in a header file
void auto_heartbeat(AUTO *autopilot) {
  if (!autopilot->mymech)
    return;
  auto_sensor_event(autopilot);
  if (autopilot->weaplist == NULL ||
      autopilot->xcode.context->tick % AUTO_PROFILE_TICK == 0)
    auto_update_profile_event(autopilot);
  auto_gun_event(autopilot);
}
