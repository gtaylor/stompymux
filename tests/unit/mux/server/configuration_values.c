/* configuration_values.c -- focused configuration value interpreter tests */

#include <stdio.h>
#include <string.h>

#include "mux/server/configuration.h"
#include "mux/server/configuration_internal.h"
#include "mux/server/platform.h"

static char syntax_expectation[128];

void configuration_log_syntax(ConfigurationContext *context [[maybe_unused]],
                              DbRef player [[maybe_unused]],
                              const char *command [[maybe_unused]],
                              const char *expectation,
                              const char *value [[maybe_unused]]) {
  (void)snprintf(syntax_expectation, sizeof(syntax_expectation), "%s",
                 expectation);
}

static int check_limit(const char *text, int expected_result,
                       int expected_value) {
  char mutable_text[16];
  int value = 30;
  ConfigurationCall call = {.value = &value, .text = mutable_text};

  (void)snprintf(mutable_text, sizeof(mutable_text), "%s", text);
  syntax_expectation[0] = '\0';
  if (cf_player_name_length_limit(&call) != expected_result ||
      value != expected_value)
    return -1;
  if (expected_result < 0) {
    char expected[96];

    (void)snprintf(
        expected, sizeof(expected),
        "Expected an integer from 2 through %zu: ", PLAYER_NAME_STORAGE_LIMIT);
    if (strcmp(syntax_expectation, expected) != 0)
      return -1;
  }
  return 0;
}

int main(void) {
  return check_limit("2", 0, 2) < 0 || check_limit("255", 0, 255) < 0 ||
                 check_limit("0", -1, 30) < 0 || check_limit("1", -1, 30) < 0 ||
                 check_limit("256", -1, 30) < 0
             ? 1
             : 0;
}
