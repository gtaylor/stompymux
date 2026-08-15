/* game.c - Core game notifications, database dumps, and shutdown operations. */

#include <asm-generic/errno-base.h>
#include <errno.h>
#include <sodium/utils.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

#include "btech/persistence.h"
#include "btech/special_objects.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/macro.h"
#include "mux/communication/channel_registry.h"
#include "mux/communication/comsys.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/persistence/commac_persistence.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h"
#include "mux/server/database_bootstrap.h"
#include "mux/server/file_cache.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/version.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/owned_text.h"
#include "mux/support/password.h"
#include "mux/world/database_check.h"
#include "mux/world/object_spatial.h"
#include "mux/world/player_cache.h"

static void do_dump_optimize(EvaluationContext *evaluation, DbRef player);
static void init_rlimit(MuxServer *server);

/*
 * used to allocate storage for temporary stuff, cleared before command
 * execution
 */

void do_dump(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  notify_checked(evaluation, player, player, "Dumping...",
                 MSG_ME_ALL | MSG_F_DOWN);

  /*
   * DUMP_OPTIMIZE takes advantage of a feature of GDBM to compress
   * unused space in the database, and will not be very useful
   * except sparingly, perhaps done every month or so.
   */

  if (key & DUMP_OPTIMIZE)
    do_dump_optimize(evaluation, player);
  else
    fork_and_dump(invocation->context->runtime->server_control, key);
}

static void do_dump_optimize(EvaluationContext *evaluation, DbRef player) {
  raw_notify(evaluation, player, "Database is memory based.");
}

/**
 * print out stuff into error file
 */
void report(CommandContext *command) {
  STARTLOG(command->log, LOG_BUGS, "BUG", "INFO") {
    log_text("Command: '");
    log_text(command->debug_command);
    log_text("'");
    ENDLOG(command->log);
  }
  if (is_good_obj(command->world->database, command->player)) {
    STARTLOG(command->log, LOG_BUGS, "BUG", "INFO") {
      log_text("Player: ");
      log_name_and_loc(command->log, command->player);
      if ((command->enactor != command->player) &&
          is_good_obj(command->world->database, command->enactor)) {
        log_text(" Enactor: ");
        log_name_and_loc(command->log, command->enactor);
      }
      ENDLOG(command->log);
    }
  }
}

/**
 * Notifies the object #target of the message msg, and
 * optionally notify the contents, neighbors, and location also.
 */
static OwnedText format_forwarded_message(const char *msg, const char *prefix) {
  char *plain = alloc_lbuf("format_forwarded_message");
  char *cursor = plain;
  if (prefix && *prefix) {
    safe_str(prefix, plain, &cursor);
    safe_chr(' ', plain, &cursor);
  }
  safe_str(msg, plain, &cursor);
  *cursor = '\0';
  return owned_text_take(plain);
}

static OwnedText dflt_from_msg(GameDatabase *database, DbRef sender,
                               DbRef sendloc) {
  char *tp;
  char *tbuff;

  tp = tbuff = alloc_lbuf("notify_checked.fwdlist");
  safe_str("From ", tbuff, &tp);
  if (is_good_obj(database, sendloc))
    safe_str(game_object_name(database, sendloc), tbuff, &tp);
  else
    safe_str(game_object_name(database, sender), tbuff, &tp);
  safe_chr(',', tbuff, &tp);
  *tp = '\0';
  return owned_text_take(tbuff);
}

