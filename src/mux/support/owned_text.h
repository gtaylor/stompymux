/* Explicit ownership for text that may borrow or own heap storage. */

#pragma once

typedef struct OwnedText {
  const char *text;
  char *owned;
} OwnedText;

static inline OwnedText owned_text_borrow(const char *text) {
  return (OwnedText){.text = text};
}

static inline OwnedText owned_text_take(char *text) {
  return (OwnedText){.text = text, .owned = text};
}

void owned_text_release(OwnedText *text);
char *owned_text_relinquish(OwnedText *text);
