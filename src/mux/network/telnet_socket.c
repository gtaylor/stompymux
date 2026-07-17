/* libuv TCP listeners and client I/O. */

#include "mux/server/platform.h"

#include "mux/server/libuv.h"

#include "libtelnet.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/network/connect_flow.h"
#include "mux/network/telnet_handler.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/diagnostics.h"
#include "mux/server/file_cache.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"

typedef struct DescriptorWrite DescriptorWrite;
struct DescriptorWrite {
  uv_write_t request;
  Descriptor *descriptor;
  size_t size;
  char data[];
};

static uv_tcp_t listener4;
static bool listener4_initialized;
#ifdef IPV6_SUPPORT
static uv_tcp_t listener6;
static bool listener6_initialized;
#endif

static void descriptor_read_alloc(uv_handle_t *handle, size_t suggested_size,
                                  uv_buf_t *buffer);
static void descriptor_read(uv_stream_t *stream, ssize_t read_size,
                            const uv_buf_t *buffer);
static void accept_new_connection(uv_stream_t *server, int status);

static void descriptor_write_complete(uv_write_t *request, int status) {
  DescriptorWrite *write = request->data;
  Descriptor *descriptor = write->descriptor;

  descriptor->pending_writes--;
  descriptor->output_size -= (int)write->size;
  free(write);
  if (status < 0 && !descriptor->is_dead) {
    log_error(LOG_PROBLEMS, "NET", "WRITE", "Write failed on fd %d: %s",
              descriptor->descriptor, uv_strerror(status));
    descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_SOCKDIED);
  }
  descriptor_release(descriptor); // NOLINT(clang-analyzer-unix.Malloc)
}

void descriptor_write_raw(Descriptor *descriptor, const char *buffer,
                          size_t size) {
  DescriptorWrite *write;
  uv_buf_t uv_buffer;
  int status;

  if (size == 0 || descriptor->is_socket_closing)
    return;
  write = malloc(sizeof(*write) + size);
  if (write == nullptr) {
    descriptor->output_lost += (int)size;
    return;
  }
  write->descriptor = descriptor;
  write->size = size;
  memcpy(write->data, buffer, size);
  write->request.data = write;
  uv_buffer = uv_buf_init(write->data, (unsigned int)size);
  descriptor_retain(descriptor);
  descriptor->pending_writes++;
  descriptor->output_size += (int)size;
  status = uv_write(&write->request, (uv_stream_t *)descriptor->socket,
                    &uv_buffer, 1, descriptor_write_complete);
  if (status < 0) {
    descriptor->pending_writes--;
    descriptor->output_size -= (int)size;
    descriptor->output_lost += (int)size;
    free(write);
    descriptor_release(descriptor);
    if (!descriptor->is_dead)
      descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_SOCKDIED);
  }
}

void descriptor_write(Descriptor *descriptor, const char *buffer, size_t size) {
  if (descriptor->telnet != nullptr)
    telnet_send(descriptor->telnet, buffer, size);
  else
    descriptor_write_raw(descriptor, buffer, size);
}

static void listener_closed(uv_handle_t *handle) {
  bool *initialized = handle->data;

  *initialized = false;
}

void mux_release_socket(void) {
  if (listener4_initialized && !uv_is_closing((uv_handle_t *)&listener4))
    uv_close((uv_handle_t *)&listener4, listener_closed);
#ifdef IPV6_SUPPORT
  if (listener6_initialized && !uv_is_closing((uv_handle_t *)&listener6))
    uv_close((uv_handle_t *)&listener6, listener_closed);
#endif
}

int eradicate_broken_fd(int fd) {
  Descriptor *descriptor;
  DescriptorIterator iterator = descriptor_iterator_all();

  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    if (fd == 0 || descriptor->descriptor == fd)
      descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_SOCKDIED);
  }
  return 0;
}

static bool listener_start(uv_tcp_t *listener, bool *initialized, int port,
                           bool ipv6) {
  struct sockaddr_in address4;
  struct sockaddr_in6 address6;
  const struct sockaddr *address;
  int status;

  status = uv_tcp_init(server_lifecycle_loop(), listener);
  if (status < 0)
    goto fail;
  *initialized = true;
  listener->data = initialized;
  if (ipv6) {
    uv_ip6_addr("::", port, &address6);
    address = (const struct sockaddr *)&address6;
  } else {
    uv_ip4_addr("0.0.0.0", port, &address4);
    address = (const struct sockaddr *)&address4;
  }
  status = uv_tcp_bind(listener, address, ipv6 ? UV_TCP_IPV6ONLY : 0);
  if (status < 0)
    goto fail_close;
  status = uv_listen((uv_stream_t *)listener, 25, accept_new_connection);
  if (status < 0)
    goto fail_close;
  return true;

fail_close:
  uv_close((uv_handle_t *)listener, listener_closed);
fail:
  log_error(LOG_ALWAYS, "NET", "LISTEN", "Unable to listen on port %d: %s",
            port, uv_strerror(status));
  return false;
}

bool telnet_socket_listen(int port) {
  if (!listener_start(&listener4, &listener4_initialized, port, false))
    return false;
#ifdef IPV6_SUPPORT
  if (!listener_start(&listener6, &listener6_initialized, port, true)) {
    mux_release_socket();
    return false;
  }
#endif
  return true;
}

static void discarded_connection_closed(uv_handle_t *handle) {
  Descriptor *descriptor = handle->data;

  free(handle);
  free(descriptor);
}

