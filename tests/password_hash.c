/* password_hash.c -- Argon2id password hashing integration test */

#include <string.h>

#include "mux/server/server_config.h"
#include "mux/support/password.h"

int main(void) {
  ServerConfiguration configuration = {0};
  char hash[crypto_pwhash_STRBYTES];

  configuration.password_hash_opslimit = 3;
  configuration.password_hash_memlimit = 12 * 1024 * 1024;
  if (!password_initialize() ||
      !password_hash(&configuration, "correct horse battery staple", hash) ||
      !password_verify("correct horse battery staple", hash) ||
      password_verify("incorrect", hash) || strncmp(hash, "$argon2id$", 10)) {
    return 1;
  }
  sodium_memzero(hash, sizeof(hash));
  return 0;
}
