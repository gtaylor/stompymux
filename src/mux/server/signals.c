/* signals.c - Process signal registration and server shutdown handlers. */

#include "mux/server/platform.h"
#include <signal.h>
#include <uv.h>

#include "mux/objects/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/game.h"
#include "mux/server/server_api.h"
#include "mux/server/server_control.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/signals.h"

static void signal_shutdown(uv_signal_t *handle, int signo);
static void signal_SEGV(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_BUS(int signo, siginfo_t *siginfo, void *ucontext);

/* POSIX sigaction has no user-data parameter, so crash handlers bridge to the
 * single process-owned signal service through this module-scoped selector. */
static SignalHandlers *active_signal_handlers = nullptr;

struct SignalHandlers {
  struct sigaction segv_action;
  struct sigaction bus_action;
  stack_t alternate_stack;
  stack_t regular_stack;
  uv_signal_t signal_int;
  uv_signal_t signal_term;
  uv_signal_t signal_usr2;
  ServerLifecycle *lifecycle;
  DescriptorRegistry *descriptors;
  ServerLog *log;
  CommandContext *command;
  ServerControl *control;
  bool initialized;
  int closing_count;
};

constexpr size_t ALT_STACK_SIZE = 0x40000;
constexpr size_t ALT_STACK_ALIGN = 0x1000;

SignalHandlers *signal_handlers_create(uv_loop_t *loop,
                                       ServerControl *control) {
  SignalHandlers *handlers = calloc(1, sizeof(*handlers));
  int error_code;

  if (handlers == nullptr)
    return nullptr;
  handlers->control = control;
  handlers->lifecycle = control->lifecycle;
  handlers->descriptors = control->descriptors;
  handlers->log = control->log;
  handlers->command = control->command;
  active_signal_handlers = handlers;
  handlers->segv_action = (struct sigaction){
      .sa_sigaction = signal_SEGV,
      .sa_flags = (int)(SA_SIGINFO | SA_RESETHAND | SA_RESTART)};
  handlers->bus_action = (struct sigaction){
      .sa_sigaction = signal_BUS,
      .sa_flags = (int)(SA_SIGINFO | SA_RESETHAND | SA_RESTART)};
  dprintk("creating alternate signal stack.");
  error_code = posix_memalign(&handlers->alternate_stack.ss_sp, ALT_STACK_ALIGN,
                              ALT_STACK_SIZE);
  if (error_code == 0) {
    handlers->alternate_stack.ss_size = ALT_STACK_SIZE;
    handlers->alternate_stack.ss_flags = 0;
    memset(handlers->alternate_stack.ss_sp, 0, ALT_STACK_SIZE);
    dperror(sigaltstack(&handlers->alternate_stack, &handlers->regular_stack) <
            0);
    dprintk("Current stack at 0x%lx with length 0x%lx and flags 0x%x",
            (unsigned long)handlers->regular_stack.ss_sp,
            handlers->regular_stack.ss_size, handlers->regular_stack.ss_flags);
    dprintk("Signal stack at 0x%lx with length 0x%lx and flags 0x%x",
            (unsigned long)handlers->alternate_stack.ss_sp,
            handlers->alternate_stack.ss_size,
            handlers->alternate_stack.ss_flags);
    handlers->segv_action.sa_flags |= SA_ONSTACK;
    handlers->bus_action.sa_flags |= SA_ONSTACK;
  } else {
    dprintk("posix_memalign failed with %s", strerror(error_code));
    log_error(
        handlers->log, LOG_PROBLEMS, "SIG", "ERR",
        "posix_memalign() failed with error %s, alternate stack not used.",
        strerror(error_code));
    log_error(handlers->log, LOG_PROBLEMS, "SIG", "ERR",
              "running signal_handlers without sigaltstack() will corrupt your "
              "coredumps!");
    handlers->alternate_stack.ss_sp = nullptr;
  }
  dprintk("binding signals.");
  uv_signal_init(loop, &handlers->signal_int);
  uv_signal_init(loop, &handlers->signal_term);
  uv_signal_init(loop, &handlers->signal_usr2);
  uv_handle_set_data((uv_handle_t *)&handlers->signal_int, handlers);
  uv_handle_set_data((uv_handle_t *)&handlers->signal_term, handlers);
  uv_handle_set_data((uv_handle_t *)&handlers->signal_usr2, handlers);
  uv_signal_start_oneshot(&handlers->signal_int, signal_shutdown, SIGINT);
  uv_signal_start_oneshot(&handlers->signal_term, signal_shutdown, SIGTERM);
  uv_signal_start_oneshot(&handlers->signal_usr2, signal_shutdown, SIGUSR2);
  handlers->initialized = true;
  sigaction(SIGSEGV, &handlers->segv_action, nullptr);
  sigaction(SIGBUS, &handlers->bus_action, nullptr);
  signal(SIGCHLD, SIG_IGN);
  signal(SIGPIPE, SIG_IGN);
  dprintk("done.");
  return handlers;
}

void signal_handlers_unbind(SignalHandlers *handlers) {
  if (handlers == nullptr)
    return;
  signal(SIGINT, SIG_DFL);
  signal(SIGTERM, SIG_DFL);
  signal(SIGPIPE, SIG_DFL);
  signal(SIGUSR2, SIG_DFL);
  signal(SIGSEGV, SIG_DFL);
  signal(SIGBUS, SIG_DFL);
  signal(SIGCHLD, SIG_DFL);
  if (active_signal_handlers == handlers)
    active_signal_handlers = nullptr;
  if (handlers->alternate_stack.ss_sp != nullptr) {
    void *temp_ptr;
    handlers->alternate_stack.ss_flags = SS_DISABLE;
    temp_ptr = handlers->alternate_stack.ss_sp;
    sigaltstack(&handlers->alternate_stack, nullptr);
    free(temp_ptr);
    handlers->alternate_stack.ss_sp = nullptr;
  }
}

static void signal_handle_closed(uv_handle_t *handle) {
  SignalHandlers *handlers = uv_handle_get_data(handle);

  handlers->closing_count--;
  if (handlers->closing_count == 0)
    free(handlers);
}

void signal_handlers_destroy(SignalHandlers *handlers) {
  if (handlers == nullptr)
    return;
  if (handlers->initialized) {
    uv_signal_stop(&handlers->signal_int);
    uv_signal_stop(&handlers->signal_term);
    uv_signal_stop(&handlers->signal_usr2);
    handlers->closing_count = 3;
    uv_close((uv_handle_t *)&handlers->signal_int, signal_handle_closed);
    uv_close((uv_handle_t *)&handlers->signal_term, signal_handle_closed);
    uv_close((uv_handle_t *)&handlers->signal_usr2, signal_handle_closed);
    handlers->initialized = false;
  }
  signal_handlers_unbind(handlers);
  if (handlers->closing_count == 0)
    free(handlers);
}

static void signal_shutdown(uv_signal_t *handle, int signo) {
  SignalHandlers *handlers = uv_handle_get_data((uv_handle_t *)handle);
  if (signo == SIGINT) {
    dprintk("caught SIGINT");
    server_shutdown(handlers->control, NOTHING, SHUTDN_EXIT,
                    "received SIGINT from kernel.");
  } else if (signo == SIGTERM) {
    dprintk("caught SIGTERM");
    server_shutdown(handlers->control, NOTHING, SHUTDN_EXIT,
                    "received SIGTERM from kernel.");
  } else
    server_shutdown(handlers->control, NOTHING, SHUTDN_EXIT | SHUTDN_KILLED,
                    "received SIGUSR2 from kernel.");
}

static void signal_SEGV(int signo, siginfo_t *siginfo, void *ucontext) {
  SignalHandlers *handlers = active_signal_handlers;

  dprintk("caught SIGSEGV");
  if (handlers == nullptr)
    return;
  server_lifecycle_release_sockets(handlers->lifecycle);
  switch (siginfo->si_code) {
  case SEGV_MAPERR:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Invalid access of unmapped memory at %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case SEGV_ACCERR:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Invalid access of protected memory at %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  default:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Unhandled SEGV at %p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  }
  dump_database_internal(handlers->control, DUMP_CRASHED);
  report(handlers->command);
}

static void signal_BUS(int signo, siginfo_t *siginfo, void *ucontext) {
  SignalHandlers *handlers = active_signal_handlers;

  dprintk("caught SIGBUS");
  if (handlers == nullptr)
    return;
  server_lifecycle_release_sockets(handlers->lifecycle);
  switch (siginfo->si_code) {
  case BUS_ADRALN:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Invalid address alignment accessing %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case BUS_ADRERR:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Invalid access of non-existent physical memory at "
                  "%p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case BUS_OBJERR:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Invalid object-specific hardware error accessing "
                  "%p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  default:
    raw_broadcast(handlers->descriptors, 0,
                  "Game: Unhandled SIGBUS at %p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  }
  dump_database_internal(handlers->control, DUMP_CRASHED);
  report(handlers->command);
}
