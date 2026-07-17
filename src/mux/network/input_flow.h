/* input_flow.h - Reusable multi-step interactive input engine for
 * descriptors. */

#pragma once

#include "mux/network/descriptor.h"

constexpr int FLOW_STEP_NAME_SIZE = 64;

typedef enum FlowAction {
  FLOW_ACTION_WAIT,   /* Stay on the current step; (re)print its prompt. */
  FLOW_ACTION_GOTO,   /* Move to a named step in the same flow. */
  FLOW_ACTION_DONE,   /* Flow finished successfully; tear down. */
  FLOW_ACTION_CANCEL, /* Flow aborted; tear down. */
} FlowAction;

typedef struct FlowOutcome {
  FlowAction action;
  char next_step[FLOW_STEP_NAME_SIZE]; /* Used when action == FLOW_ACTION_GOTO.
                                        */
  /*
   * Text to show. For FLOW_ACTION_WAIT, nullptr repeats the last prompt.
   * For FLOW_ACTION_GOTO/DONE/CANCEL, a non-null prompt is sent once as a
   * message before the transition/teardown; nullptr sends nothing.
   */
  const char *prompt;
} FlowOutcome;

/*
 * input == nullptr means the step just became current (flow start, or right
 * after a FLOW_ACTION_GOTO into it) -- return the prompt to display. input !=
 * nullptr means one submitted line while this step was current.
 */
typedef FlowOutcome (*FlowStepFn)(Descriptor *descriptor, void *flow_data,
                                  const char *step, const char *input);

int descriptor_flow_start(Descriptor *descriptor, const char *initial_step,
                          FlowStepFn step_fn, void *flow_data,
                          void (*destroy)(void *flow_data));
void descriptor_flow_cancel(Descriptor *descriptor);
void descriptor_flow_destroy(Descriptor *descriptor);
void descriptor_flow_handle(Descriptor *descriptor, char *input);

typedef struct FlowMenuItem {
  const char *key;
  const char *label;
} FlowMenuItem;

void flow_render_menu(char *buffer, size_t buffer_size, const char *header,
                      const FlowMenuItem *items, int item_count);
int flow_match_menu(const FlowMenuItem *items, int item_count,
                    const char *input);

typedef enum FlowYesNo {
  FLOW_YESNO_INVALID = -1,
  FLOW_YESNO_NO = 0,
  FLOW_YESNO_YES = 1,
} FlowYesNo;

FlowYesNo flow_parse_yesno(const char *input);
