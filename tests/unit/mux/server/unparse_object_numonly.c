/* unparse_object_numonly.c -- owned object-name rendering tests */

#include <string.h>

#include "btech/ids.h"
#include "btech/special_objects.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/lbuf_text.h"

const char *game_object_name(GameDatabase *database, DbRef thing) {
  (void)database;
  (void)thing;
  return "Valid";
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *msg, int key) {
  (void)evaluation;
  (void)target;
  (void)sender;
  (void)msg;
  (void)key;
}

void btech_special_object_flag_changed(BtechContext *context,
                                       BtechObjectId player, // NOLINT
                                       BtechObjectId object, bool from,
                                       bool to) {
  (void)context;
  (void)player;
  (void)object;
  (void)from;
  (void)to;
}

int main(void) {
  GameObject objects[2] = {};
  GameDatabase database = {.object_storage = objects, .top = 1, .size = 1};
  game_database_object(&database, 0)->type = OBJECT_TYPE_THING;

  LbufText nothing = unparse_object_numonly(&database, NOTHING);
  LbufText home = unparse_object_numonly(&database, HOME);
  LbufText invalid = unparse_object_numonly(&database, 3);
  LbufText valid = unparse_object_numonly(&database, 0);
  int result = 0;

  if (nothing.owned == nullptr || strcmp(nothing.text, "*NOTHING*") != 0)
    result = 1;
  else if (home.owned == nullptr || strcmp(home.text, "*HOME*") != 0)
    result = 2;
  else if (invalid.owned == nullptr ||
           strcmp(invalid.text, "*ILLEGAL*(#3)") != 0)
    result = 3;
  else if (valid.owned == nullptr || strcmp(valid.text, "Valid(#0)") != 0)
    result = 4;

  lbuf_text_release(&nothing);
  lbuf_text_release(&home);
  lbuf_text_release(&invalid);
  lbuf_text_release(&valid);
  return result;
}
