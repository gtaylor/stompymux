/** @file
 * Process signal registration and removal interface.
 */
#pragma once

// IWYU pragma: no_include "mux/commands/command_runtime.h"
// IWYU pragma: no_include "mux/server/server_control.h"
// IWYU pragma: no_include "mux/world/world_context.h"
// IWYU pragma: no_include "uv.h"

typedef struct uv_loop_s uv_loop_t;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct SignalHandlers SignalHandlers;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerControl ServerControl;

/** Creates signal handlers. @param[in] loop Event loop. @param[in] control
 * Control. */

SignalHandlers *signal_handlers_create(uv_loop_t *loop, ServerControl *control);
/** Executes signal handlers unbind. @param[in,out] handlers Handlers. */

void signal_handlers_unbind(SignalHandlers *handlers);
/** Destroys signal handlers. @param[in,out] handlers Handlers. */

void signal_handlers_destroy(SignalHandlers *handlers);
