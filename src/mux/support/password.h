/** @file
 * Player password hashing interfaces.
 */
#pragma once

#include <crypto_pwhash.h>

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

/** Initializes password. */

bool password_initialize(void);
/** Executes password hash. @param[in] configuration Server configuration.
 * @param[in] password Password. @param[in,out] hash Hash. */

bool password_hash(const ServerConfiguration *configuration,
                   const char *password, char hash[crypto_pwhash_STRBYTES]);
/** Executes password verify. @param[in] password Password. @param[in] hash
 * Hash. */

bool password_verify(const char *password, const char *hash);
