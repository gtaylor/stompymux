/* telnet_socket.h - Descriptor socket lifecycle and connection-management
 * interface. */

#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

typedef struct uv_loop_s uv_loop_t;
typedef struct TelnetSockets TelnetSockets;
typedef struct ConnectionRuntime ConnectionRuntime;

TelnetSockets *telnet_sockets_create(uv_loop_t *loop,
                                     ConnectionRuntime *runtime);
void telnet_sockets_destroy(TelnetSockets *sockets);
void telnet_sockets_release(TelnetSockets *sockets);
bool telnet_sockets_listen(TelnetSockets *sockets, int port);

void telnet_sockets_close(TelnetSockets *sockets, bool emergency,
                          const char *message);
int telnet_sockets_eradicate_fd(TelnetSockets *sockets, int fd);
void descriptor_write(Descriptor *d, const char *buffer, size_t size);
void descriptor_write_raw(Descriptor *d, const char *buffer, size_t size);
