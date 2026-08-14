#pragma once

#include <stdbool.h>

typedef struct TemplateCriticalMetadata {
  char data[16];
  char brand[16];
} TemplateCriticalMetadata;

TemplateCriticalMetadata
template_critical_metadata_format(int data, bool show_brand, int brand);
