/* Explicit ownership for text that may borrow storage or own an LBUF. */

#include "mux/support/lbuf_text.h"

#include "mux/support/alloc.h"

LbufText lbuf_text_borrow(const char *text) { return (LbufText){.text = text}; }

LbufText lbuf_text_take(char *text) {
  return (LbufText){.text = text, .owned = text};
}

void lbuf_text_release(LbufText *text) {
  if (text == nullptr)
    return;
  free_lbuf(text->owned);
  *text = (LbufText){};
}
