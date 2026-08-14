/* Page recipient collection and last-page state tests. */

#include "mux/communication/page_recipients.h"

#include <stdint.h>
#include <stdio.h>

#include "mux/objects/db.h"
#include "mux/objects/player_account.h"
#include "mux/server/platform.h"

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top;
}

typedef struct ExpectedRecipient {
  size_t position;
  DbRef recipient;
} ExpectedRecipient;

static bool recipient_matches(GameDatabase *database,
                              ExpectedRecipient expected) {
  PlayerPageRecipientResult result =
      player_account_last_page_recipient(&(PlayerPageRecipientRequest){
          .account = {.database = database, .player = 0},
          .position = expected.position});
  return result.found && result.recipient == expected.recipient;
}

static int empty_list_preserves_last_page(GameDatabase *database) {
  const DbRef PREVIOUS[] = {99};
  PageRecipientList list = {0};

  if (!player_account_last_page_set(database, 0, PREVIOUS, 1) ||
      !page_recipient_list_initialize(&list, 3))
    return 1;
  page_recipient_list_store(&list, database, 0);
  const bool PRESERVED =
      player_account_last_page_count(database, 0) == 1 &&
      recipient_matches(database, (ExpectedRecipient){0, 99});
  page_recipient_list_destroy(&list);
  return PRESERVED ? 0 : 1;
}

static int recipients_preserve_order(GameDatabase *database) {
  PageRecipientList list = {0};

  if (!page_recipient_list_initialize(&list, 3) ||
      !page_recipient_list_append(&list, 42) ||
      !page_recipient_list_append(&list, 7) ||
      !page_recipient_list_append(&list, 999) ||
      page_recipient_list_append(&list, 1000) || list.count != 3) {
    page_recipient_list_destroy(&list);
    return 1;
  }
  page_recipient_list_store(&list, database, 0);
  const bool ORDERED =
      player_account_last_page_count(database, 0) == 3 &&
      recipient_matches(database, (ExpectedRecipient){0, 42}) &&
      recipient_matches(database, (ExpectedRecipient){1, 7}) &&
      recipient_matches(database, (ExpectedRecipient){2, 999});
  page_recipient_list_destroy(&list);
  return ORDERED ? 0 : 1;
}

static int initialization_failure_resets_list(void) {
  DbRef sentinel = 0;
  PageRecipientList list = {.recipients = &sentinel, .count = 2, .capacity = 3};

  if (page_recipient_list_initialize(&list, SIZE_MAX) ||
      list.recipients != nullptr || list.count != 0 || list.capacity != 0)
    return 1;
  page_recipient_list_destroy(&list);
  return 0;
}

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {.object_storage = objects, .top = 1, .size = 1};
  int failures = 0;

  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  if (empty_list_preserves_last_page(&database)) {
    (void)fprintf(stderr, "empty recipient list replaced last-page state\n");
    failures++;
  }
  if (recipients_preserve_order(&database)) {
    (void)fprintf(stderr, "page recipient order or capacity was incorrect\n");
    failures++;
  }
  if (initialization_failure_resets_list()) {
    (void)fprintf(stderr, "failed initialization left recipient list unsafe\n");
    failures++;
  }
  player_account_clear(&database, 0);
  return failures != 0;
}
