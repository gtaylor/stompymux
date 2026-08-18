/** @file
 * Explicit ownership for text that may borrow or own heap storage.
 */
#pragma once

typedef struct OwnedText {
  const char *text;
  char *owned;
} OwnedText;

/** Executes owned text borrow. @param[in] text Text to process. */

static inline OwnedText owned_text_borrow(const char *text) {
  return (OwnedText){.text = text};
}

/** Executes owned text take. @param[in,out] text Text to process. */

static inline OwnedText owned_text_take(char *text) {
  return (OwnedText){.text = text, .owned = text};
}

/** Executes owned text release. @param[in,out] text Text to process. */

void owned_text_release(OwnedText *text);
/** Executes owned text relinquish. @param[in,out] text Text to process. */

char *owned_text_relinquish(OwnedText *text);
