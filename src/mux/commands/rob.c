/*
 * rob.c -- Commands dealing with giving and taking things
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"

/*
 * ---------------------------------------------------------------------------
 * * give_thing, do_give: Give away things.
 */

static void give_thing(DbRef giver, DbRef recipient, int key, char *what) {
  DbRef thing;
  char *str, *sp;

  init_match(giver, what, TYPE_THING);
  match_possession();
  match_me();
  thing = match_result();

  switch (thing) {
  case NOTHING:
    notify(giver, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify(giver, "I don't know which you mean!");
    return;
  }

  if (thing == giver) {
    notify(giver, "You can't give yourself away!");
    return;
  }
  if (((Typeof(thing) != TYPE_THING) && (Typeof(thing) != TYPE_PLAYER)) ||
      !(Enter_ok(recipient) || controls(giver, recipient))) {
    notify(giver, "Permission denied.");
    return;
  }
  if (!could_doit(giver, thing, A_LGIVE)) {
    sp = str = alloc_lbuf("do_give.gfail");
    safe_str((char *)"You can't give ", str, &sp);
    safe_str(Name(thing), str, &sp);
    safe_str((char *)" away.", str, &sp);
    *sp = '\0';

    did_it(giver, thing, A_GFAIL, str, A_OGFAIL, NULL, A_AGFAIL, (char **)NULL,
           0);
    free_lbuf(str);
    return;
  }
  if (!could_doit(thing, recipient, A_LRECEIVE)) {
    sp = str = alloc_lbuf("do_give.rfail");
    safe_str(Name(recipient), str, &sp);
    safe_str((char *)" doesn't want ", str, &sp);
    safe_str(Name(thing), str, &sp);
    safe_chr('.', str, &sp);
    *sp = '\0';

    did_it(giver, recipient, A_RFAIL, str, A_ORFAIL, NULL, A_ARFAIL,
           (char **)NULL, 0);
    free_lbuf(str);
    return;
  }
  move_via_generic(thing, recipient, giver, 0);
  divest_object(thing);
  if (!(key & GIVE_QUIET)) {
    str = alloc_lbuf("do_give.thing.ok");
    StringCopy(str, Name(giver));
    notify_with_cause(recipient, giver,
                      tprintf("%s gave you %s.", str, Name(thing)));
    notify(giver, "Given.");
    notify_with_cause(thing, giver,
                      tprintf("%s gave you to %s.", str, Name(recipient)));
    free_lbuf(str);
  }
  did_it(giver, thing, A_DROP, NULL, A_ODROP, NULL, A_ADROP, (char **)NULL, 0);
  did_it(recipient, thing, A_SUCC, NULL, A_OSUCC, NULL, A_ASUCC, (char **)NULL,
         0);
}

void do_give(DbRef player, DbRef cause, int key, char *who, char *amnt) {
  DbRef recipient;

  /*
   * check recipient
   */

  init_match(player, who, TYPE_PLAYER);
  match_neighbor();
  match_possession();
  match_me();
  if (Long_Fingers(player)) {
    match_player();
    match_absolute();
  }
  recipient = match_result();
  switch (recipient) {
  case NOTHING:
    notify(player, "Give to whom?");
    return;
  case AMBIGUOUS:
    notify(player, "I don't know who you mean!");
    return;
  }

  give_thing(player, recipient, key, amnt);
}
