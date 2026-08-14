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

static char *read_write_buffered(const GmvBufferedBidirectionalCall *call) {
  DescriptorFixture *fixture = (DescriptorFixture *)call->mech;

  if (call->mode)
    fixture->value = atoi(call->value);
  (void)snprintf(call->buffer, LBUF_SIZE, "%d", fixture->value);
  return call->buffer;
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
      {.source_kind = GMV_SOURCE_BUFFERED_BIDIRECTIONAL_CALLBACK,
       .source.buffered_bidirectional_callback = read_write_buffered,
       .type = TYPE_STRFUNC_BD_BUF},
      {.source_kind = GMV_SOURCE_SENTINEL},
  };
  DescriptorFixture fixture = {.value = 42};
  char buffer[LBUF_SIZE];
  char write_value[] = "73";
  bool round_trip;

  descriptors[4].source.buffered_bidirectional_callback(
      &(GmvBufferedBidirectionalCall){.mode = 1,
                                      .mech = (Mech *)&fixture,
                                      .value = write_value,
                                      .buffer = buffer});
  descriptors[4].source.buffered_bidirectional_callback(
      &(GmvBufferedBidirectionalCall){.mech = (Mech *)&fixture,
                                      .buffer = buffer});
  round_trip = fixture.value == 73 && !strcmp(buffer, "73");

  return descriptors[0].source.mech_key == MECH_SCRIPT_MAP_DBREF &&
                 descriptors[1].source.field_offset ==
                     offsetof(DescriptorFixture, value) &&
                 !strcmp(descriptors[2].source.string_callback(0, nullptr),
                         "string") &&
                 !strcmp(
                     descriptors[3].source.buffered_callback(nullptr, buffer),
                     "buffered") &&
                 descriptors[4].type == TYPE_STRFUNC_BD_BUF && round_trip &&
                 descriptors[5].source_kind == GMV_SOURCE_SENTINEL
             ? 0
             : 1;
}
