/* input_flow.c - Reusable multi-step interactive input engine for
 * descriptors. */

#include "mux/network/input_flow.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/network/netcommon.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/formatting.h"

constexpr int FLOW_MAX_GOTO_CHAIN = 32;

struct InputFlow {
  FlowStepFn step_fn;
  void *flow_data;
  void (*destroy)(void *flow_data);
  char step[FLOW_STEP_NAME_SIZE];
  char *last_prompt;
};

static void flow_send_prompt(Descriptor *d, const char *prompt) {
  descriptor_queue_string(
      d, tprintf("%s%s%s", ANSI_HILITE, prompt ? prompt : "", ANSI_NORMAL));
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
    safe_copy_str(items[index].key, buffer, &bufc, max);
    safe_copy_str(") ", buffer, &bufc, max);
    safe_copy_str(items[index].label, buffer, &bufc, max);
    safe_copy_str("\r\n", buffer, &bufc, max);
  }
  *bufc = '\0';
}

int flow_match_menu(const FlowMenuItem *items, int item_count,
                    const char *input) {
  const char *start;
  const char *end;
  size_t length;
  int index;

  while (*input && isascii((unsigned char)*input) &&
         isspace((unsigned char)*input))
    input++;
  start = input;
  end = start + strlen(start);
  while (end > start && isascii((unsigned char)end[-1]) &&
         isspace((unsigned char)end[-1]))
    end--;
  length = (size_t)(end - start);

  for (index = 0; index < item_count; index++) {
    if (strlen(items[index].key) == length &&
        strncasecmp(items[index].key, start, length) == 0)
      return index;
  }
  return -1;
}

FlowYesNo flow_parse_yesno(const char *input) {
  while (*input && isascii((unsigned char)*input) &&
         isspace((unsigned char)*input))
    input++;
  if (*input == 'y' || *input == 'Y')
    return FLOW_YESNO_YES;
  if (*input == 'n' || *input == 'N')
    return FLOW_YESNO_NO;
  return FLOW_YESNO_INVALID;
}
