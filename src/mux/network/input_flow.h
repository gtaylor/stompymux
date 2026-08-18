/** @file
 * Reusable multi-step interactive input engine for descriptors.
 */
#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

constexpr int FLOW_STEP_NAME_SIZE = 64;

typedef enum FlowAction : int {
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
   * message before the transition/teardown; nullptr sends nothing. Borrowed
   * prompt storage must remain valid until the outcome is synchronously
   * applied and the text has been queued or copied.
   */
  const char *prompt;
} FlowOutcome;

/*
 * input == nullptr means the step just became current (flow start, or right
 * after a FLOW_ACTION_GOTO into it) -- return the prompt to display. input !=
 * nullptr means one submitted line while this step was current.
 */
typedef struct FlowStepCall {
  Descriptor *descriptor;
  void *flow_data;
  const char *step;
  const char *input;
} FlowStepCall;

typedef FlowOutcome (*FlowStepFn)(const FlowStepCall *call);

typedef struct FlowStartRequest {
  Descriptor *descriptor;
  const char *initial_step;
  FlowStepFn step;
  void *flow_data;
  void (*destroy)(void *flow_data);
} FlowStartRequest;

/** Starts descriptor flow. @param[in] request Request. */

bool descriptor_flow_start(const FlowStartRequest *request);
/** Executes descriptor flow cancel. @param[in,out] descriptor Network
 * descriptor. */

void descriptor_flow_cancel(Descriptor *descriptor);
/** Destroys descriptor flow. @param[in,out] descriptor Network descriptor. */

void descriptor_flow_destroy(Descriptor *descriptor);
/** Executes descriptor flow handle. @param[in,out] descriptor Network
 * descriptor. @param[in] input Input. */

void descriptor_flow_handle(Descriptor *descriptor, const char *input);

typedef struct FlowMenuItem {
  const char *key;
  const char *label;
} FlowMenuItem;

/** Executes flow render menu. @param[out] buffer Caller-owned output storage.
 * @param[in] buffer_size Size of buffer in bytes. @param[in] header Header.
 * @param[in] items Items. @param[in] item_count Number of item entries. */

void flow_render_menu(char *buffer, size_t buffer_size, const char *header,
                      const FlowMenuItem *items, int item_count);
/** Executes flow match menu. @param[in] items Items. @param[in] item_count
 * Number of item entries. @param[in] input Input. */

int flow_match_menu(const FlowMenuItem *items, int item_count,
                    const char *input);

typedef enum FlowYesNo : int {
  FLOW_YESNO_INVALID = -1,
  FLOW_YESNO_NO = 0,
  FLOW_YESNO_YES = 1,
} FlowYesNo;

/** Executes flow parse yesno. @param[in] input Input. */

FlowYesNo flow_parse_yesno(const char *input);
