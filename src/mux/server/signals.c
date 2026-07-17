/* signals.c - Process signal registration and server shutdown handlers. */

#include "mux/server/libuv.h"
#include "mux/server/platform.h"
#include <signal.h>

#include "mux/database/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/signals.h"

static void signal_shutdown(uv_signal_t *handle, int signo);
static void signal_PIPE(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_SEGV(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_BUS(int signo, siginfo_t *siginfo, void *ucontext);

struct sigaction saPIPE = {.sa_sigaction = signal_PIPE, .sa_flags = SA_SIGINFO};
struct sigaction saSEGV = {.sa_sigaction = signal_SEGV,
                           .sa_flags =
                               (int)(SA_SIGINFO | SA_RESETHAND | SA_RESTART)};
struct sigaction saBUS = {.sa_sigaction = signal_BUS,
                          .sa_flags =
                              (int)(SA_SIGINFO | SA_RESETHAND | SA_RESTART)};

stack_t sighandler_stack;
stack_t regular_stack;
static uv_signal_t signal_int;
static uv_signal_t signal_term;
static uv_signal_t signal_usr2;
static bool signals_initialized;

constexpr size_t ALT_STACK_SIZE = 0x40000;
constexpr size_t ALT_STACK_ALIGN = 0x1000;

void bind_signals(void) {
  int error_code;
  dprintk("creating alternate signal stack.");
  error_code =
      posix_memalign(&sighandler_stack.ss_sp, ALT_STACK_ALIGN, ALT_STACK_SIZE);
  if (error_code == 0) {
    sighandler_stack.ss_size = ALT_STACK_SIZE;
    sighandler_stack.ss_flags = 0;
    memset(sighandler_stack.ss_sp, 0, ALT_STACK_SIZE);
    dperror(sigaltstack(&sighandler_stack, &regular_stack) < 0);
    dprintk("Current stack at 0x%lx with length 0x%lx and flags 0x%x",
            (unsigned long)regular_stack.ss_sp, regular_stack.ss_size,
            regular_stack.ss_flags);
    dprintk("Signal stack at 0x%lx with length 0x%lx and flags 0x%x",
            (unsigned long)sighandler_stack.ss_sp, sighandler_stack.ss_size,
            sighandler_stack.ss_flags);
    saSEGV.sa_flags |= SA_ONSTACK;
    saBUS.sa_flags |= SA_ONSTACK;
  } else {
    dprintk("posix_memalign failed with %s", strerror(error_code));
    log_error(
        LOG_PROBLEMS, "SIG", "ERR",
        "posix_memalign() failed with error %s, alternate stack not used.",
        strerror(error_code));
    log_error(LOG_PROBLEMS, "SIG", "ERR",
              "running signal_handlers without sigaltstack() will corrupt your "
              "coredumps!");
    sighandler_stack.ss_sp = nullptr;
  }
  dprintk("binding signals.");
  uv_signal_init(server_lifecycle_loop(), &signal_int);
  uv_signal_init(server_lifecycle_loop(), &signal_term);
  uv_signal_init(server_lifecycle_loop(), &signal_usr2);
  uv_signal_start_oneshot(&signal_int, signal_shutdown, SIGINT);
  uv_signal_start_oneshot(&signal_term, signal_shutdown, SIGTERM);
  uv_signal_start_oneshot(&signal_usr2, signal_shutdown, SIGUSR2);
  signals_initialized = true;
  sigaction(SIGSEGV, &saSEGV, nullptr);
  sigaction(SIGBUS, &saBUS, nullptr);
  signal(SIGCHLD, SIG_IGN);
  signal(SIGPIPE, SIG_IGN);
  dprintk("done.");
}

void unbind_signals(void) {
  signal(SIGINT, SIG_DFL);
  signal(SIGTERM, SIG_DFL);
  signal(SIGPIPE, SIG_DFL);
  signal(SIGUSR2, SIG_DFL);
  signal(SIGSEGV, SIG_DFL);
  signal(SIGBUS, SIG_DFL);
  signal(SIGCHLD, SIG_DFL);
  if (sighandler_stack.ss_sp != nullptr) {
    void *temp_ptr;
    sighandler_stack.ss_flags = SS_DISABLE;
    temp_ptr = sighandler_stack.ss_sp;
    sigaltstack(&sighandler_stack, nullptr);
    free(temp_ptr);
    sighandler_stack.ss_sp = nullptr;
  }
}

static void signal_handle_closed(uv_handle_t *handle) {}

void signals_shutdown(void) {
  if (!signals_initialized)
    return;
  uv_signal_stop(&signal_int);
  uv_signal_stop(&signal_term);
  uv_signal_stop(&signal_usr2);
  uv_close((uv_handle_t *)&signal_int, signal_handle_closed);
  uv_close((uv_handle_t *)&signal_term, signal_handle_closed);
  uv_close((uv_handle_t *)&signal_usr2, signal_handle_closed);
  signals_initialized = false;
  unbind_signals();
}

/* do_shutdown()'s message parameter matches the CMDENT dispatch signature
   (char *), which isn't const-correct; these literals are only read. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
static void signal_shutdown(uv_signal_t *handle, int signo) {
  if (signo == SIGINT) {
    dprintk("caught SIGINT");
    do_shutdown(NOTHING, 0, SHUTDN_EXIT,
                (char *)"received SIGINT from kernel.");
  } else if (signo == SIGTERM) {
    dprintk("caught SIGTERM");
    do_shutdown(NOTHING, 0, SHUTDN_EXIT,
                (char *)"received SIGTERM from kernel.");
  } else
    do_shutdown(NOTHING, 0, SHUTDN_EXIT | SHUTDN_KILLED,
                (char *)"received SIGUSR2 from kernel.");
}

static void signal_PIPE(int signo, siginfo_t *siginfo, void *ucontext) {
  dprintk("caught SIGPIPE");
  eradicate_broken_fd(siginfo->si_fd);
}

#pragma clang diagnostic pop

static void signal_SEGV(int signo, siginfo_t *siginfo, void *ucontext) {
  dprintk("caught SIGSEGV");
  mux_release_socket();
  switch (siginfo->si_code) {
  case SEGV_MAPERR:
    raw_broadcast(0,
                  "Game: Invalid access of unmapped memory at %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case SEGV_ACCERR:
    raw_broadcast(0,
                  "Game: Invalid access of protected memory at %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  default:
    raw_broadcast(0, "Game: Unhandled SEGV at %p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  }
  dump_database_internal(DUMP_CRASHED);
  report();
}

static void signal_BUS(int signo, siginfo_t *siginfo, void *ucontext) {
  dprintk("caught SIGBUS");
  mux_release_socket();
  switch (siginfo->si_code) {
  case BUS_ADRALN:
    raw_broadcast(0,
                  "Game: Invalid address alignment accessing %p. "
                  "Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case BUS_ADRERR:
    raw_broadcast(0,
                  "Game: Invalid access of non-existent physical memory at "
                  "%p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  case BUS_OBJERR:
    raw_broadcast(0,
                  "Game: Invalid object-specific hardware error accessing "
                  "%p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  default:
    raw_broadcast(0, "Game: Unhandled SIGBUS at %p. Writing a crash snapshot.",
                  siginfo->si_addr);
    break;
  }
  dump_database_internal(DUMP_CRASHED);
  report();
}
