/*
 * version.c - version information
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"

#include "mux/server/version.h"

void do_version(DbRef player, DbRef cause, int extra) {
  notify(player, mudstate.version);
}

char *mux_version = BTMUX_VERSION_STRING
#ifdef DEBUG
    " DEBUG"
#else
    " RELEASE"
#endif
    " built on " MUX_BUILD_DATE;

void init_version(void) {
  strlcpy(mudstate.version, mux_version, sizeof(mudstate.version));

  STARTLOG(LOG_ALWAYS, "INI", "START") {
    log_text((char *)"Starting: ");
    log_text(mudstate.version);
    ENDLOG;
  }
}
