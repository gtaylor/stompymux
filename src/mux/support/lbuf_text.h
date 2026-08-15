/* Explicit ownership for text that may borrow storage or own an LBUF. */

#pragma once

typedef struct LbufText {
  const char *text;
  char *owned;
} LbufText;

LbufText lbuf_text_borrow(const char *text);
LbufText lbuf_text_take(char *text);
void lbuf_text_release(LbufText *text);