void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *msg, int key) {
  char *msg_copy;
  DbRef targetloc;
  DbRef recip;
  DbRef obj;
  int has_neighbors;
  int target_audible;

  /*
   * If speaker is invalid or message is empty, just exit
   */

  if (!is_good_obj(evaluation->world->database, target) || !msg || !*msg)
    return;

  /*
   * Enforce a recursion limit
   */

  evaluation->notification_nesting++;
  if (evaluation->notification_nesting >=
      evaluation->world->configuration->ntfy_nest_lim) {
    evaluation->notification_nesting--;
    return;
  }
  if (key & MSG_ME) {
    msg_copy = alloc_lbuf("notify_checked");
    (void)string_copy_bounded(msg_copy, LBUF_SIZE, msg);
  } else {
    msg_copy = nullptr;
  }

  switch (typeof_obj(evaluation->world->database, target)) {
  case OBJECT_TYPE_PLAYER:
    if (key & MSG_ME) {
      raw_notify(evaluation, target, msg_copy);
    }
    [[fallthrough]];
  case OBJECT_TYPE_THING:
  case OBJECT_TYPE_ROOM:

    has_neighbors = has_location(evaluation->world->database, target);
    targetloc = where_is(evaluation->world->database, target);
    target_audible = is_audible(evaluation->world->database, target);

    /*
     * Deliver message through audible exits
     */

    if (key & MSG_INV_EXITS) {
      DOLIST(evaluation->world->database, obj,
             game_object_exits(evaluation->world->database, target)) {
        recip = game_object_location(evaluation->world->database, obj);
        if (is_audible(evaluation->world->database, obj) && recip != target) {
          OwnedText distance_message =
              format_forwarded_message(msg, "From a distance,");
          notify_checked(evaluation, recip, sender, distance_message.text,
                         MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE);
          owned_text_release(&distance_message);
        }
      }
    }
    /*
     * Deliver message through neighboring audible exits
     */

    if (has_neighbors && ((key & MSG_NBR_EXITS) ||
                          ((key & MSG_NBR_EXITS_A) && target_audible))) {

      /*
       * If from inside, we have to add the prefix string *
       *
       * *  * * of * the container.
       */

      OwnedText forwarded = owned_text_borrow(msg);
      if (key & MSG_S_INSIDE) {
        OwnedText prefix =
            dflt_from_msg(evaluation->world->database, sender, target);
        forwarded = format_forwarded_message(msg, prefix.text);
        owned_text_release(&prefix);
      }

      DOLIST(evaluation->world->database, obj,
             game_object_exits(
                 evaluation->world->database,
                 game_object_location(evaluation->world->database, target))) {
        recip = game_object_location(evaluation->world->database, obj);
        if (is_good_obj(evaluation->world->database, recip) &&
            is_audible(evaluation->world->database, obj) &&
            (recip != targetloc) && (recip != target)) {
          OwnedText distance_message =
              format_forwarded_message(forwarded.text, "From a distance,");
          notify_checked(evaluation, recip, sender, distance_message.text,
                         MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE);
          owned_text_release(&distance_message);
        }
      }
      owned_text_release(&forwarded);
    }
    /*
     * Deliver message to contents
     */

    if (key & MSG_INV) {

      /*
       * Don't prefix the message if we were given the * *
       * * * MSG_NOPREFIX key.
       */

      OwnedText forwarded = owned_text_borrow(msg);
      if (key & MSG_S_OUTSIDE)
        forwarded = format_forwarded_message(msg, "");
      DOLIST(evaluation->world->database, obj,
             game_object_contents(evaluation->world->database, target)) {
        if (obj != target) {
          notify_checked(evaluation, obj, sender, forwarded.text,
                         MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE);
        }
      }
      owned_text_release(&forwarded);
    }
    /*
     * Deliver message to neighbors
     */

    if (has_neighbors &&
        ((key & MSG_NBR) || ((key & MSG_NBR_A) && target_audible))) {
      OwnedText forwarded = owned_text_borrow(msg);
      if (key & MSG_S_INSIDE) {
        OwnedText prefix =
            dflt_from_msg(evaluation->world->database, sender, target);
        forwarded = format_forwarded_message(msg, prefix.text);
        owned_text_release(&prefix);
      }
      DOLIST(evaluation->world->database, obj,
             game_object_contents(evaluation->world->database, targetloc)) {
        if ((obj != target) && (obj != targetloc)) {
          notify_checked(evaluation, obj, sender, forwarded.text,
                         MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE);
        }
      }
      owned_text_release(&forwarded);
    }
    /*
     * Deliver message to container
     */

    if (has_neighbors &&
        ((key & MSG_LOC) || ((key & MSG_LOC_A) && target_audible))) {
      OwnedText forwarded = owned_text_borrow(msg);
      if (key & MSG_S_INSIDE) {
        OwnedText prefix =
            dflt_from_msg(evaluation->world->database, sender, target);
        forwarded = format_forwarded_message(msg, prefix.text);
        owned_text_release(&prefix);
      }
      notify_checked(evaluation, targetloc, sender, forwarded.text,
                     MSG_ME | MSG_F_UP | MSG_S_INSIDE);
      owned_text_release(&forwarded);
    }
    break;
  default:
    break;
  }
  if (msg_copy)
    free_buf(msg_copy);
  evaluation->notification_nesting--;
}

