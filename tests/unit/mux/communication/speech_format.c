#include "mux/communication/speech_format.h"

#include <string.h>

int main(void) {
  char buffer[LBUF_SIZE];

  speech_format_say(buffer, "Alex", "hello");
  if (strcmp(buffer, "Alex says \"hello\"") != 0)
    return 1;

  speech_format_pose(buffer, "Alex", "smiles", true);
  if (strcmp(buffer, "Alex smiles") != 0)
    return 2;

  speech_format_pose(buffer, "Alex", "'s grin widens", false);
  if (strcmp(buffer, "Alex's grin widens") != 0)
    return 3;

  return 0;
}
