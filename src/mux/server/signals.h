/* signals.h - Process signal registration and removal interface. */

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

SignalHandlers *signal_handlers_create(uv_loop_t *loop, ServerControl *control);
void signal_handlers_unbind(SignalHandlers *handlers);
void signal_handlers_destroy(SignalHandlers *handlers);
