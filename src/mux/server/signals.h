/* signals.h - Process signal registration and removal interface. */

#pragma once

typedef struct uv_loop_s uv_loop_t;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct SignalHandlers SignalHandlers;
typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerControl ServerControl;

SignalHandlers *signal_handlers_create(uv_loop_t *loop, ServerControl *control);
void signal_handlers_unbind(SignalHandlers *handlers);
void signal_handlers_destroy(SignalHandlers *handlers);
