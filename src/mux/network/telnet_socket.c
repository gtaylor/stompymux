/*
 * telnet_socket.c
 */

#include "mux/server/platform.h"

#include <errno.h>
#include <event2/buffer.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#include "libtelnet.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/network/telnet_handler.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/file_cache.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/red_black_tree.h"

#include "mux/server/debug.h"

static struct event *listen_sock_ev;
#ifdef IPV6_SUPPORT
static struct event *listen6_sock_ev;
#endif

int mux_bound_socket = -1;
#ifdef IPV6_SUPPORT
int mux_bound_socket6 = -1;
#endif
int ndescriptors = 0;

Descriptor *descriptor_list = NULL;

static void accept_new_connection(evutil_socket_t sock, short event, void *arg);
static Descriptor *initializesock(int s, struct sockaddr_storage *saddr,
                                  int saddr_len);
static int process_input(Descriptor *d);

void descriptor_write(Descriptor *d, const char *buffer, size_t size) {
  if (d->telnet != NULL)
    telnet_send(d->telnet, buffer, size);
  else
    bufferevent_write(d->sock_buff, buffer, size);
}

static void telnet_socket_clear_strings(Descriptor *d) {
  if (d->output_prefix != NULL) {
    free_lbuf(d->output_prefix);
    d->output_prefix = NULL;
  }
  if (d->output_suffix != NULL) {
    free_lbuf(d->output_suffix);
    d->output_suffix = NULL;
  }
}

static void telnet_socket_free_queues(Descriptor *d) {
  d->input_tail = 0;
  memset(d->input, 0, sizeof(d->input));
}

int descriptor_compare(void *vleft, void *vright, void *token) {
  DbRef left = (DbRef)vleft;
  DbRef right = (DbRef)vright;

  return (left - right);
}

void descriptor_hash_add(Descriptor *d) {
  Descriptor *hdesc;

  descriptor_retain(d);

  hdesc =
      (Descriptor *)red_black_tree_find(mudstate.desctree, (void *)d->player);
  d->hashnext = hdesc;
  red_black_tree_insert(mudstate.desctree, (void *)d->player, d);
}

static void desc_delhash(Descriptor *d) {
  Descriptor *hdesc = NULL;
  char buffer[4096];

  hdesc =
      (Descriptor *)red_black_tree_find(mudstate.desctree, (void *)d->player);
  if (!hdesc) {
    snprintf(buffer, 4096,
             "desc_delhash: unable to find player(%ld)'s descriptors from "
             "hashtable.\n",
             d->player);
    log_text(buffer);
    return;
  }

  if (hdesc == d && hdesc->hashnext) {
    red_black_tree_insert(mudstate.desctree, (void *)d->player, d->hashnext);
    d->hashnext = NULL;
    descriptor_release(d);
    return;
  } else if (hdesc == d) {
    red_black_tree_delete(mudstate.desctree, (void *)d->player);
    descriptor_release(d);
    return;
  }

  while (hdesc->hashnext != NULL) {
    if (hdesc->hashnext == d) {
      hdesc->hashnext = d->hashnext;
      d->hashnext = NULL;
      descriptor_release(d);
      break;
    }
    hdesc = hdesc->hashnext;
  }
  return;
}

void descriptor_retain(Descriptor *d) { d->refcount++; }

