/** @file
 * Descriptor socket lifecycle and connection-management interface.
 */
#pragma once

#include <stddef.h>

// IWYU pragma: no_include "mux/network/connection_runtime.h"
// IWYU pragma: no_include "mux/network/descriptor.h"
// IWYU pragma: no_include "uv.h"

typedef struct uv_loop_s UvLoopT;
typedef struct TelnetSockets TelnetSockets;
typedef struct ConnectionRuntime ConnectionRuntime;
typedef struct Descriptor Descriptor;

/** Creates telnet sockets. @param[in] loop Event loop. @param[in] runtime
 * Runtime services. */

TelnetSockets *telnet_sockets_create(UvLoopT *loop, ConnectionRuntime *runtime);
/** Destroys telnet sockets. @param[in,out] sockets Sockets. */

void telnet_sockets_destroy(TelnetSockets *sockets);
/** Executes telnet sockets release. @param[in,out] sockets Sockets. */

void telnet_sockets_release(TelnetSockets *sockets);
/** Executes telnet sockets listen. @param[in,out] sockets Sockets. @param[in]
 * port Port. */

bool telnet_sockets_listen(TelnetSockets *sockets, int port);

/** Closes telnet sockets. @param[in,out] sockets Sockets. @param[in] emergency
 * Emergency. @param[in] message Message. */

void telnet_sockets_close(TelnetSockets *sockets, bool emergency,
                          const char *message);
/** Executes telnet sockets eradicate fd. @param[in,out] sockets Sockets.
 * @param[in] fd Fd. */

bool telnet_sockets_eradicate_fd(TelnetSockets *sockets, int fd);
/** Executes descriptor write. @param[in,out] d D. @param[in] buffer
 * Caller-owned output storage. @param[in] size Storage size in bytes. */

void descriptor_write(Descriptor *d, const char *buffer, size_t size);
/** Writes raw for descriptor. @param[in,out] d D. @param[in] buffer
 * Caller-owned output storage. @param[in] size Storage size in bytes. */

void descriptor_write_raw(Descriptor *d, const char *buffer, size_t size);
