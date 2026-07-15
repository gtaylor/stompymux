/* telnet_socket.h - Descriptor socket lifecycle and connection-management
 * interface. */

#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

extern int ndescriptors;

int descriptor_compare(void *vleft, void *vright, void *token);
void descriptor_hash_add(Descriptor *d);
void accept_client_input(evutil_socket_t fd, short event, void *arg);
void flush_sockets(void);
void close_sockets(int emergency, char *message);
void emergency_shutdown(void);
void descriptor_shutdown(Descriptor *descriptor, int reason);
int eradicate_broken_fd(int fd);
void mux_release_socket(void);
void descriptor_retain(Descriptor *d);
void descriptor_release(Descriptor *d);
void descriptor_write(Descriptor *d, const char *buffer, size_t size);
void telnet_socket_listen(int port);
