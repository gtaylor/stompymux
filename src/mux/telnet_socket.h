#pragma once

#include <stddef.h>

#include "interface.h"

extern int ndescriptors;

int desc_cmp(void *vleft, void *vright, void *token);
void desc_addhash(DESC *d);
void accept_client_input(int fd, short event, void *arg);
void flush_sockets(void);
void close_sockets(int emergency, char *message);
int eradicate_broken_fd(int fd);
void mux_release_socket(void);
void bind_descriptor(DESC *d);
void release_descriptor(DESC *d);
void bsd_write_callback(struct bufferevent *bufev, void *arg);
void bsd_read_callback(struct bufferevent *bufev, void *arg);
void bsd_error_callback(struct bufferevent *bufev, short whut, void *arg);
void telnet_socket_write(DESC *d, const char *buffer, size_t size);
void telnet_socket_listen(int port);
