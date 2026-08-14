#include "template_format_internal.h"

#include <string.h>

int main(void) {
  TemplateCriticalMetadata metadata =
      template_critical_metadata_format(3, true, 7);
  if (strcmp(metadata.data, "3") != 0 || strcmp(metadata.brand, "7 ") != 0)
    return 1;

  metadata = template_critical_metadata_format(0, true, 11);
  if (strcmp(metadata.data, "-") != 0 || strcmp(metadata.brand, "11 ") != 0)
    return 2;

  metadata = template_critical_metadata_format(3, false, 7);
  if (strcmp(metadata.data, "3") != 0 || strcmp(metadata.brand, "") != 0)
    return 3;
  return 0;
}
