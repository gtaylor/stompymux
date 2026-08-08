/* input_flow.c - Reusable multi-step interactive input engine for
 * descriptors. */

#include "mux/network/input_flow.h"

#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"

constexpr int FLOW_MAX_GOTO_CHAIN = 32;

struct InputFlow {
  FlowStepFn step_fn;
  void *flow_data;
  void (*destroy)(void *flow_data);
  char step[FLOW_STEP_NAME_SIZE];
  char *last_prompt;
};

static const FlowMenuItem *flow_menu_item_at(const FlowMenuItem *items,
                                             size_t item_count, size_t index) {
  return checked_storage_at_const(items, item_count, sizeof(*items), index);
}

static unsigned char flow_input_at(const char *input, size_t length,
                                   size_t index) {
  return *(const unsigned char *)checked_storage_at_const(input, length,
                                                          sizeof(char), index);
}

static void flow_send_prompt(Descriptor *d, const char *prompt) {
  descriptor_queue_string(d, tprintf("[bold]%s[reset]", prompt ? prompt : ""));
}

void descriptor_flow_destroy(Descriptor *d) {
  InputFlow *flow = d->flow;

  if (flow == nullptr)
    return;

  d->flow = nullptr;
  if (flow->destroy != nullptr)
    flow->destroy(flow->flow_data);
  free_lbuf(flow->last_prompt);
  free(flow);
}

void descriptor_flow_cancel(Descriptor *d) { descriptor_flow_destroy(d); }

static void flow_apply_outcome(Descriptor *d, FlowOutcome outcome) {
  InputFlow *flow;
  int iterations = 0;

  for (;;) {
    flow = d->flow;
    switch (outcome.action) {
    case FLOW_ACTION_WAIT:
      if (outcome.prompt != nullptr) {
        free_lbuf(flow->last_prompt);
        flow->last_prompt = alloc_lbuf("flow_last_prompt");
        StringCopyTrunc(flow->last_prompt, outcome.prompt, LBUF_SIZE - 1);
      }
      flow_send_prompt(d, flow->last_prompt);
      return;
    case FLOW_ACTION_GOTO:
      if (outcome.prompt != nullptr)
        descriptor_queue_string(d, outcome.prompt);
      if (++iterations > FLOW_MAX_GOTO_CHAIN) {
        log_error(descriptor_log(d), LOG_BUGS, "FLOW", "LOOP",
                  "Interactive flow on descriptor %d exceeded %d GOTO steps "
                  "without input; cancelling.",
                  d->descriptor, FLOW_MAX_GOTO_CHAIN);
        descriptor_flow_destroy(d);
        return;
      }
      StringCopyTrunc(flow->step, outcome.next_step, FLOW_STEP_NAME_SIZE - 1);
      outcome = flow->step_fn(d, flow->flow_data, flow->step, nullptr);
      continue;
    case FLOW_ACTION_DONE:
    case FLOW_ACTION_CANCEL:
      if (outcome.prompt != nullptr)
        descriptor_queue_string(d, outcome.prompt);
      descriptor_flow_destroy(d);
      return;
    }
    return;
  }
}

int descriptor_flow_start(Descriptor *d, const char *initial_step,
                          FlowStepFn step_fn, void *flow_data,
                          void (*destroy)(void *flow_data)) {
  InputFlow *flow;
  FlowOutcome outcome;

  if (d->flow != nullptr)
    return 0;

  flow = malloc(sizeof(InputFlow));
  flow->step_fn = step_fn;
  flow->flow_data = flow_data;
  flow->destroy = destroy;
  flow->last_prompt = nullptr;
  StringCopyTrunc(flow->step, initial_step, FLOW_STEP_NAME_SIZE - 1);
  d->flow = flow;

  outcome = step_fn(d, flow_data, flow->step, nullptr);
  flow_apply_outcome(d, outcome);
  return 1;
}

void descriptor_flow_handle(Descriptor *d, char *input) {
  InputFlow *flow = d->flow;
  FlowOutcome outcome;

  outcome = flow->step_fn(d, flow->flow_data, flow->step, input);
  flow_apply_outcome(d, outcome);
}

void flow_render_menu(char *buffer, size_t buffer_size, const char *header,
                      const FlowMenuItem *items, int item_count) {
  char *bufc = buffer;
  int max = (int)buffer_size - 1;
  int index;

  *buffer = '\0';
  if (header != nullptr && *header != '\0') {
    safe_copy_str(header, buffer, &bufc, max);
    safe_copy_str("\r\n", buffer, &bufc, max);
  }
  for (index = 0; index < item_count; index++) {
    const FlowMenuItem *item =
        flow_menu_item_at(items, (size_t)item_count, (size_t)index);

    safe_copy_str(item->key, buffer, &bufc, max);
    safe_copy_str(") ", buffer, &bufc, max);
    safe_copy_str(item->label, buffer, &bufc, max);
    safe_copy_str("\r\n", buffer, &bufc, max);
  }
  *bufc = '\0';
}

int flow_match_menu(const FlowMenuItem *items, int item_count,
                    const char *input) {
  size_t input_length = strlen(input);
  size_t start = 0;
  size_t end = input_length;
  int index;

  while (start < input_length &&
         isascii(flow_input_at(input, input_length, start)) &&
         (isspace)(flow_input_at(input, input_length, start)))
    start++;
  while (end > start && isascii(flow_input_at(input, input_length, end - 1)) &&
         (isspace)(flow_input_at(input, input_length, end - 1)))
    end--;

  for (index = 0; index < item_count; index++) {
    const FlowMenuItem *item =
        flow_menu_item_at(items, (size_t)item_count, (size_t)index);
    size_t length = end - start;

    if (strlen(item->key) == length &&
        strncasecmp(item->key, checked_string_suffix(input, start), length) ==
            0)
      return index;
  }
  return -1;
}

FlowYesNo flow_parse_yesno(const char *input) {
  size_t length = strlen(input);
  size_t offset = 0;

  while (offset < length && isascii(flow_input_at(input, length, offset)) &&
         (isspace)(flow_input_at(input, length, offset)))
    offset++;
  if (offset == length)
    return FLOW_YESNO_INVALID;
  unsigned char character = flow_input_at(input, length, offset);
  if (character == 'y' || character == 'Y')
    return FLOW_YESNO_YES;
  if (character == 'n' || character == 'N')
    return FLOW_YESNO_NO;
  return FLOW_YESNO_INVALID;
}
