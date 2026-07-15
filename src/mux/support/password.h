/* password.h - Player password hashing interfaces. */

#pragma once

#include <sodium.h>

int password_initialize(void);
int password_hash(const char *password, char hash[crypto_pwhash_STRBYTES]);
int password_verify(const char *password, const char *hash);
