#include <stdio.h>
#include <string.h>

#include "btech/core/context_internal.h"
#include "btechstats.h"
#include "btechstats_api.h"
#include "btechstats_internal.h"
#include "character_value_settings.h"

int char_getvaluecode(BtechContext *context, const char *name) {
  (void)context;
  for (int code = 0; code < NUM_CHARVALUES; code++) {
    const CharacterValue *definition = character_value_definition(code);
    if (strcmp(definition->name, name) == 0)
      return code;
  }
  return -1;
}

int main(void) {
  BtechContext first = {0};
  BtechContext second = {0};
  const int RUNNING = char_getvaluecode(&first, "Running");

  btech_character_value_settings_initialize(&first.character_values);
  btech_character_value_settings_initialize(&second.character_values);
  if (RUNNING < 0) {
    fputs("Running skill is missing from the character catalog\n", stderr);
    return 1;
  }
  for (int code = 0; code < NUM_CHARVALUES; code++) {
    const int DEFAULT = character_value_definition(code)->default_xp_threshold;
    if (character_value_xp_threshold(&first, code) != DEFAULT ||
        character_value_xp_threshold(&second, code) != DEFAULT) {
      fprintf(stderr, "default threshold mismatch at character code %d\n",
              code);
      return 1;
    }
  }
  const int DEFAULT_THRESHOLD =
      character_value_definition(RUNNING)->default_xp_threshold;
  if (DEFAULT_THRESHOLD <= 0) {
    fputs("Running skill must have a positive default threshold\n", stderr);
    return 1;
  }

  character_value_xp_threshold_set(&(CharacterValueThreshold){
      .context = &first, .code = RUNNING, .threshold = 0});
  if (character_value_xp_threshold(&first, RUNNING) != 0 ||
      character_value_xp_threshold(&second, RUNNING) != DEFAULT_THRESHOLD ||
      btthreshold_func(&first, "Running") != 0) {
    fputs("threshold override leaked across contexts or missed scripting\n",
          stderr);
    return 1;
  }

  btech_character_value_settings_initialize(&first.character_values);
  if (character_value_xp_threshold(&first, RUNNING) != 0) {
    fputs("repeated initialization reset a runtime override\n", stderr);
    return 1;
  }
  return 0;
}