void descriptor_release(Descriptor *d) {
  d->refcount--;
  if (d->refcount == 0) {
    dprintk("%p destructing", d);
    telnet_socket_free_queues(d);

    if (d->program_data != NULL) {
      int num = 0;
      Descriptor *dtemp;
      DESC_ITER_PLAYER(d->player, dtemp) num++;

      if (num == 0) {
        int index;

        for (index = 0; index < MAX_GLOBAL_REGS; index++) {
          free_lbuf(d->program_data->wait_regs[index]);
        }
        free(d->program_data);
      }
    }
    telnet_socket_clear_strings(d);
    if (d->descriptor) {
      fsync(d->descriptor);
      event_free(d->sock_ev);
      d->sock_ev = NULL;
      shutdown(d->descriptor, 2);
      close(d->descriptor);
    }
    d->descriptor = 0;
    descriptor_telnet_destroy(d);
    if (d->sock_buff)
      bufferevent_free(d->sock_buff);
    d->sock_buff = NULL;

    if (d->prev) {
      d->prev->next = d->next;
    } else { /* d was the first one! */
      descriptor_list = d->next;
    }

    if (d->next)
      d->next->prev = d->prev;

    ndescriptors--;
    free(d);
  }
}

static int bind_mux_socket(int port) {
  int s, opt;
  struct sockaddr_in server;

  s = socket(AF_INET, SOCK_STREAM, 0);
  if (s < 0) {
    log_perror("NET", "FAIL", NULL, "creating master socket");
    exit(3);
  }
  opt = 1;
  if (setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (char *)&opt, sizeof(opt)) < 0) {
    log_perror("NET", "FAIL", NULL, "setsockopt");
  }

  if (fcntl(s, F_SETFD, FD_CLOEXEC) < 0) {
    log_perror("LOGCACHE", "FAIL", NULL, "fcntl(fd, F_SETFD, FD_CLOEXEC)");
    close(s);
    abort();
  }
  server.sin_family = AF_INET;
  server.sin_addr.s_addr = INADDR_ANY;
  server.sin_port = htons(port);
  if (bind(s, (struct sockaddr *)&server, sizeof(server))) {
    log_perror("NET", "FAIL", NULL, "bind");
    close(s);
    exit(4);
  }
  dprintk("connection socket raised and bound, %d", s);
  listen(s, 25);
  return s;
}

void mux_release_socket() {
  dprintk("releasing mux main socket.");
  if (listen_sock_ev != NULL) {
    event_free(listen_sock_ev);
    listen_sock_ev = NULL;
  }
  if (mux_bound_socket != -1)
    close(mux_bound_socket);
  mux_bound_socket = -1;
#ifdef IPV6_SUPPORT
  if (listen6_sock_ev != NULL) {
    event_free(listen6_sock_ev);
    listen6_sock_ev = NULL;
  }
  if (mux_bound_socket6 != -1)
    close(mux_bound_socket6);
  mux_bound_socket6 = -1;
#endif
}

int eradicate_broken_fd(int fd) {
  struct stat statbuf;
  Descriptor *d, *dtemp;

  DESC_SAFEITER_ALL(d, dtemp) {
    if ((fd && d->descriptor == fd) ||
        (!fd && fstat(d->descriptor, &statbuf) < 0)) {
      /* An invalid player connection... eject, eject, eject. */
      log_error(LOG_PROBLEMS, "ERR", "EBADF",
                "Broken descriptor %d for player #%d", d->descriptor,
                d->player);
      event_del(d->sock_ev);
      close(d->descriptor);
      descriptor_shutdown(d, R_SOCKDIED);
    }
  }
  if (mux_bound_socket != -1 && fstat(mux_bound_socket, &statbuf) < 0) {
    log_error(LOG_PROBLEMS, "ERR", "EBADF",
              "Broken descriptor on our main port.");
    mux_bound_socket = -1;
    return -1;
  }
#ifdef IPV6_SUPPORT
  if (mux_bound_socket6 != -1 && fstat(mux_bound_socket6, &statbuf) < 0) {
    log_error(LOG_PROBLEMS, "ERR", "EBADF",
              "Broken descriptor for our ipv6 port.");
    mux_bound_socket6 = -1;
    return -1;
  }
#endif
  return 0;
}

