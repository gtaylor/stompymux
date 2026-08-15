/*
 * version.c - version information
 */

#include "mux/server/version.h"
#include "btmux_build_config.h"
#include "mux/commands/command_context.h" // IWYU pragma: keep
#include "mux/commands/command_handlers.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

void do_version(CommandInvocation *invocation) {
  notify_checked(&invocation->context->evaluation, invocation->player,
                 invocation->player, invocation->context->runtime->version,
                 MSG_ME_ALL | MSG_F_DOWN);
}

static constexpr char MUX_VERSION[] =
    BTECH_VERSION_STRING " RELEASE built on " MUX_BUILD_DATE;

void init_version(MuxServer *server) {
  (void)string_copy_bounded(server->version, sizeof(server->version),
                            MUX_VERSION);

  STARTLOG(&server->log, LOG_ALWAYS, "INI", "START") {
    log_text("Starting: ");
    log_text(server->version);
    ENDLOG(&server->log);
  }
}
