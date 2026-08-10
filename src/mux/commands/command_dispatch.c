/*
 * command.c - command parser and support routines
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/commands.h"
#include "mux/commands/command.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_internal.h"
#include "mux/commands/command_parser.h"
#include "mux/commands/macro.h"
#include "mux/communication/access_policy.h"
#include "mux/communication/comsys.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/configuration_context.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/movement_commands.h"
#include "mux/world/world_context.h"

int check_access(GameDatabase *database,
                 const ServerConfiguration *configuration, DbRef player,
                 int mask) {
  int succ, fail;

  if (mask & CA_DISABLED)
    return 0;
  if (is_god(database, player) || configuration->is_initializing)
    return 1;

  succ = fail = 0;
  if (mask & CA_GOD)
    fail++;
  if (mask & CA_WIZARD) {
    if (is_wizard(database, player))
      succ++;
    else
      fail++;
  }
  if ((succ == 0) && (mask & CA_ADMIN)) {
    if (is_wizard(database, player))
      succ++;
    else
      fail++;
  }
  if (succ > 0)
    fail = 0;
  if (fail > 0)
    return 0;

  /*
   * Check for forbidden flags.
   */

  if (!is_wizard(database, player) &&
      (((mask & CA_NO_SUSPECT) && is_suspect(database, player)) ||
       (!configuration->btech_ooc_comsys && (mask & CA_NO_IC) &&
        is_in_character_location(database, configuration, player)) ||
       ((mask & CA_NO_IC) && is_gagged(database, player))))
    return 0;
  return 1;
}

static inline bool is_protected(CMDENT *cmdp, int x) { return cmdp->perms & x; }

static char **command_argument_slot(char **arguments, size_t capacity,
                                    size_t index) {
  return (char **)checked_storage_at((void *)arguments, capacity,
                                     sizeof(*arguments), index);
}

static char *command_split_slash(char *text) {
  const size_t length = strlen(text);
  size_t offset = 0;

  while (offset < length && *(const char *)checked_storage_at_const(
                                text, length + 1, sizeof(char), offset) != '/')
    offset++;
  if (offset == length)
    return nullptr;
  *(char *)checked_storage_at(text, length + 1, sizeof(char), offset) = '\0';
  return checked_storage_at(text, length + 1, sizeof(char), offset + 1);
}

static void command_invoke(CMDENT *command, CommandContext *context,
                           DbRef player, DbRef cause, int key, char *unparsed,
                           char *first, char *second, char **vector,
                           int vector_count, char **command_arguments,
                           int command_argument_count) {
  CommandInvocation invocation = {
      .context = context,
      .player = player,
      .cause = cause,
      .key = key,
      .unparsed = unparsed,
      .first = first,
      .second = second,
      .vector = vector,
      .vector_count = vector_count,
      .command_arguments = command_arguments,
      .command_argument_count = command_argument_count,
  };

  command->handler.invoke(&invocation);
}

/*
 * ---------------------------------------------------------------------------
 * * process_cmdent: Perform indicated command with passed args.
 */
typedef struct CommandEntryDispatch {
  CommandContext *context;
  CMDENT *command;
  char *switches;
  DbRef player;
  DbRef cause;
  char *arguments;
  char *unparsed_command;
} CommandEntryDispatch;

