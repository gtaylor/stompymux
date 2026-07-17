/* validation.c -- Input validation unit test */

#include <string.h>

#include "mux/server/server_state.h"
#include "mux/support/validation.h"

ServerConfiguration mudconf;

char *strip_ansi_r(char *destination, const char *source, size_t size) {
  memcpy(destination, source, size);
  destination[size] = '\0';
  return destination;
}

int string_compare(const char *first, const char *second) {
  return strcmp(first, second);
}

int main(void) {
  if (!ok_new_player_name("Alice") || !ok_new_player_name("A1") ||
      !ok_new_player_name("a_"))
    return 1;
  if (ok_new_player_name("A") || ok_new_player_name("1a") ||
      ok_new_player_name("_a"))
    return 1;
  return 0;
}
