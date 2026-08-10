#include "values_internal.h"

typedef struct DescriptorFixture {
  int value;
} DescriptorFixture;

static char *read_string(int mode, Mech *mech) {
  (void)mode;
  (void)mech;
  static char value[] = "string";

  return value;
}

static char *read_buffered(Mech *mech, char *buffer) {
  (void)mech;
  strcpy(buffer, "buffered");
  return buffer;
}

int main(void) {
  GMV descriptors[] = {
      {.source_kind = GMV_SOURCE_MECH_KEY,
       .source.mech_key = MECH_SCRIPT_MAP_DBREF},
      {.source_kind = GMV_SOURCE_FIELD_OFFSET,
       .source.field_offset = offsetof(DescriptorFixture, value)},
      {.source_kind = GMV_SOURCE_STRING_CALLBACK,
       .source.string_callback = read_string},
      {.source_kind = GMV_SOURCE_BUFFERED_CALLBACK,
       .source.buffered_callback = read_buffered},
      {.source_kind = GMV_SOURCE_SENTINEL},
  };
  DescriptorFixture fixture = {.value = 42};
  char buffer[32];

  return descriptors[0].source.mech_key == MECH_SCRIPT_MAP_DBREF &&
                 descriptors[1].source.field_offset ==
                     offsetof(DescriptorFixture, value) &&
                 fixture.value == 42 &&
                 !strcmp(descriptors[2].source.string_callback(0, nullptr),
                         "string") &&
                 !strcmp(
                     descriptors[3].source.buffered_callback(nullptr, buffer),
                     "buffered") &&
                 descriptors[4].source_kind == GMV_SOURCE_SENTINEL
             ? 0
             : 1;
}
