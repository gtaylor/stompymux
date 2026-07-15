/* password.c - Player password hashing with libsodium's Argon2id API. */

#include "mux/server/platform.h"

#include "mux/server/server_state.h"
#include "mux/support/password.h"

int password_initialize(void) { return sodium_init() >= 0; }

int password_hash(const char *password, char hash[crypto_pwhash_STRBYTES]) {
  size_t password_length;

  password_length = strnlen(password, LBUF_SIZE);
  if (password_length >= LBUF_SIZE || mudconf.password_hash_opslimit < 1 ||
      mudconf.password_hash_memlimit < 1024 * 1024) {
    return 0;
  }
  return crypto_pwhash_str_alg(
             hash, password, (unsigned long long)password_length,
             (unsigned long long)mudconf.password_hash_opslimit,
             (size_t)mudconf.password_hash_memlimit,
             crypto_pwhash_ALG_ARGON2ID13) == 0;
}

int password_verify(const char *password, const char *hash) {
  size_t password_length;

  password_length = strnlen(password, LBUF_SIZE);
  if (password_length >= LBUF_SIZE)
    return 0;
  return crypto_pwhash_str_verify(hash, password,
                                  (unsigned long long)password_length) == 0;
}
