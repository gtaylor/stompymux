/* Explicit ownership for text that may borrow or own heap storage. */

#include "mux/support/owned_text.h"

#include "mux/support/alloc.h"

void owned_text_release(OwnedText *text) {
  if (text == nullptr)
    return;
  free_buf(text->owned);
  *text = (OwnedText){};
}

char *owned_text_relinquish(OwnedText *text) {
  if (text == nullptr)
    return nullptr;
  char *owned = text->owned;
  text->text = nullptr;
  text->owned = nullptr;
  return owned;
}
