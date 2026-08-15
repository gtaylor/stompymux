/* owned_text.c -- borrowed and owned text tests */

#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/owned_text.h"
#include "mux/support/stringutil.h"

static int check_borrowed_text(void) {
  const char ORIGINAL[] = "borrowed";
  OwnedText text = owned_text_borrow(ORIGINAL);

  if (text.text != ORIGINAL || text.owned != nullptr)
    return 1;
  owned_text_release(&text);
  owned_text_release(&text);
  if (strcmp(ORIGINAL, "borrowed") != 0)
    return 2;
  return text.text == nullptr && text.owned == nullptr ? 0 : 3;
}

static int check_owned_text(void) {
  char *owned = alloc_lbuf("owned_text_test");
  (void)string_copy_bounded(owned, LBUF_SIZE, "owned");
  OwnedText text = owned_text_take(owned);

  if (text.text != owned || text.owned != owned)
    return 4;
  if (strcmp(text.text, "owned") != 0)
    return 5;
  owned_text_release(&text);
  owned_text_release(&text);
  return text.text == nullptr && text.owned == nullptr ? 0 : 6;
}

static int check_normalized_text(void) {
  OwnedText munged = munge_space("  one\t two  ");
  OwnedText trimmed = trim_spaces("\n three   four\r\n");
  int result = 0;

  if (munged.owned == nullptr || strcmp(munged.text, "one two") != 0)
    result = 7;
  else if (trimmed.owned == nullptr || strcmp(trimmed.text, "three four") != 0)
    result = 8;

  owned_text_release(&munged);
  owned_text_release(&trimmed);
  return result;
}

static int check_relinquished_text(void) {
  char *owned = alloc_lbuf("owned_text_relinquish_test");
  OwnedText text = owned_text_take(owned);
  char *relinquished = owned_text_relinquish(&text);

  if (relinquished != owned || text.text != nullptr || text.owned != nullptr)
    return 9;
  if (owned_text_relinquish(nullptr) != nullptr)
    return 10;
  free_buf(relinquished);
  owned_text_release(&text);
  return 0;
}

int main(void) {
  const int BORROWED_RESULT = check_borrowed_text();
  if (BORROWED_RESULT != 0)
    return BORROWED_RESULT;
  const int OWNED_RESULT = check_owned_text();
  if (OWNED_RESULT != 0)
    return OWNED_RESULT;
  const int NORMALIZED_RESULT = check_normalized_text();
  if (NORMALIZED_RESULT != 0)
    return NORMALIZED_RESULT;
  return check_relinquished_text();
}
