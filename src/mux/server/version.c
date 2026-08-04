/*
 * version.c - version information
 */

#include "mux/server/game.h"
#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/commands/command_handlers.h"
#include "mux/objects/db.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

#include "mux/server/version.h"

void do_version(CommandInvocation *invocation) {
  notify_checked(&invocation->context->evaluation, invocation->player,
                 invocation->player, invocation->context->runtime->version,
                 MSG_ME_ALL | MSG_F_DOWN);
}

const char *mux_version = BTMUX_VERSION_STRING
#ifdef DEBUG
    " DEBUG"
#else
    " RELEASE"
#endif
    " built on " MUX_BUILD_DATE;

void init_version(MuxServer *server) {
  strlcpy(server->version, mux_version, sizeof(server->version));

  STARTLOG(&server->log, LOG_ALWAYS, "INI", "START") {
    log_text("Starting: ");
    log_text(server->version);
    ENDLOG(&server->log);
  }
}
