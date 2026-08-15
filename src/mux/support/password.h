/* password.h - Player password hashing interfaces. */

#pragma once

#include <crypto_pwhash.h>

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

bool password_initialize(void);
bool password_hash(const ServerConfiguration *configuration,
                   const char *password, char hash[crypto_pwhash_STRBYTES]);
bool password_verify(const char *password, const char *hash);
