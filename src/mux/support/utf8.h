/** @file
 * Strict UTF-8 and printable ASCII helpers.
 */
#pragma once

#include <stddef.h>
#include <stdint.h>

typedef struct Utf8DecodeResult {
  uint32_t codepoint;
  size_t length;
} Utf8DecodeResult;

typedef struct Utf8ValidatorState {
  uint32_t codepoint;
  uint32_t minimum;
  unsigned int remaining;
} Utf8ValidatorState;

/** Executes utf8 decode. @param[in] text Text to process. @param[in] length
 * Text or storage length. @param[out] result Result. */

bool utf8_decode(const char *text, size_t length, Utf8DecodeResult *result);
/** Executes utf8 validate. @param[in] text Text to process. @param[in] length
 * Text or storage length. */

bool utf8_validate(const char *text, size_t length);
/** Executes utf8 validate printable. @param[in] text Text to process.
 * @param[in] length Text or storage length. */

bool utf8_validate_printable(const char *text, size_t length);
/** Executes utf8 is printable ascii. @param[in] text Text to process.
 * @param[in] length Text or storage length. */

bool utf8_is_printable_ascii(const char *text, size_t length);
/** Executes utf8 valid prefix length. @param[in] text Text to process.
 * @param[in] length Text or storage length. */

size_t utf8_valid_prefix_length(const char *text, size_t length);
/** Starts utf8 previous codepoint. @param[in] text Text to process. @param[in]
 * length Text or storage length. */

size_t utf8_previous_codepoint_start(const char *text, size_t length);
/** Copies truncated for utf8. @param[out] destination Destination storage.
 * @param[in] destination_size Size of destination in bytes. @param[in] source
 * Source value. */

size_t utf8_copy_truncated(char *destination, size_t destination_size,
                           const char *source);
/** Initializes utf8 validator. @param[out] state State to inspect or update. */

void utf8_validator_initialize(Utf8ValidatorState *state);
/** Executes utf8 validator feed. @param[in,out] state State to inspect or
 * update. @param[in] text Text to process. @param[in] length Text or storage
 * length. */

bool utf8_validator_feed(Utf8ValidatorState *state, const char *text,
                         size_t length);
/** Executes utf8 validator is complete. @param[in] state State to inspect or
 * update. */

bool utf8_validator_is_complete(const Utf8ValidatorState *state);
/** Executes utf8 sanitize. @param[out] destination Destination storage.
 * @param[in] destination_size Size of destination in bytes. @param[in] source
 * Source value. @param[in] source_length Source length. */

size_t utf8_sanitize(char *destination, size_t destination_size,
                     const char *source, size_t source_length);