static void discard_connection(Descriptor *descriptor) {
  uv_close((uv_handle_t *)descriptor->socket, discarded_connection_closed);
}

static void descriptor_read_alloc(uv_handle_t *handle, size_t suggested_size,
                                  uv_buf_t *buffer) {
  buffer->base = malloc(LBUF_SIZE);
  buffer->len = buffer->base == nullptr ? 0 : LBUF_SIZE;
}

static void descriptor_read(uv_stream_t *stream, ssize_t read_size,
                            const uv_buf_t *buffer) {
  Descriptor *descriptor = stream->data;

  if (read_size < 0) {
    free(buffer->base);
    if (!descriptor->is_dead)
      descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_SOCKDIED);
    return;
  }
  if (read_size == 0) {
    free(buffer->base);
    return;
  }
  if (descriptor->is_autodark) {
    descriptor->is_autodark = false;
    s_flags(descriptor->player, obj_flags(descriptor->player) & ~DARK);
  }
  descriptor->input_tot += (int)read_size;
  descriptor_retain(descriptor);
  if (is_wizard(descriptor->player) && read_size >= 9 &&
      strncmp("@segfault", buffer->base, 9) == 0) {
    descriptor_queue_string(descriptor,
                            "@segfault failed. (check logfile for reason.)\n");
    *(char *)0xDEADBEEF = '9';
  }
  descriptor_telnet_receive(descriptor, buffer->base, (size_t)read_size);
  free(buffer->base);
  descriptor_release(descriptor); // NOLINT(clang-analyzer-unix.Malloc)
}

static void accept_new_connection(uv_stream_t *server, int status) {
  Descriptor *descriptor;
  struct sockaddr_storage address;
  int address_size = sizeof(address);
  uv_os_fd_t descriptor_fd;
  char address_name[1024];
  char address_port[32];

  if (status < 0)
    return;
  descriptor = calloc(1, sizeof(*descriptor));
  if (descriptor == nullptr)
    return;
  descriptor->socket = malloc(sizeof(*descriptor->socket));
  if (descriptor->socket == nullptr) {
    free(descriptor);
    return;
  }
  if (uv_tcp_init(server_lifecycle_loop(), descriptor->socket) < 0) {
    free(descriptor->socket);
    free(descriptor);
    return;
  }
  descriptor->socket->data = descriptor;
  if (uv_accept(server, (uv_stream_t *)descriptor->socket) < 0) {
    discard_connection(descriptor);
    return;
  }
  uv_tcp_getpeername(descriptor->socket, (struct sockaddr *)&address,
                     &address_size);
  uv_fileno((uv_handle_t *)descriptor->socket, &descriptor_fd);
  descriptor->descriptor = (int)descriptor_fd;
  getnameinfo((struct sockaddr *)&address, (socklen_t)address_size,
              address_name, sizeof(address_name), address_port,
              sizeof(address_port), NI_NUMERICHOST | NI_NUMERICSERV);

  if (site_data_check(&address, address_size, mudstate.access_list) ==
      H_FORBIDDEN) {
    log_error(LOG_NET | LOG_SECURITY, "NET", "SiteData",
              "Connection refused from %s %s.", address_name, address_port);
    fcache_rawdump(descriptor->descriptor, FC_CONN_SITE);
    discard_connection(descriptor);
    return;
  }

  descriptor->connected_at = mudstate.now;
  descriptor->retries_left = mudconf.retry_limit;
  descriptor->timeout = mudconf.idle_timeout;
  descriptor->host_info =
      site_data_check(&address, address_size, mudstate.access_list) |
      site_data_check(&address, address_size, mudstate.suspect_list);
  descriptor->quota = mudconf.cmd_quota_max;
  snprintf(descriptor->addr, sizeof(descriptor->addr), "%s", address_name);
  uv_tcp_nodelay(descriptor->socket, 1);
  if (!descriptor_telnet_initialize(descriptor)) {
    discard_connection(descriptor);
    return;
  }

  if (!descriptor_register(descriptor)) {
    descriptor_telnet_destroy(descriptor);
    discard_connection(descriptor);
    return;
  }
  log_error(LOG_NET, "NET", "CONN", "Connection opened from %s %s.",
            address_name, address_port);
  if (uv_read_start((uv_stream_t *)descriptor->socket, descriptor_read_alloc,
                    descriptor_read) < 0) {
    descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_SOCKDIED);
    return;
  }
  descriptor_welcome(descriptor);
  descriptor_start_connect_flow(descriptor);
}

void flush_sockets(void) {
  /* uv_write submits output directly to the loop; no manual flush is needed. */
}

void close_sockets(int emergency, const char *message) {
  Descriptor *descriptor;
  DescriptorIterator iterator = descriptor_iterator_all();

  mux_release_socket();
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    uv_buf_t buffer;

    if (emergency) {
      buffer = uv_buf_init((char *)(uintptr_t)message,
                           (unsigned int)strlen(message));
      uv_try_write((uv_stream_t *)descriptor->socket, &buffer, 1);
    } else {
      descriptor_queue_string(descriptor, message);
      descriptor_queue_write(descriptor, "\r\n", 2);
    }
    descriptor_shutdown(descriptor, DESCRIPTOR_SHUTDOWN_GOING_DOWN);
    if (emergency)
      descriptor_force_close(descriptor);
  }
}

void emergency_shutdown(void) { close_sockets(1, "Going down - Bye.\n"); }
