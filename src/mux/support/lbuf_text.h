/* Explicit ownership for text that may borrow storage or own an LBUF. */

#pragma once

typedef struct LbufText {
  const char *text;
  char *owned;
} LbufText;

static inline LbufText lbuf_text_borrow(const char *text) {
  return (LbufText){.text = text};
}

static inline LbufText lbuf_text_take(char *text) {
  return (LbufText){.text = text, .owned = text};
}

void lbuf_text_release(LbufText *text);
