#include "template_format_internal.h"

#include <stdio.h>

TemplateCriticalMetadata
template_critical_metadata_format(int data, bool show_brand, int brand) {
  TemplateCriticalMetadata metadata = {0};

  if (data != 0)
    (void)snprintf(metadata.data, sizeof(metadata.data), "%d", data);
  else
    (void)snprintf(metadata.data, sizeof(metadata.data), "-");
  if (show_brand)
    (void)snprintf(metadata.brand, sizeof(metadata.brand), "%d ", brand);
  return metadata;
}
