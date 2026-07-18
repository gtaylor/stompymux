/* access.c - Object visibility, lock, and hearing permission checks. */

#include "mux/world/access.h"

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/boolexp.h"
#include "mux/database/powers.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/formatting.h"
#include "mux/world/world_context.h"

int could_doit_with_context(EvaluationContext *context, DbRef player,
                            DbRef thing, int locknum) {
  char *key;
  DbRef aowner;
  long aflags;
  int doit;

  /*
   * no if nonplayer trys to get key
   */

  if (!is_player(context->world->database, player) &&
      has_key_flag(context->world->database, thing)) {
    return 0;
  }
  if (is_pass_locks(context->world->database, player))
    return 1;

  key =
      attribute_get(context->world->database, thing, locknum, &aowner, &aflags);
  doit = eval_boolexp_atr(context, player, thing, thing, key);
  free_lbuf(key);
  return doit;
}

int can_see(EvaluationContext *evaluation,
            const ServerConfiguration *configuration, DbRef player, DbRef thing,
            int can_see_loc) {
  /*
   * Don't show if all the following apply: * Sleeping players should *
   *
   * *  * * not be seen. * The thing is a disconnected player. * The
   * player * is  *  * * not a puppet.
   */

  if (configuration->dark_sleepers &&
      is_player(evaluation->world->database, thing) &&
      !is_connected(evaluation->world->database, thing) &&
      !is_puppet(evaluation->world->database, thing)) {
    return 0;
  }
  /*
   * You don't see yourself or exits
   */

  if ((player == thing) || is_exit(evaluation->world->database, thing)) {
    return 0;
  }
  /*
   * If loc is not dark, you see it if it's not dark or you control it.
   * * * * * If loc is dark, you see it if you control it.  Seeing your
   * * own * * * dark objects is controlled by see_own_dark. *
   * In * dark *  * locations, you also see things that are LIGHT and
   * !DARK.
   */

  if (can_see_loc) {
    return (!is_dark(evaluation->world->database, thing) ||
            (configuration->see_own_dark &&
             is_myopic_exam(evaluation, player, thing)));
  } else {
    return ((is_light(evaluation->world->database, thing) &&
             !is_dark(evaluation->world->database, thing)) ||
            (configuration->see_own_dark &&
             is_myopic_exam(evaluation, player, thing)));
  }
}
void handle_ears(EvaluationContext *evaluation, DbRef thing, int could_hear,
                 int can_hear) {
  char *buff, *bp;

  if (!could_hear && can_hear) {
    buff = alloc_lbuf("handle_ears.grow");
    StringCopy(buff, game_object_name(evaluation->world->database, thing));
    if (is_exit(evaluation->world->database, thing)) {
      for (bp = buff; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
    }
    notify_checked(evaluation, thing, thing,
                   tprintf("%s grows ears and can now hear.", buff),
                   (MSG_ME | MSG_NBR | MSG_LOC | MSG_INV));
    free_lbuf(buff);
  } else if (could_hear && !can_hear) {
    buff = alloc_lbuf("handle_ears.lose");
    StringCopy(buff, game_object_name(evaluation->world->database, thing));
    if (is_exit(evaluation->world->database, thing)) {
      for (bp = buff; *bp && (*bp != ';'); bp++)
        ;
      *bp = '\0';
    }
    notify_checked(evaluation, thing, thing,
                   tprintf("%s loses its ears and becomes deaf.", buff),
                   (MSG_ME | MSG_NBR | MSG_LOC | MSG_INV));
    free_lbuf(buff);
  }
}
