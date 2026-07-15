/* signals.c - Process signal registration and server shutdown handlers. */

#include "mux/server/platform.h"
#include <signal.h>

#include "mux/database/flags.h"
#include "mux/server/debug.h"
#include "mux/server/server_api.h"
#include "mux/server/signals.h"

static void signal_TERM(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_PIPE(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_USR2(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_SEGV(int signo, siginfo_t *siginfo, void *ucontext);
static void signal_BUS(int signo, siginfo_t *siginfo, void *ucontext);

struct sigaction saTERM = {.sa_sigaction = signal_TERM,
                           .sa_flags = SA_SIGINFO | SA_RESETHAND | SA_RESTART};
struct sigaction saPIPE = {.sa_sigaction = signal_PIPE, .sa_flags = SA_SIGINFO};
struct sigaction saUSR2 = {.sa_sigaction = signal_USR2,
                           .sa_flags = SA_SIGINFO | SA_RESETHAND | SA_RESTART};
struct sigaction saSEGV = {.sa_sigaction = signal_SEGV,
                           .sa_flags = SA_SIGINFO | SA_RESETHAND | SA_RESTART};
struct sigaction saBUS = {.sa_sigaction = signal_BUS,
                          .sa_flags = SA_SIGINFO | SA_RESETHAND | SA_RESTART};

stack_t sighandler_stack;
stack_t regular_stack;

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
  dperror(sigaction(SIGINT, &saTERM, nullptr) < 0);
  dperror(sigaction(SIGTERM, &saTERM, nullptr) < 0);
  //	sigaction(SIGPIPE, &saPIPE, NULL);
  sigaction(SIGUSR2, &saUSR2, nullptr);
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

static void signal_TERM(int signo, siginfo_t *siginfo, void *ucontext) {
  if (signo == SIGINT) {
    dprintk("caught SIGINT");
    do_shutdown(NOTHING, 0, SHUTDN_EXIT, "received SIGINT from kernel.");
  } else {
    dprintk("caught SIGTERM");
    do_shutdown(NOTHING, 0, SHUTDN_EXIT, "received SIGTERM from kernel.");
  }
}

static void signal_PIPE(int signo, siginfo_t *siginfo, void *ucontext) {
  dprintk("caught SIGPIPE");
  eradicate_broken_fd(siginfo->si_fd);
}

/* SIGUSR2 is an operator-requested shutdown that preserves a .KILLED dump. */
static void signal_USR2(int signo, siginfo_t *siginfo, void *ucontext) {
  dprintk("caught SIGUSR2");
  do_shutdown(NOTHING, 0, SHUTDN_EXIT | SHUTDN_KILLED,
              "received SIGUSR2 from kernel.");
}

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
