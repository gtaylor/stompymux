#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "script_functions_api.h"

static BtechScriptCall test_call(char *buffer, size_t capacity) {
  return (BtechScriptCall){
      .output = {.buffer = buffer, .cursor = buffer, .capacity = capacity}};
}

static char *buffer_at(char *buffer, size_t capacity, size_t offset) {
  return checked_storage_at(buffer, capacity, sizeof(char), offset);
}

static void check(bool condition) {
  if (!condition)
    abort();
}

static void test_error_status_is_explicit(void) {
  char buffer[32] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 BAD INPUT");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-1 BAD INPUT") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), strlen(buffer)));
}

static void test_sentinel_text_can_be_a_success(void) {
  char buffer[32] = "#-1";
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  call.output.cursor = buffer_at(buffer, sizeof(buffer), strlen(buffer));
  BtechScriptResult result =
      btech_script_result_finish(&call, BTECH_SCRIPT_TEXT);

  check(result.status == BTECH_SCRIPT_OK);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-1") == 0);
}

static void test_error_respects_output_capacity(void) {
  char buffer[6] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 TOO LONG");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(strcmp(buffer, "#-1 T") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), sizeof(buffer) - 1));
}

static void test_error_handles_one_byte_output(void) {
  char buffer[1] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 TOO LONG");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(strcmp(buffer, "") == 0);
  check(call.output.cursor == buffer);
}

static void test_preformatted_error_status_is_explicit(void) {
  char buffer[32] = "#-7 FORMATTED";
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  call.output.cursor = buffer_at(buffer, sizeof(buffer), strlen(buffer));
  BtechScriptResult result = btech_script_error_output(&call);

  check(result.status == BTECH_SCRIPT_ERROR);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-7 FORMATTED") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), strlen(buffer)));
}

int main(void) {
  test_error_status_is_explicit();
  test_sentinel_text_can_be_a_success();
  test_error_respects_output_capacity();
  test_error_handles_one_byte_output();
  test_preformatted_error_status_is_explicit();
  return 0;
}