void accept_client_input(evutil_socket_t fd, short event, void *arg) {
  Descriptor *connection = (Descriptor *)arg;
  if (connection->descriptor != fd)
    return;

  if (connection->flags & DS_AUTODARK) {
    connection->flags &= ~DS_AUTODARK;
    s_Flags(connection->player, Flags(connection->player) & ~DARK);
  }
  descriptor_retain(connection);
  if (!process_input(connection)) {
    eradicate_broken_fd(fd);
  }
  descriptor_release(connection); // NOLINT(clang-analyzer-unix.Malloc)
}

static void bsd_write_callback(struct bufferevent *bufev, void *arg) {}

static void bsd_read_callback(struct bufferevent *bufev, void *arg) {}

static void bsd_error_callback(struct bufferevent *bufev, short whut,
                               void *arg) {
  dprintk("error %d", whut);
}

void telnet_socket_listen(int port) {
  dprintk("starting socket listener on %d.", mux_bound_socket);
#ifdef IPV6_SUPPORT
  dprintk("starting IPv6 socket listener on %d.", mux_bound_socket6);
#endif

  if (mux_bound_socket < 0) {
    mux_bound_socket = bind_mux_socket(port);
  }
  listen_sock_ev = event_new(server_lifecycle_event_base(), mux_bound_socket,
                             EV_READ | EV_PERSIST, accept_new_connection, NULL);
  if (listen_sock_ev == NULL)
    return;
  event_add(listen_sock_ev, NULL);

#ifdef IPV6_SUPPORT
  if (mux_bound_socket6 < 0) {
    mux_bound_socket6 = bind_mux6_socket(port);
  }
  listen6_sock_ev =
      event_new(server_lifecycle_event_base(), mux_bound_socket6,
                EV_READ | EV_PERSIST, accept_new6_connection, NULL);
  if (listen6_sock_ev == NULL)
    return;
  event_add(listen6_sock_ev, NULL);
#endif
}

static void accept_new_connection(evutil_socket_t sock, short event,
                                  void *arg) {
  int newsock, addr_len;
  struct sockaddr_storage addr;
  char addrname[1024];
  char addrport[32];

  addr_len = sizeof(struct sockaddr);

  newsock = accept(sock, (struct sockaddr *)&addr, (unsigned int *)&addr_len);
  if (newsock < 0)
    return;

  getnameinfo((struct sockaddr *)&addr, addr_len, addrname, 1024, addrport, 32,
              NI_NUMERICHOST | NI_NUMERICSERV);

  if (site_data_check(&addr, addr_len, mudstate.access_list) == H_FORBIDDEN) {
    log_error(LOG_NET | LOG_SECURITY, "NET", "SiteData",
              "Connection refused from %s %s.", addrname, addrport);

    fcache_rawdump(newsock, FC_CONN_SITE);
    shutdown(newsock, 2);
    close(newsock);
    errno = 0;
  } else {
    log_error(LOG_NET, "NET", "CONN", "Connection opened from %s %s.", addrname,
              addrport);

    initializesock(newsock, &addr, addr_len);
  }
  return;
}

/*
 * Disconnect reasons that get written to the logfile
 */

static const char *disc_reasons[] = {"Unspecified",
                                     "Quit",
                                     "Inactivity Timeout",
                                     "Booted",
                                     "Remote Close or Net Failure",
                                     "Game Shutdown",
                                     "Login Retry Limit",
                                     "Logins Disabled",
                                     "Too Many Connected Players"};

/*
 * Disconnect reasons that get fed to A_ADISCONNECT via
 * descriptor_announce_disconnect
 */

static const char *disc_messages[] = {"unknown",  "quit",     "timeout",
                                      "boot",     "netdeath", "shutdown",
                                      "badlogin", "nologins", "gamefull"};

