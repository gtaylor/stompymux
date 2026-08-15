#include "mux/support/checked_storage.h"

#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

typedef enum FailureCase {
  FAILURE_NULL_STORAGE,
  FAILURE_ZERO_ELEMENT_SIZE,
  FAILURE_INDEX_OUT_OF_RANGE,
  FAILURE_OFFSET_OVERFLOW,
  FAILURE_REGION_OUT_OF_RANGE,
  FAILURE_NULL_SENTINEL,
  FAILURE_MISSING_SENTINEL,
} FailureCase;

static bool never_sentinel(const void *element) {
  (void)element;
  return false;
}

[[noreturn]] static void trigger_failure(FailureCase failure) {
  unsigned char storage[2] = {1, 2};
  switch (failure) {
  case FAILURE_NULL_STORAGE:
    (void)checked_storage_at(nullptr, 1, 1, 0);
    break;
  case FAILURE_ZERO_ELEMENT_SIZE:
    (void)checked_storage_at(storage, 1, 0, 0);
    break;
  case FAILURE_INDEX_OUT_OF_RANGE:
    (void)checked_storage_at(storage, 2, 1, 2);
    break;
  case FAILURE_OFFSET_OVERFLOW:
    (void)checked_storage_at(storage, SIZE_MAX, SIZE_MAX, 2);
    break;
  case FAILURE_REGION_OUT_OF_RANGE:
    (void)checked_storage_region(storage, sizeof(storage), 1, 2);
    break;
  case FAILURE_NULL_SENTINEL:
    (void)checked_storage_sentinel_count(storage, 1, sizeof(storage), nullptr);
    break;
  case FAILURE_MISSING_SENTINEL:
    (void)checked_storage_sentinel_count(storage, 1, sizeof(storage),
                                         never_sentinel);
    break;
  }
  _exit(2);
}

static int expect_failure(FailureCase failure, const char *message) {
  int descriptors[2];
  if (pipe(descriptors) != 0)
    return 1;
  pid_t child = fork();
  if (child < 0)
    return 1;
  if (child == 0) {
    const struct rlimit CORE_LIMIT = {.rlim_cur = 0, .rlim_max = 0};
    if (setrlimit(RLIMIT_CORE, &CORE_LIMIT) != 0) {
      perror("setrlimit");
      _exit(4);
    }
    (void)close(descriptors[0]);
    if (dup2(descriptors[1], STDERR_FILENO) < 0)
      _exit(3);
    (void)close(descriptors[1]);
    trigger_failure(failure);
  }
  (void)close(descriptors[1]);
  char output[512] = {0};
  size_t output_size = 0;
  while (output_size < sizeof(output) - 1) {
    ssize_t length =
        read(descriptors[0],
             checked_storage_region(output, sizeof(output), output_size,
                                    sizeof(output) - output_size),
             sizeof(output) - output_size - 1);
    if (length > 0) {
      output_size += (size_t)length;
      continue;
    }
    if (length < 0 && errno == EINTR)
      continue;
    break;
  }
  (void)close(descriptors[0]);
  int status = 0;
  pid_t waited;
  do {
    waited = waitpid(child, &status, 0);
  } while (waited < 0 && errno == EINTR);
  if (waited != child || output_size == 0 || !WIFSIGNALED(status) ||
      WTERMSIG(status) != SIGABRT ||
      strstr(output, "checked_storage:") == nullptr ||
      strstr(output, message) == nullptr) {
    return 1;
  }
  return 0;
}


/* The allocation family reports overflow by exiting rather than aborting, so
 * it needs its own expectation of the child's termination mode. */
