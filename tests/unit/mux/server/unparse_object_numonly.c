/* unparse_object_numonly.c -- owned object-name rendering tests */

#include <string.h>

#include "btech/ids.h"
#include "btech/special_objects.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/owned_text.h"

const char *game_object_name(GameDatabase *database [[maybe_unused]],
                             DbRef thing [[maybe_unused]]) {
  return "Valid";
}

// NOLINTNEXTLINE(bugprone-easily-swappable-parameters)
void notify_checked(EvaluationContext *evaluation [[maybe_unused]],
                    DbRef target [[maybe_unused]],
                    DbRef sender [[maybe_unused]],
                    const char *msg [[maybe_unused]],
                    int key [[maybe_unused]]) {}

void btech_special_object_flag_changed(BtechContext *context [[maybe_unused]],
                                       BtechObjectId player
                                       [[maybe_unused]], // NOLINT
                                       BtechObjectId object [[maybe_unused]],
                                       bool from [[maybe_unused]],
                                       bool to [[maybe_unused]]) {}

int main(void) {
  GameObject objects[3] = {};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};
  game_database_object(&database, 0)->type = OBJECT_TYPE_THING;
  game_database_object(&database, 0)->has_dark_flag = true;
  game_database_object(&database, GOD)->type = OBJECT_TYPE_PLAYER;

  OwnedText nothing = unparse_object_numonly(&database, NOTHING);
  OwnedText home = unparse_object_numonly(&database, HOME);
  OwnedText invalid = unparse_object_numonly(&database, 3);
  OwnedText valid = unparse_object_numonly(&database, 0);
  OwnedText rendered_nothing = unparse_object(&database, nullptr, 0, NOTHING);
  OwnedText rendered_home = unparse_object(&database, nullptr, 0, HOME);
  OwnedText rendered_invalid = unparse_object(&database, nullptr, 0, 3);
  OwnedText examinable = unparse_object(&database, nullptr, GOD, 0);
  OwnedText hidden_details = unparse_object(&database, nullptr, 0, 0);
  ObjectFlagSet flags = {};
  object_flag_set_set(&flags, OBJECT_FLAG_DARK, true);
  OwnedText decoded =
      decode_flags(&(DecodeFlagsRequest){.database = &database,
                                         .player = GOD,
                                         .object_type = OBJECT_TYPE_THING,
                                         .flags = &flags});
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
  else if (decoded.owned == nullptr || strcmp(decoded.text, "D") != 0)
    result = 10;

  owned_text_release(&nothing);
  owned_text_release(&home);
  owned_text_release(&invalid);
  owned_text_release(&valid);
  owned_text_release(&rendered_nothing);
  owned_text_release(&rendered_home);
  owned_text_release(&rendered_invalid);
  owned_text_release(&examinable);
  owned_text_release(&hidden_details);
  owned_text_release(&decoded);
  return result;
}