void descriptor_shutdown(Descriptor *d, int reason) {
  d->flags |= DS_DEAD;
  if (d->flags & DS_CONNECTED) {
    fcache_dump(d, FC_QUIT);

    log_error(LOG_NET | LOG_LOGIN, "NET", "DISC",
              "[%d/%s] Logout by %s(#%d), <Reason: %s>", d->descriptor, d->addr,
              Name(d->player), d->player, disc_reasons[reason]);

    /*
     * If requested, write an accounting record of the form: * *
     * * * * * Plyr# Flags Cmds ConnTime Loc [Site]
     * <DiscRsn>  * *  * Name
     */

    log_error(
        LOG_ACCOUNTING, "DIS", "ACCT", "%d %s %d %d %d [%s] <%s> %s", d->player,
        decode_flags(GOD, Flags(d->player), Flags2(d->player),
                     Flags3(d->player)),
        d->command_count, mudstate.now - d->connected_at, Location(d->player),
        d->addr, disc_reasons[reason], Name(d->player));

    descriptor_announce_disconnect(d->player, d, disc_messages[reason]);
    desc_delhash(d);
  }
  descriptor_release(d); // NOLINT(clang-analyzer-unix.Malloc)
  /* dprintk("shutdown."); */
}

static void make_nonblocking(int s) {
  long flags = 0;

  if (fcntl(s, F_GETFL, &flags) < 0) {
    log_perror("NET", "FAIL", "make_nonblocking", "fcntl F_GETFL");
  }
  flags |= O_NONBLOCK;
  if (fcntl(s, F_SETFL, flags) < 0) {
    log_perror("NET", "FAIL", "make_nonblocking", "fcntl F_SETFL");
  }
  flags = 1;
  if (setsockopt(s, IPPROTO_TCP, TCP_NODELAY, &flags, sizeof(flags)) < 0) {
    log_perror("NET", "FAIL", "make_nonblocking", "setsockopt NDELAY");
  }
}

static Descriptor *initializesock(int s, struct sockaddr_storage *saddr,
                                  int saddr_len) {
  Descriptor *d;

  ndescriptors++;
  d = malloc(sizeof(Descriptor));
  memset(d, 0, sizeof(Descriptor));

  d->descriptor = s;
  d->flags = 0;
  d->connected_at = mudstate.now;
  d->retries_left = mudconf.retry_limit;
  d->command_count = 0;
  d->timeout = mudconf.idle_timeout;
  d->host_info = site_data_check(saddr, saddr_len, mudstate.access_list) |
                 site_data_check(saddr, saddr_len, mudstate.suspect_list);
  d->player = 0;
  d->addr[0] = '\0';
  d->username[0] = '\0';
  make_nonblocking(s);
  d->output_prefix = NULL;
  d->output_suffix = NULL;
  d->output_size = 0;
  d->output_tot = 0;
  d->output_lost = 0;
  d->input_size = 0;
  d->input_tot = 0;
  d->input_lost = 0;
  memset(d->input, 0, sizeof(d->input));
  d->input_tail = 0;
  d->quota = mudconf.cmd_quota_max;
  d->program_data = NULL;
  d->last_time = 0;
  d->hashnext = NULL;
  getnameinfo((struct sockaddr *)saddr, saddr_len, d->addr, sizeof(d->addr),
              NULL, 0, NI_NUMERICHOST);

  if (descriptor_list)
    descriptor_list->prev = d;
  d->next = descriptor_list;
  d->prev = NULL;
  descriptor_list = d;

  d->sock_buff =
      bufferevent_socket_new(server_lifecycle_event_base(), d->descriptor, 0);
  if (d->sock_buff == NULL) {
    descriptor_list = d->next;
    if (descriptor_list != NULL)
      descriptor_list->prev = NULL;
    ndescriptors--;
    close(d->descriptor);
    free(d);
    return NULL;
  }
  bufferevent_setcb(d->sock_buff, bsd_read_callback, bsd_write_callback,
                    bsd_error_callback, NULL);
  bufferevent_disable(d->sock_buff, EV_READ);
  bufferevent_enable(d->sock_buff, EV_WRITE);
  if (!descriptor_telnet_initialize(d)) {
    bufferevent_free(d->sock_buff);
    d->sock_buff = NULL;
    descriptor_list = d->next;
    if (descriptor_list != NULL)
      descriptor_list->prev = NULL;
    ndescriptors--;
    close(d->descriptor);
    free(d);
    return NULL;
  }
  d->sock_ev = event_new(server_lifecycle_event_base(), d->descriptor,
                         EV_READ | EV_PERSIST, accept_client_input, d);
  if (d->sock_ev == NULL) {
    descriptor_telnet_destroy(d);
    bufferevent_free(d->sock_buff);
    d->sock_buff = NULL;
    descriptor_list = d->next;
    if (descriptor_list != NULL)
      descriptor_list->prev = NULL;
    ndescriptors--;
    close(d->descriptor);
    free(d);
    return NULL;
  }
  event_add(d->sock_ev, NULL);
  descriptor_retain(d);
  descriptor_welcome(d);
  return d;
}

