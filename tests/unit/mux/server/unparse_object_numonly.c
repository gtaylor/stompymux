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
  GameObject objects[3] = {};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};
  game_database_object(&database, 0)->type = OBJECT_TYPE_THING;
  game_database_object(&database, 0)->has_dark_flag = true;
  game_database_object(&database, GOD)->type = OBJECT_TYPE_PLAYER;

  LbufText nothing = unparse_object_numonly(&database, NOTHING);
  LbufText home = unparse_object_numonly(&database, HOME);
  LbufText invalid = unparse_object_numonly(&database, 3);
  LbufText valid = unparse_object_numonly(&database, 0);
  LbufText rendered_nothing = unparse_object(&database, nullptr, 0, NOTHING);
  LbufText rendered_home = unparse_object(&database, nullptr, 0, HOME);
  LbufText rendered_invalid = unparse_object(&database, nullptr, 0, 3);
  LbufText examinable = unparse_object(&database, nullptr, GOD, 0);
  LbufText hidden_details = unparse_object(&database, nullptr, 0, 0);
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
  else if (rendered_nothing.owned == nullptr ||
           strcmp(rendered_nothing.text, "*NOTHING*") != 0)
    result = 5;
  else if (rendered_home.owned == nullptr ||
           strcmp(rendered_home.text, "*HOME*") != 0)
    result = 6;
  else if (rendered_invalid.owned == nullptr ||
           strcmp(rendered_invalid.text, "*ILLEGAL*(#3)") != 0)
    result = 7;
  else if (examinable.owned == nullptr ||
           strcmp(examinable.text, "Valid(#0:D)") != 0)
    result = 8;
  else if (hidden_details.owned == nullptr ||
           strcmp(hidden_details.text, "Valid") != 0)
    result = 9;

  lbuf_text_release(&nothing);
  lbuf_text_release(&home);
  lbuf_text_release(&invalid);
  lbuf_text_release(&valid);
  lbuf_text_release(&rendered_nothing);
  lbuf_text_release(&rendered_home);
  lbuf_text_release(&rendered_invalid);
  lbuf_text_release(&examinable);
  lbuf_text_release(&hidden_details);
  return result;
}
