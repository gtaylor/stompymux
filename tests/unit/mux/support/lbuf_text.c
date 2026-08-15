/* lbuf_text.c -- borrowed and owned LBUF text tests */

#include <string.h>

#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/lbuf_text.h"

static int check_borrowed_text(void) {
  const char original[] = "borrowed";
  LbufText text = lbuf_text_borrow(original);

  if (text.text != original || text.owned != nullptr)
    return 1;
  lbuf_text_release(&text);
  lbuf_text_release(&text);
  if (strcmp(original, "borrowed") != 0)
    return 2;
  return text.text == nullptr && text.owned == nullptr ? 0 : 3;
}

static int check_owned_text(void) {
  char *owned = alloc_lbuf("lbuf_text_test");
  (void)string_copy_bounded(owned, LBUF_SIZE, "owned");
  LbufText text = lbuf_text_take(owned);

  if (text.text != owned || text.owned != owned)
    return 4;
  if (strcmp(text.text, "owned") != 0)
    return 5;
  lbuf_text_release(&text);
  lbuf_text_release(&text);
  return text.text == nullptr && text.owned == nullptr ? 0 : 6;
}

int main(void) {
  const int BORROWED_RESULT = check_borrowed_text();
  if (BORROWED_RESULT != 0)
    return BORROWED_RESULT;
  return check_owned_text();
}
