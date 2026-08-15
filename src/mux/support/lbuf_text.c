/* Explicit ownership for text that may borrow storage or own an LBUF. */

#include "mux/support/lbuf_text.h"

#include "mux/support/alloc.h"

void lbuf_text_release(LbufText *text) {
  if (text == nullptr)
    return;
  free_lbuf(text->owned);
  *text = (LbufText){};
}
