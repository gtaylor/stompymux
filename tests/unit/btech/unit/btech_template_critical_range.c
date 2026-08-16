#include "template_load_internal.h"

#include <string.h>

int main(void) {
  char command[] = "CRIT_3-7";
  int first = 0;
  int last = 0;
  if (!template_parse_critical_range(command, &first, &last) || first != 3 ||
      last != 7 || strcmp(command, "CRIT_3") != 0)
    return 1;

  char single[] = "CRIT_4";
  if (!template_parse_critical_range(single, &first, &last) || first != 4 ||
      last != 4)
    return 2;
  return 0;
}