static int expect_allocation_failure(const char *message) {
  int descriptors[2];
  if (pipe(descriptors) != 0)
    return 1;
  pid_t child = fork();
  if (child < 0)
    return 1;
  if (child == 0) {
    const struct rlimit CORE_LIMIT = {.rlim_cur = 0, .rlim_max = 0};
    if (setrlimit(RLIMIT_CORE, &CORE_LIMIT) != 0) {
      perror("setrlimit");
      _exit(4);
    }
    (void)close(descriptors[0]);
    if (dup2(descriptors[1], STDERR_FILENO) < 0)
      _exit(3);
    (void)close(descriptors[1]);
    (void)checked_storage_allocate_array(SIZE_MAX, 2);
    _exit(2);
  }
  (void)close(descriptors[1]);
  char output[512] = {0};
  size_t output_size = 0;
  while (output_size < sizeof(output) - 1) {
    ssize_t length =
        read(descriptors[0],
             checked_storage_region(output, sizeof(output), output_size,
                                    sizeof(output) - output_size),
             sizeof(output) - output_size - 1);
    if (length > 0) {
      output_size += (size_t)length;
      continue;
    }
    if (length < 0 && errno == EINTR)
      continue;
    break;
  }
  (void)close(descriptors[0]);
  int status = 0;
  pid_t waited;
  do {
    waited = waitpid(child, &status, 0);
  } while (waited < 0 && errno == EINTR);
  if (waited != child || !WIFEXITED(status) ||
      WEXITSTATUS(status) != EXIT_FAILURE ||
      strstr(output, "checked_storage: allocation failed") == nullptr ||
      strstr(output, message) == nullptr) {
    return 1;
  }
  return 0;
}

static bool array_overflow_is_nullable(void) {
  return checked_storage_try_allocate_array(SIZE_MAX, 2) == nullptr &&
         checked_storage_try_allocate_array(SIZE_MAX / 2 + 1, 2) == nullptr;
}

static bool exact_array_capacity_is_allowed(void) {
  void *storage = checked_storage_try_allocate_array(4, sizeof(int));

  if (storage == nullptr)
    return false;
  const bool ZEROED = ((const unsigned char *)storage)[0] == 0;
  free(storage);
  return ZEROED;
}

static bool zero_size_yields_unique_storage(void) {
  void *first = checked_storage_try_allocate(0);
  void *second = checked_storage_allocate(0);
  void *empty_array = checked_storage_try_allocate_array(0, sizeof(int));
  void *zero_element = checked_storage_try_allocate_array(4, 0);
  const bool DISTINCT = first != nullptr && second != nullptr &&
                        empty_array != nullptr && zero_element != nullptr &&
                        first != second;

  free(first);
  free(second);
  free(empty_array);
  free(zero_element);
  return DISTINCT;
}

static bool reallocate_preserves_contents(void) {
  char *storage = checked_storage_try_allocate(4);

  if (storage == nullptr)
    return false;
  storage[0] = 'a';
  char *grown = checked_storage_try_reallocate(storage, 64);
  if (grown == nullptr) {
    free(storage);
    return false;
  }
  const bool PRESERVED = grown[0] == 'a';
  char *empty = checked_storage_try_reallocate(grown, 0);
  const bool NONNULL = empty != nullptr;
  free(empty != nullptr ? empty : grown);
  return PRESERVED && NONNULL;
}

int main(void) {
  int failures = 0;
  if (expect_failure(FAILURE_NULL_STORAGE, "null storage")) {
    fprintf(stderr, "null-storage failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_ZERO_ELEMENT_SIZE, "zero element size")) {
    fprintf(stderr, "zero-element-size failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_INDEX_OUT_OF_RANGE, "index out of bounds")) {
    fprintf(stderr, "out-of-range index failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_OFFSET_OVERFLOW,
                     "offset multiplication overflow")) {
    fprintf(stderr, "offset-overflow failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_REGION_OUT_OF_RANGE, "region out of bounds")) {
    fprintf(stderr, "invalid-region failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_NULL_SENTINEL, "null sentinel callback")) {
    fprintf(stderr, "null-sentinel failure case failed\n");
    ++failures;
  }
  if (expect_failure(FAILURE_MISSING_SENTINEL, "sentinel not found")) {
    fprintf(stderr, "missing-sentinel failure case failed\n");
    ++failures;
  }
  if (expect_allocation_failure("element count overflows")) {
    fprintf(stderr, "array-overflow allocation failure case failed\n");
    ++failures;
  }
  if (!array_overflow_is_nullable()) {
    fprintf(stderr, "try array overflow did not return nullptr\n");
    ++failures;
  }
  if (!exact_array_capacity_is_allowed()) {
    fprintf(stderr, "valid array allocation failed or was not zeroed\n");
    ++failures;
  }
  if (!zero_size_yields_unique_storage()) {
    fprintf(stderr, "zero-sized allocation did not yield unique storage\n");
    ++failures;
  }
  if (!reallocate_preserves_contents()) {
    fprintf(stderr, "reallocation lost contents or returned nullptr\n");
    ++failures;
  }
  return failures != 0;
}
