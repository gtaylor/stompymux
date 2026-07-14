#pragma once

#include <stddef.h>

#include "descriptor.h"

extern int ndescriptors;

int desc_cmp(void *vleft, void *vright, void *token);
void desc_addhash(DESC *d);
void accept_client_input(evutil_socket_t fd, short event, void *arg);
void flush_sockets(void);
void close_sockets(int emergency, char *message);
void emergency_shutdown(void);
void shutdownsock(DESC *descriptor, int reason);
int eradicate_broken_fd(int fd);
void mux_release_socket(void);
void bind_descriptor(DESC *d);
void release_descriptor(DESC *d);
void telnet_socket_write(DESC *d, const char *buffer, size_t size);
void telnet_socket_listen(int port);
