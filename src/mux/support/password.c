/* password.c - Player password hashing with libsodium's Argon2id API. */

#include "mux/server/server_config.h" // IWYU pragma: keep
#include <core.h>
#include <crypto_pwhash.h>
#include <string.h>

#include "mux/support/alloc.h"
#include "mux/support/password.h"

bool password_initialize(void) { return sodium_init() >= 0; }

bool password_hash(const ServerConfiguration *configuration,
                   const char *password, char hash[crypto_pwhash_STRBYTES]) {
  size_t password_length;

  password_length = strnlen(password, LBUF_SIZE);
  if (password_length >= LBUF_SIZE ||
      configuration->password_hash_opslimit < 1 ||
      configuration->password_hash_memlimit < 1024 * 1024) {
    return 0;
  }
  return crypto_pwhash_str_alg(
             hash, password, (unsigned long long)password_length,
             (unsigned long long)configuration->password_hash_opslimit,
             (size_t)configuration->password_hash_memlimit,
             crypto_pwhash_ALG_ARGON2ID13) == 0;
}

bool password_verify(const char *password, const char *hash) {
  size_t password_length;

  password_length = strnlen(password, LBUF_SIZE);
  if (password_length >= LBUF_SIZE)
    return 0;
  return crypto_pwhash_str_verify(hash, password,
                                  (unsigned long long)password_length) == 0;
}