void notify_excluding(const ExcludingNotification *notification) {
  EvaluationContext *evaluation = notification->evaluation;
  DbRef loc = notification->location;
  DbRef player = notification->sender;
  const char *msg = notification->message;
  DbRef first;

  bool location_excluded = false;
  for (size_t index = 0; index < notification->exception_count; index++)
    if (loc == *(const DbRef *)checked_storage_at_const(
                   notification->exceptions, 2, sizeof(DbRef), index))
      location_excluded = true;
  if (!location_excluded)
    notify_checked(evaluation, loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A));
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, loc)) {
    bool excluded = false;
    for (size_t index = 0; index < notification->exception_count; index++)
      if (first == *(const DbRef *)checked_storage_at_const(
                       notification->exceptions, 2, sizeof(DbRef), index))
        excluded = true;
    if (!excluded)
      notify_checked(evaluation, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
  }
}

void do_shutdown(CommandInvocation *invocation) {
  server_shutdown(&(ServerShutdownRequest){
      .control = invocation->context->runtime->server_control,
      .player = invocation->player,
      .options = invocation->key,
      .message = invocation->first});
}

void server_shutdown(const ServerShutdownRequest *request) {
  ServerControl *control = request->control;
  DbRef player = request->player;
  int key = request->options;
  const char *message = request->message;
  btech_special_objects_reset(control->btech);
  if (player != NOTHING) {
    raw_broadcast(control->descriptors, 0, "Game: Shutdown by %s",
                  game_object_name(control->database, player));
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown by ");
      log_name(control->log, player);
      ENDLOG(control->log);
    }
  } else {
    raw_broadcast(control->descriptors, 0, "Game: Fatal Error: %s", message);
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Fatal error: ");
      log_text(message);
      ENDLOG(control->log);
    }
  }
  if (player != NOTHING) {
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown status: ");
      log_text(message);
      ENDLOG(control->log);
    }
  }

  /*
   * Do we perform a normal or an emergency shutdown?  Normal shutdown
   * * * * * is handled by exiting the server lifecycle event loop,
   * emergency  * * * * shutdown is done here.
   */

  if (key & SHUTDN_PANIC) {

    /*
     * Close down the network interface
     */

    server_lifecycle_close_connections(control->lifecycle, true,
                                       "Going down - Bye.\n");

    /*
     * Close the attribute text db and dump the header db
     */

    STARTLOG(control->log, LOG_ALWAYS, "DMP", "PANIC") {
      log_text("Panic dump: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
    dump_database_internal(control, DUMP_CRASHED);

    STARTLOG(control->log, LOG_ALWAYS, "DMP", "DONE") {
      log_text("Panic dump complete: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
  } else if (key & SHUTDN_KILLED) {
    STARTLOG(control->log, LOG_ALWAYS, "DMP", "KILLED") {
      log_text("Killed dump: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
    dump_database_internal(control, DUMP_KILLED);
    STARTLOG(control->log, LOG_ALWAYS, "DMP", "DONE") {
      log_text("Killed dump complete: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
  }
  /*
   * Set up for normal shutdown
   */

  server_lifecycle_stop(control->lifecycle);
}

int dump_database_internal(ServerControl *control, int dump_type) {
  return gamedb_dump(control->persistence, dump_type);
}

void dump_database(ServerControl *control) {
  STARTLOG(control->log, LOG_DBSAVES, "DMP", "DUMP") {
    log_text("Dumping: ");
    log_text(control->configuration->database.gamedb);
    ENDLOG(control->log);
  }

  dump_database_internal(control, DUMP_NORMAL);
  STARTLOG(control->log, LOG_DBSAVES, "DMP", "DONE") {
    log_text("Dump complete: ");
    log_text(control->configuration->database.gamedb);
    ENDLOG(control->log);
  }
}

void fork_and_dump(ServerControl *control, int key) {
  if (*control->configuration->database.dump_msg)
    raw_broadcast(control->descriptors, 0, "%s",
                  control->configuration->database.dump_msg);

  log_error((LogEntry){.log = control->log,
                       .key = LOG_DBSAVES,
                       .primary = "DMP",
                       .secondary = "CHKPT"},
            "Saving database: %s", control->configuration->database.gamedb);

  if (!key || (key & DUMP_STRUCT)) {
    if (control->configuration->database.fork_dump) {
      /* Fork and dump.  */
      switch (fork()) {
      case -1: /* fork() failed */
        /* FIXME: Make this error message conform.  */
        log_perror(&(LogSystemError){.log = control->log,
                                     .primary = "DMP",
                                     .secondary = "FAIL",
                                     .failing_object = "fork()"});
        return;

      case 0: /* child */
        server_lifecycle_unbind_signals(control->lifecycle);
        dump_database_internal(control, DUMP_NORMAL);
        /* You generally don't want to run atexit()
         * handlers and that sort of thing.  */
        _exit(0);
        break;

      default: /* parent */
        break;
      }
    } else {
      /* Just dump.  */
      dump_database_internal(control, DUMP_NORMAL);
    }
  }

  if (*control->configuration->database.postdump_msg)
    raw_broadcast(control->descriptors, 0, "%s",
                  control->configuration->database.postdump_msg);
}

static int load_game(MuxServer *server) {
  STARTLOG(&server->log, LOG_STARTUP, "INI", "LOAD") {
    log_text("Loading: ");
    log_text(server->configuration->database.gamedb);
    ENDLOG(&server->log);
  };
  if (gamedb_load(&server->persistence,
                  server->configuration->database.gamedb) < 0) {
    STARTLOG(&server->log, LOG_ALWAYS, "INI", "FATAL") {
      log_text("Error loading ");
      log_text(server->configuration->database.gamedb);
      ENDLOG(&server->log);
    }
    return -1;
  }

  /* Load the mecha stuff.. */
  btech_special_objects_load(server->btech);

  STARTLOG(&server->log, LOG_STARTUP, "INI", "LOAD") {
    log_text("Load complete.");
    ENDLOG(&server->log);
  }
  /*
   * everything ok
   */
  return (0);
}

bool is_hearer(EvaluationContext *evaluation, DbRef thing) {
  return is_connected(evaluation->world->database, thing);
}

void do_readcache(CommandInvocation *invocation) {
  fcache_load(&invocation->context->evaluation,
              invocation->context->runtime->files, invocation->player);
}

int main(int argc, char *argv[]) {
  MuxServer server;
  const char *config_file;
  struct stat database_status;
  bool create_database;
  char god_password[BOOTSTRAP_PASSWORD_SIZE];
  char wizard_password[BOOTSTRAP_PASSWORD_SIZE];
  char *argument_one =
      argc > 1 ? *(char **)checked_storage_at((void *)argv, (size_t)argc,
                                              sizeof(*argv), 1)
               : nullptr;

  if (argc > 2 || (argc > 1 && !strcmp(argument_one, "-s")) ||
      (argc > 1 && strcmp(argument_one, "--restart") == 0)) {
    (void)fprintf(stderr, "Usage: %s [config-file]\n",
                  *(char **)checked_storage_at((void *)argv, (size_t)argc,
                                               sizeof(*argv), 0));
    exit(1);
  }

  config_file = CONF_FILE;
  if (argc > 1) {
    config_file = argument_one;
  }
  if (!mux_server_create(&server)) {
    (void)fprintf(stderr, "Unable to create MUX server resources.\n");
    return 2;
  }
  server.start_time = time(nullptr);
  if (server.start_time == (time_t)-1)
    server.start_time = 0;
  server.process_start_time = server.start_time;
  btech_context_set_process_start_time(server.btech, server.process_start_time);
  server.database.top = -1;
  configuration_initialize(&server.configuration_context);
  init_rlimit(&server);
  init_chantab(&server.channels);
  init_flagtab(&server.world_indexes);
  init_powertab(&server.world_indexes);
  init_version(&server);

  hash_table_initialize(&server.world_indexes.players, 250 * HASH_FACTOR);
  if (configuration_read(&server.configuration_context, config_file) < 0)
    goto fail;

  if (!password_initialize()) {
    (void)fprintf(stderr, "Unable to initialize password hashing.\n");
    goto fail;
  }

  if (!*server.configuration->database.gamedb) {
    (void)fprintf(
        stderr, "Required configuration directive game_database is missing.\n");
    goto fail;
  }

  if (btech_persistence_register(&server.persistence, server.btech) < 0) {
    (void)fprintf(stderr, "Unable to register BTech SQLite persistence.\n");
    goto fail;
  }

  if (commac_persistence_register(&server.persistence) < 0) {
    (void)fprintf(stderr, "Unable to register commac SQLite persistence.\n");
    goto fail;
  }

  if (!mux_server_load_content(&server)) {
    (void)fprintf(stderr, "Unable to load MUX server content.\n");
    goto fail;
  }
  db_free(&server.database);

  server.record_players = 0;
  if (!database_bootstrap_god_is_wizard_player(server.configuration)) {
    (void)fprintf(stderr,
                  "Invalid database bootstrap configuration: #1 must be a "
                  "player with wizard = true.\n");
    goto fail;
  }
  create_database =
      ((stat(server.configuration->database.gamedb, &database_status) < 0 &&
        errno == ENOENT) != 0);
  if (create_database) {
    if (database_bootstrap(&server.background_command.evaluation, god_password,
                           wizard_password) < 0 ||
        gamedb_create(&server.persistence) < 0)
      goto fail;
    (void)fprintf(stderr,
                  "Created %s. Initial credentials (shown once):\n"
                  "  GOD (#1): %s\n  Wizard (#2): %s\n",
                  server.configuration->database.gamedb, god_password,
                  wizard_password);
    sodium_memzero(god_password, sizeof(god_password));
    sodium_memzero(wizard_password, sizeof(wizard_password));
  } else if (load_game(&server) < 0) {
    STARTLOG(&server.log, LOG_ALWAYS, "INI", "LOAD") {
      log_text("Couldn't load: ");
      log_text(server.configuration->database.gamedb);
      ENDLOG(&server.log);
    }
    goto fail;
  }
  server_lifecycle_prepare(server.lifecycle);

  /*
   * Do a consistency check and set up the freelist
   */

  database_check(&(DatabaseCheckRequest){
      .evaluation = &server.background_command.evaluation, .player = NOTHING});

  /*
   * Reset all the hash stats
   */

  hash_table_reset(&server.command_registry.commands);
  hash_table_reset(&server.command_registry.macros);
  channel_registry_reset_statistics(&server.channels);
  hash_table_reset(&server.world_indexes.flags);
  hash_table_reset(&server.world_indexes.players);

  if (!server_lifecycle_boot(server.lifecycle)) {
    goto fail;
  }

  /*
   * go do it
   */

  server_lifecycle_run(server.lifecycle, server.configuration->port);

  server_lifecycle_close_connections(server.lifecycle, false,
                                     "Going down - Bye");
  dump_database(&server.server_control);

  mux_server_destroy(&server);
  return 0;

fail:
  mux_server_destroy(&server);
  return 2;
}

static void init_rlimit(MuxServer *server) {
  struct rlimit rlimit;

  if (getrlimit(RLIMIT_NOFILE, &rlimit)) {
    log_perror(&(LogSystemError){.log = &server->log,
                                 .primary = "RLM",
                                 .secondary = "FAIL",
                                 .failing_object = "getrlimit()"});
    return;
  }
  rlimit.rlim_cur = rlimit.rlim_max;
  if (setrlimit(RLIMIT_NOFILE, &rlimit))
    log_perror(&(LogSystemError){.log = &server->log,
                                 .primary = "RLM",
                                 .secondary = "FAIL",
                                 .failing_object = "setrlimit()"});
}