static int process_input(Descriptor *d) {
  char buf[LBUF_SIZE];
  int got;

  if (d->flags & DS_DEAD) {
    dprintk("Bailing on process_input %p %d %s %ld", d, d->descriptor,
            (d->player ? Name(d->player) : ""), d->player);
    return 0;
  }

  memset(buf, 0, sizeof(buf));

  got = read(d->descriptor, buf, (sizeof buf - 1));

  if (got <= 0) {
    if (errno == EINTR)
      return 1;
    else if (errno == EAGAIN)
      return 1;
    else {
      dprintk("error %s (errno %d) read on fd %d descriptor %p %s(%ld)\n",
              strerror(errno), errno, d->descriptor, d,
              (d->player ? Name(d->player) : ""), d->player);
      descriptor_shutdown(d, R_SOCKDIED);
      return 1;
    }
  }

  d->input_tot += got;
  descriptor_retain(d);

  if (Wizard(d->player) && strncmp("@segfault", buf, 9) == 0) {
    descriptor_queue_string(d,
                            "@segfault failed. (check logfile for reason.)\n");
    *(char *)0xDEADBEEF = '9';
  }

  descriptor_telnet_receive(d, buf, (size_t)got);

  descriptor_release(d); // NOLINT(clang-analyzer-unix.Malloc)
  return 1;
}

void flush_sockets() {
  Descriptor *d, *dnext;
  DESC_SAFEITER_ALL(d, dnext) {
    if (d->sock_buff != NULL) {
      struct evbuffer *output = bufferevent_get_output(d->sock_buff);

      if (evbuffer_get_length(output) > 0)
        evbuffer_write(output, d->descriptor);
    }
    fsync(d->descriptor);
  }
}

void close_sockets(int emergency, char *message) {
  Descriptor *d, *dnext;

  DESC_SAFEITER_ALL(d, dnext) {
    if (emergency) {
      if (write(d->descriptor, message, strlen(message)) < 0)
        log_perror("NET", "FAIL", NULL, "write");
      if (shutdown(d->descriptor, 2) < 0)
        log_perror("NET", "FAIL", NULL, "shutdown");
      dprintk("shutting down fd %d", d->descriptor);
      dprintk(
          "output evbuffer contiguous space: %ld, totallen: %ld",
          evbuffer_get_contiguous_space(bufferevent_get_output(d->sock_buff)),
          evbuffer_get_length(bufferevent_get_output(d->sock_buff)));
      fsync(d->descriptor);
      event_base_loop(server_lifecycle_event_base(), EVLOOP_ONCE);
      event_free(d->sock_ev);
      d->sock_ev = NULL;
      bufferevent_free(d->sock_buff);
      d->sock_buff = NULL;
      close(d->descriptor);
    } else {
      descriptor_queue_string(d, message);
      descriptor_queue_write(d, "\r\n", 2);
      descriptor_shutdown(d, R_GOING_DOWN);
    }
  }
  mux_release_socket();
}

void emergency_shutdown(void) {
  close_sockets(1, (char *)"Going down - Bye.\n");
}