static void process_cmdent(const CommandEntryDispatch *dispatch) {
  CommandContext *context = dispatch->context;
  CMDENT *cmdp = dispatch->command;
  char *switchp = dispatch->switches;
  DbRef player = dispatch->player;
  DbRef cause = dispatch->cause;
  char *arg = dispatch->arguments;
  char *unp_command = dispatch->unparsed_command;
  char *buf1 = nullptr, *buf2 = nullptr, tchar = '\x00';
  char *args[MAX_ARG];
  int nargs = 0, i = 0, fail = 0, parse_flags = 0, key = 0, xkey = 0;

  memset((void *)args, 0, sizeof(char *) * MAX_ARG);

  /*
   * Perform object type checks.
   */

  fail = 0;
  if (is_protected(cmdp, CA_LOCATION) &&
      !has_location(context->world->database, player))
    fail++;
  if (is_protected(cmdp, CA_CONTENTS) &&
      !has_contents(context->world->database, player))
    fail++;
  if (is_protected(cmdp, CA_PLAYER) &&
      (typeof_obj(context->world->database, player) != OBJECT_TYPE_PLAYER))
    fail++;
  if (fail > 0) {
    notify_checked(&context->evaluation, player, player,
                   "Command incompatible with invoker type.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  /*
   * Check global flags
   */

  if (is_protected(cmdp, CA_QUEUE) &&
      !context->world->configuration->is_command_queue_enabled) {
    notify_checked(&context->evaluation, player, player,
                   "Sorry, queueing and triggering are not allowed now.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  key = cmdp->extra & ~SW_MULTIPLE;
  if (key & SW_GOT_UNIQUE) {
    i = 1;
    key = key & ~SW_GOT_UNIQUE;
  } else {
    i = 0;
  }

  /*
   * Check if we have permission to execute the command
   */

  /* Asumption: base command permission required for all sub-commands */
  if (!check_access(context->world->database, context->world->configuration,
                    player, cmdp->perms)) {
    notify_checked(&context->evaluation, player, player, "Permission denied.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  /*
   * Check command switches.  Note that there may be more than one, * *
   *
   * *  * *  * *  * * and that we OR all of them together along with
   * the * extra * value * * * from * the command table to produce the
   * key * value in * the handler * *  * call.
   */

  if (switchp && cmdp->switches) {
    do {
      buf1 = command_split_slash(switchp);
      xkey = name_table_search(context->world->database,
                               context->world->configuration, player,
                               cmdp->switches, switchp);
      if (xkey == -1) {
        notify_printf(&context->evaluation, player,
                      "Unrecognized switch '%s' for command '%s'.", switchp,
                      cmdp->cmdname);
        return;
      } else if (xkey == -2) {
        notify_checked(&context->evaluation, player, player,
                       "Permission denied.", MSG_ME_ALL | MSG_F_DOWN);
        return;
      } else if (!(xkey & SW_MULTIPLE)) {
        if (i == 1) {
          notify_checked(&context->evaluation, player, player,
                         "Illegal combination of switches.",
                         MSG_ME_ALL | MSG_F_DOWN);
          return;
        }
        i = 1;
      } else {
        xkey &= ~SW_MULTIPLE;
      }
      key |= xkey;
      switchp = buf1;
    } while (buf1);
  } else if (switchp) {
    notify_printf(&context->evaluation, player,
                  "Command %s does not take switches.", cmdp->cmdname);
    return;
  }
  /*
   * We are allowed to run the command.  Now, call the handler using
   * the appropriate calling sequence and arguments.
   */

  parse_flags = COMMAND_PARSE_STRIP_LEADING | COMMAND_PARSE_STRIP_TRAILING;
  if (cmdp->callseq & CS_STRIP_AROUND)
    parse_flags |= COMMAND_PARSE_STRIP_AROUND;
  else if (!(cmdp->callseq & CS_UNPARSE))
    parse_flags |= COMMAND_PARSE_STRIP;

  switch (cmdp->callseq & CS_NARG_MASK) {
  case CS_NO_ARGS: /*
                    * <cmd>   (no args)
                    */
    command_invoke(cmdp, context, player, cause, key, unp_command, nullptr,
                   nullptr, nullptr, 0, nullptr, 0);
    break;
  case CS_ONE_ARG: /*
                    * <cmd> <arg>
                    */

    /*
     * If an unparsed command, just give it to the handler
     */

    if (cmdp->callseq & CS_UNPARSE) {
      command_invoke(cmdp, context, player, cause, key, unp_command, nullptr,
                     nullptr, nullptr, 0, nullptr, 0);
      break;
    }
    buf1 = parse_to(
        &(CommandParseRequest){.configuration = context->world->configuration,
                               .source = &arg,
                               .options = parse_flags});

    /*
     * Call the correct handler
     */

    command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                   nullptr, nullptr, 0, nullptr, 0);

    break;
  case CS_TWO_ARG: /*
                    * <cmd> <arg1> = <arg2>
                    */

    /*
     * Interpret ARG1
     */

    buf1 = parse_to(&(CommandParseRequest){
        .configuration = context->world->configuration,
        .source = &arg,
        .delimiter = '=',
        .options = COMMAND_PARSE_STRIP | COMMAND_PARSE_STRIP_TRAILING});

    /*
     * Handle when no '=' was specified
     */

    if (!arg || (arg && !*arg)) {
      arg = &tchar;
      *arg = '\0';
    }
    if (cmdp->callseq & CS_ARGV) {

      /*
       * Arg2 is ARGV style.  Go get the args
       */

      parse_arglist(&(CommandArgumentListRequest){
          .configuration = context->world->configuration,
          .source = arg,
          .options = parse_flags,
          .arguments = args,
          .maximum_arguments = MAX_ARG});
      for (nargs = 0;
           nargs < MAX_ARG &&
           *command_argument_slot(args, MAX_ARG, (size_t)nargs) != nullptr;
           nargs++)
        ;

      /*
       * Call the correct command handler
       */

      command_invoke(cmdp, context, player, cause, key, unp_command, buf1,
                     nullptr, args, nargs, nullptr, 0);

      /*
       * Free the argument buffers
       */

      for (i = 0; i < nargs; i++)
        if (*command_argument_slot(args, MAX_ARG, (size_t)i) != nullptr)
          free_lbuf(*command_argument_slot(args, MAX_ARG, (size_t)i));

    } else {

      if (cmdp->callseq & CS_UNPARSE) {
        buf2 = parse_to(&(CommandParseRequest){
            .configuration = context->world->configuration,
            .source = &arg,
            .options = COMMAND_PARSE_NO_COMPRESS});
      } else {
        buf2 = parse_to(&(CommandParseRequest){
            .configuration = context->world->configuration,
            .source = &arg,
            .options = parse_flags});
      }

      /*
       * Call the correct command handler
       */

      command_invoke(cmdp, context, player, cause, key, unp_command, buf1, buf2,
                     nullptr, 0, nullptr, 0);
    }
    break;
  default:
    break;
  }
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * process_command: Execute a command.
 */

void process_command(CommandContext *context, char *command, char *args[],
                     int nargs) {
  CommandRuntime *runtime = context->runtime;
  ServerConfiguration *configuration = runtime->world->configuration;
  CommandRegistry *registry = runtime->command_registry;
  const DbRef player = context->player;
  const DbRef cause = context->enactor;
  const bool interactive = context->interactive;
  char *arg = nullptr, *lcbuf = nullptr, *slashp = nullptr;
  const char *cmdsave = nullptr;
  int succ = 0, lua_succ = 0, i = 0;
  DbRef exit = 0;
  CMDENT *cmdp = nullptr;
  char *macroout = nullptr;
  int macerr = 0;

  /*
   * Robustify player
   */

  cmdsave = context->debug_command;
  context->debug_command = "< process_command >";

  if (!command) {
    abort();
  }

  if (!is_good_obj(context->world->database, player)) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_BUGS,
                         .primary = "CMD",
                         .secondary = "PLYR"},
              "Bad player in process_command: %ld", player);
    context->debug_command = cmdsave;
    goto exit;
  }

  /*
   * Make sure player isn't going or halted
   */

  if (is_going(context->world->database, player) ||
      (is_halted(context->world->database, player) &&
       !((typeof_obj(context->world->database, player) == OBJECT_TYPE_PLAYER) &&
         interactive))) {
    notify_printf(&context->evaluation, player,
                  "Attempt to execute command by halted object #%ld", player);
    context->debug_command = cmdsave;
    goto exit;
  }

  if (is_suspect(context->world->database, player)) {
    STARTLOG(context->log, LOG_SUSPECTCMDS | LOG_ALLCOMMANDS, "CMD", "SUS") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_command.LOG.allcmds");
      (void)snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
    send_channel(
        &context->evaluation, "SuspectsLog", "%s (#%ld) (in #%ld) entered: %s",
        game_object_name(context->world->database, player), player,
        game_object_location(context->world->database, player), command);
  } else {
    STARTLOG(context->log, LOG_ALLCOMMANDS, "CMD", "ALL") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_command.LOG.allcmds");
      (void)snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
  }

  /*
   * Reset recursion limits
   */
  command_context_reset_limits(context);

  /*
   * Eat leading whitespace, and space-compress if configured
   */

  size_t command_length = strlen(command);
  size_t command_offset = 0;

  while (command_offset < command_length &&
         (isspace)((unsigned char)*(const char *)checked_storage_at_const(
             command, command_length + 1, sizeof(char), command_offset)))
    command_offset++;
  command = checked_mutable_string_suffix(command, command_offset);
  command_length -= command_offset;
  context->debug_command = command;

  /*
   * Can we fix the @npemit thing?
   */
  if (configuration->space_compress && strncmp(command, "@npemit", 7) != 0) {
    size_t read_offset = 0;
    size_t write_offset = 0;

    while (read_offset < command_length) {
      while (read_offset < command_length) {
        const char character = *(const char *)checked_storage_at_const(
            command, command_length + 1, sizeof(char), read_offset);

        if ((isspace)((unsigned char)character))
          break;
        *(char *)checked_storage_at(command, command_length + 1, sizeof(char),
                                    write_offset++) = character;
        read_offset++;
      }
      while (read_offset < command_length &&
             (isspace)((unsigned char)*(const char *)checked_storage_at_const(
                 command, command_length + 1, sizeof(char), read_offset)))
        read_offset++;
      if (read_offset < command_length)
        *(char *)checked_storage_at(command, command_length + 1, sizeof(char),
                                    write_offset++) = ' ';
    }
    *(char *)checked_storage_at(command, command_length + 1, sizeof(char),
                                write_offset) = '\0';
    command_length = write_offset;
  }

  /*
   * Now comes the fun stuff.  First check for single-letter leadins.
   * We check these before checking HOME because
   * they are among the most frequently executed commands,
   * and they can never be the HOME command.
   */

  i = (unsigned char)*command;
  CMDENT *prefix_command = command_prefix_entry_at(registry, (size_t)i);
  if (prefix_command != nullptr && *command) {
    process_cmdent(&(CommandEntryDispatch){.context = context,
                                           .command = prefix_command,
                                           .player = player,
                                           .cause = cause,
                                           .arguments = command,
                                           .unparsed_command = command});
    context->debug_command = cmdsave;
    goto exit;
  }
  if ((*command == '.') && interactive) {
    macerr = do_macro(&context->match, context->runtime->command_registry,
                      context->runtime->macros, player, command, &macroout);
    if (!macerr)
      goto exit;
    if (macerr == 1) {
      StringCopy(command, macroout);
      free_lbuf(macroout);
    }
  } else
    macerr = 0;
  if (!do_comsystem(&context->evaluation, player, command))
    goto exit;

  /* Handle mecha stuff.. */
  if (btech_command_try_execute(
          context->btech, player,
          game_object_location(context->world->database, player), command))
    goto exit;
  /*
   * Check for the HOME command
   */

  if (string_compare(configuration, command, "home") == 0) {
    /* do_move()'s parameter isn't const-correct; "home" is only read. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
    move_command(&(MoveCommandRequest){.evaluation = &context->evaluation,
                                       .player = player,
                                       .direction = (char *)"home"});
#pragma clang diagnostic pop
    context->debug_command = cmdsave;
    goto exit;
  }

  /*
   * Only check for exits if we may use the goto command
   */
  if (check_access(context->world->database, configuration, player,
                   ((CMDENT *)registry->goto_command)->perms)) {
    /*
     * Check for an exit name
     */
    init_match_check_keys(&context->match, player, command, OBJECT_TYPE_EXIT);
    match_exit(&context->match);
    exit = last_match_result(&context->match);
    if (exit != NOTHING) {
      move_exit(&context->evaluation, player, exit, "You can't go that way.",
                0);
      context->debug_command = cmdsave;
      goto exit;
    }
  }
  /*
   * Set up a lowercase command and an arg pointer for the hashed
   * command check.  Since some types of argument
   * processing destroy the arguments, make a copy so that
   * we keep the original command line intact.  Store the
   * edible copy in lcbuf after the lowercased command.
   */
  /*
   * Removed copy of the rest of the command, since it's ok do allow
   * it to be trashed.  -dcm
   */

  lcbuf = alloc_lbuf("process_commands.LCbuf");
  size_t name_length = 0;

  while (name_length < command_length) {
    const char character = *(const char *)checked_storage_at_const(
        command, command_length + 1, sizeof(char), name_length);

    if ((isspace)((unsigned char)character))
      break;
    *(char *)checked_storage_at(lcbuf, LBUF_SIZE, sizeof(char), name_length) =
        ascii_to_lower(character);
    name_length++;
  }
  *(char *)checked_storage_at(lcbuf, LBUF_SIZE, sizeof(char), name_length) =
      '\0';
  size_t argument_offset = name_length;

  while (argument_offset < command_length &&
         (isspace)((unsigned char)*(const char *)checked_storage_at_const(
             command, command_length + 1, sizeof(char), argument_offset)))
    argument_offset++;
  arg = checked_mutable_string_suffix(command, argument_offset);

  /*
   * Strip off any command switches and save them
   */

  slashp = command_split_slash(lcbuf);

  /*
   * Check for a builtin command (or an alias of a builtin command)
   */

  cmdp = (CMDENT *)hash_table_find(lcbuf, &registry->commands);
  if (cmdp != nullptr) {
    if ((cmdp->callseq & CS_NO_MACRO) && macerr == 1)
      notify_checked(&context->evaluation, player, player,
                     "This command is unavailable as macro. Please use an "
                     "attribute instead.",
                     MSG_ME_ALL | MSG_F_DOWN);
    else
      process_cmdent(&(CommandEntryDispatch){.context = context,
                                             .command = cmdp,
                                             .switches = slashp,
                                             .player = player,
                                             .cause = cause,
                                             .arguments = arg,
                                             .unparsed_command = command});
    free_lbuf(lcbuf);
    context->debug_command = cmdsave;
    goto exit;
  }
  /* Lua handlers observe the original unmatched command. */
  if (!is_no_command(context->world->database, player))
    lua_succ +=
        lua_command_match(runtime->lua_owner->runtime, context->descriptor,
                          player, player, cause, command);
  if (has_location(context->world->database, player)) {
    lua_succ += lua_list_command_match(
        runtime->lua_owner->runtime, context->descriptor,
        game_object_contents(
            context->world->database,
            game_object_location(context->world->database, player)),
        player, cause, command);
    if (!is_no_command(context->world->database,
                       game_object_location(context->world->database, player)))
      lua_succ += lua_command_match(
          runtime->lua_owner->runtime, context->descriptor,
          game_object_location(context->world->database, player), player, cause,
          command);
  }
  if (has_contents(context->world->database, player))
    lua_succ += lua_list_command_match(
        runtime->lua_owner->runtime, context->descriptor,
        game_object_contents(context->world->database, player), player, cause,
        command);
  if (!lua_succ &&
      (game_object_zone(context->world->database,
                        game_object_location(context->world->database,
                                             player)) != NOTHING)) {
    if (typeof_obj(context->world->database,
                   game_object_zone(context->world->database,
                                    game_object_location(
                                        context->world->database, player))) ==
        OBJECT_TYPE_ROOM) {
      if (game_object_location(context->world->database, player) !=
          game_object_zone(context->world->database, player))
        lua_succ += lua_list_command_match(
            runtime->lua_owner->runtime, context->descriptor,
            game_object_contents(
                context->world->database,
                game_object_zone(
                    context->world->database,
                    game_object_location(context->world->database, player))),
            player, cause, command);
    } else if (!is_no_command(
                   context->world->database,
                   game_object_zone(context->world->database,
                                    game_object_location(
                                        context->world->database, player)))) {
      lua_succ += lua_command_match(
          runtime->lua_owner->runtime, context->descriptor,
          game_object_zone(
              context->world->database,
              game_object_location(context->world->database, player)),
          player, cause, command);
    }
  }
  if (!lua_succ &&
      (game_object_zone(context->world->database, player) != NOTHING) &&
      !is_no_command(context->world->database,
                     game_object_zone(context->world->database, player)) &&
      (game_object_zone(
           context->world->database,
           game_object_location(context->world->database, player)) !=
       game_object_zone(context->world->database, player)))
    lua_succ +=
        lua_command_match(runtime->lua_owner->runtime, context->descriptor,
                          game_object_zone(context->world->database, player),
                          player, cause, command);
  if (!lua_succ &&
      (game_object_zone(context->world->database,
                        game_object_location(context->world->database,
                                             player)) != NOTHING) &&
      (typeof_obj(context->world->database,
                  game_object_zone(context->world->database,
                                   game_object_location(
                                       context->world->database, player))) ==
       OBJECT_TYPE_ROOM) &&
      (game_object_location(context->world->database, player) !=
       game_object_zone(context->world->database, player))) {
    init_match_check_keys(&context->match, player, command, OBJECT_TYPE_EXIT);
    match_zone_exit(&context->match);
    exit = last_match_result(&context->match);
    if (exit != NOTHING) {
      free_lbuf(lcbuf);
      move_exit(&context->evaluation, player, exit, nullptr, 0);
      context->debug_command = cmdsave;
      goto exit;
    }
  }
  if (!lua_succ)
    lua_succ +=
        lua_global_command_match(runtime->lua_owner->runtime,
                                 context->descriptor, player, cause, command);
  succ = lua_succ;
  free_lbuf(lcbuf);

  /*
   * If we still didn't find anything, tell how to get help.
   */

  if (!succ) {
    notify_checked(&context->evaluation, player, player,
                   "Huh?  (Type \"help\" for help.)", MSG_ME_ALL | MSG_F_DOWN);
    STARTLOG(context->log, LOG_BADCOMMANDS, "CMD", "BAD") {
      log_name_and_loc(context->log, player);
      lcbuf = alloc_lbuf("process_commands.LOG.badcmd");
      (void)snprintf(lcbuf, LBUF_SIZE, " entered: '%s'", command);
      log_text(lcbuf);
      free_lbuf(lcbuf);
      ENDLOG(context->log);
    }
  }
  context->debug_command = cmdsave;

exit:
  return;
}

/*
 * ---------------------------------------------------------------------------
 * * list_cmdtable: List internal commands.
 */
