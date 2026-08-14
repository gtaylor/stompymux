/* Typed recipient collection for page delivery and last-page state. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/server/platform.h"

typedef struct GameDatabase GameDatabase;

typedef struct PageRecipientList {
  DbRef *recipients;
  size_t count;
  size_t capacity;
} PageRecipientList;

/* Always resets *list, including when initialization fails. */
[[nodiscard]] bool page_recipient_list_initialize(PageRecipientList *list,
                                                  size_t capacity);
void page_recipient_list_destroy(PageRecipientList *list);
[[nodiscard]] bool page_recipient_list_append(PageRecipientList *list,
                                              DbRef recipient);
void page_recipient_list_store(const PageRecipientList *list,
                               GameDatabase *database, DbRef player);
