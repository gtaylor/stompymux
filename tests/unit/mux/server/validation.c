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
  configuration.player_password_length_limit = 64;
  if (!ok_new_player_name(&configuration, "Alice") ||
      !ok_new_player_name(&configuration, "A1") ||
      !ok_new_player_name(&configuration, "a_"))
    return 1;
  if (ok_new_player_name(&configuration, "A") ||
      ok_new_player_name(&configuration, "1a") ||
      ok_new_player_name(&configuration, "_a") ||
      ok_new_player_name(&configuration, "A\033[31m") ||
      ok_new_player_name(&configuration, "Jos\xc3\xa9"))
    return 1;
  if (!ok_name(&configuration, "Caf\xc3\xa9") ||
      !ok_name(&configuration, "\xf0\x9f\x9a\x80 Launch Bay") ||
      ok_name(&configuration, "bad\xc0\xaf") ||
      ok_name(&configuration, "bad\xc2\x80") || ok_name(&configuration, ""))
    return 1;
  if (!ok_password(&configuration, "p\xc3\xa4ssword") ||
      ok_password(&configuration, "has space") ||
      ok_password(&configuration, "bad\xc0\xaf"))
    return 1;
  return 0;
}
