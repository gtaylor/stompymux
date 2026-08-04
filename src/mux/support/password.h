/* password.h - Player password hashing interfaces. */

#pragma once

#include <crypto_pwhash.h>

#include "mux/server/server_config.h"

typedef struct ServerConfiguration ServerConfiguration;

int password_initialize(void);
int password_hash(const ServerConfiguration *configuration,
                  const char *password, char hash[crypto_pwhash_STRBYTES]);
int password_verify(const char *password, const char *hash);
