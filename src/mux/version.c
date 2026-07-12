/*
 * version.c - version information
 */

#include "config.h"

#include "alloc.h"
#include "command.h"
#include "db.h"
#include "externs.h"
#include "mudconf.h"

#include "version.h"

void do_version(dbref player, dbref cause, int extra) {
  notify(player, mudstate.version);
}

char *mux_version = PACKAGE_STRING
#ifdef DEBUG
    " DEBUG"
#else
    " RELEASE"
#endif
    " built on " MUX_BUILD_DATE
    ;

void init_version(void) {
  strlcpy(mudstate.version, mux_version, sizeof(mudstate.version));

  STARTLOG(LOG_ALWAYS, "INI", "START") {
    log_text((char *)"Starting: ");
    log_text(mudstate.version);
    ENDLOG;
  }
}
