/* Typed recipient collection for page delivery and last-page state. */

#include "mux/communication/page_recipients.h"

#include <stdint.h>
#include <stdlib.h>

#include "mux/objects/player_account.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

bool page_recipient_list_initialize(PageRecipientList *list, size_t capacity) {
  *list = (PageRecipientList){};
  list->recipients = capacity > 0 ? checked_storage_try_allocate_array(
                                        capacity, sizeof(*list->recipients))
                                  : nullptr;
  if (capacity > 0 && list->recipients == nullptr)
    return false;
  list->count = 0;
  list->capacity = capacity;
  return true;
}

void page_recipient_list_destroy(PageRecipientList *list) {
  free(list->recipients);
  *list = (PageRecipientList){};
}

bool page_recipient_list_append(PageRecipientList *list, DbRef recipient) {
  if (list->count >= list->capacity)
    return false;
  *(DbRef *)checked_storage_at(list->recipients, list->capacity,
                               sizeof(*list->recipients), list->count++) =
      recipient;
  return true;
}

void page_recipient_list_store(const PageRecipientList *list,
                               GameDatabase *database, DbRef player) {
  if (list->count > 0)
    (void)player_account_last_page_set(database, player, list->recipients,
                                       list->count);
}
