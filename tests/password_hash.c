/* password_hash.c -- Argon2id password hashing integration test */

#include <string.h>

#include "mux/server/server_state.h"
#include "mux/support/password.h"

ServerConfiguration mudconf;

int main(void) {
  char hash[crypto_pwhash_STRBYTES];

  mudconf.password_hash_opslimit = 3;
  mudconf.password_hash_memlimit = 12 * 1024 * 1024;
  if (!password_initialize() ||
      !password_hash("correct horse battery staple", hash) ||
      !password_verify("correct horse battery staple", hash) ||
      password_verify("incorrect", hash) || strncmp(hash, "$argon2id$", 10)) {
    return 1;
  }
  sodium_memzero(hash, sizeof(hash));
  return 0;
}
