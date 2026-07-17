/* telnet_socket.h - Descriptor socket lifecycle and connection-management
 * interface. */

#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

void flush_sockets(void);
void close_sockets(int emergency, const char *message);
void emergency_shutdown(void);
int eradicate_broken_fd(int fd);
void mux_release_socket(void);
void descriptor_write(Descriptor *d, const char *buffer, size_t size);
void descriptor_write_raw(Descriptor *d, const char *buffer, size_t size);
bool telnet_socket_listen(int port);
