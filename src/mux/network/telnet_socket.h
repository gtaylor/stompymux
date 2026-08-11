/* telnet_socket.h - Descriptor socket lifecycle and connection-management
 * interface. */

#pragma once

#include <stddef.h>

// IWYU pragma: no_include "mux/network/connection_runtime.h"
// IWYU pragma: no_include "mux/network/descriptor.h"
// IWYU pragma: no_include "uv.h"

typedef struct uv_loop_s UvLoopT;
typedef struct TelnetSockets TelnetSockets;
typedef struct ConnectionRuntime ConnectionRuntime;
typedef struct Descriptor Descriptor;

TelnetSockets *telnet_sockets_create(UvLoopT *loop, ConnectionRuntime *runtime);
void telnet_sockets_destroy(TelnetSockets *sockets);
void telnet_sockets_release(TelnetSockets *sockets);
bool telnet_sockets_listen(TelnetSockets *sockets, int port);

void telnet_sockets_close(TelnetSockets *sockets, bool emergency,
                          const char *message);
int telnet_sockets_eradicate_fd(TelnetSockets *sockets, int fd);
void descriptor_write(Descriptor *d, const char *buffer, size_t size);
void descriptor_write_raw(Descriptor *d, const char *buffer, size_t size);
