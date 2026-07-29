/* validation.c -- Input validation unit test */

#include <string.h>

#include "mux/server/server_config.h"
#include "mux/support/validation.h"

int string_compare(const ServerConfiguration *configuration, const char *first,
                   const char *second) {
  (void)configuration;
  return strcmp(first, second);
}

int main(void) {
  ServerConfiguration configuration = {0};
  if (!ok_new_player_name(&configuration, "Alice") ||
      !ok_new_player_name(&configuration, "A1") ||
      !ok_new_player_name(&configuration, "a_"))
    return 1;
  if (ok_new_player_name(&configuration, "A") ||
      ok_new_player_name(&configuration, "1a") ||
      ok_new_player_name(&configuration, "_a") ||
      ok_new_player_name(&configuration, "A\033[31m"))
    return 1;
  return 0;
}
