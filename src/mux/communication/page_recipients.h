/** @file
 * Typed recipient collection for page delivery and last-page state.
 */
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
/** Initializes page recipient list. @param[out] list List. @param[in] capacity
 * Capacity. */

[[nodiscard]] bool page_recipient_list_initialize(PageRecipientList *list,
                                                  size_t capacity);
/** Destroys page recipient list. @param[in,out] list List. */

void page_recipient_list_destroy(PageRecipientList *list);
/** Executes page recipient list append. @param[in,out] list List. @param[in]
 * recipient Recipient. */

[[nodiscard]] bool page_recipient_list_append(PageRecipientList *list,
                                              DbRef recipient);
/** Executes page recipient list store. @param[in] list List. @param[in,out]
 * database Game database. @param[in] player Player object. */

void page_recipient_list_store(const PageRecipientList *list,
                               GameDatabase *database, DbRef player);
