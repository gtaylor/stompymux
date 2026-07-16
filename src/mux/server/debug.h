/* debug.h - Debug logging and assertion support macros. */

#pragma once

#include <errno.h>
#include <stdio.h>
#include <sys/time.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#define dassert(x)                                                             \
  do {                                                                         \
    if (!(x)) {                                                                \
      struct timeval assertion_time;                                           \
      struct tm tm;                                                            \
      time_t now;                                                              \
      time(&now);                                                              \
      localtime_r(&now, &tm);                                                  \
      gettimeofday(&assertion_time, nullptr);                                  \
      fprintf(stderr,                                                          \
              "%02d%02d%02d.%08d:%5d %s (%s:%d] failed assertion '%s'\n",      \
              tm.tm_hour, tm.tm_min, tm.tm_sec, (int)assertion_time.tv_usec,   \
              getpid(), __FUNCTION__, __FILE__, __LINE__, #x);                 \
      abort();                                                                 \
    }                                                                          \
  } while (0)

#define dperror(x)                                                             \
  do {                                                                         \
    if (x) {                                                                   \
      struct timeval assertion_time;                                           \
      struct tm tm;                                                            \
      time_t now;                                                              \
      time(&now);                                                              \
      localtime_r(&now, &tm);                                                  \
      gettimeofday(&assertion_time, nullptr);                                  \
      fprintf(                                                                 \
          stderr, "%02d%02d%02d.%08d:%5d %s (%s:%d] '%s' failed with '%s'\n",  \
          tm.tm_hour, tm.tm_min, tm.tm_sec, (int)assertion_time.tv_usec,       \
          getpid(), __FUNCTION__, __FILE__, __LINE__, #x, strerror(errno));    \
    }                                                                          \
  } while (0)

/* DEBUG only error messages */
#ifdef DEBUG
/* debug test:
 * if the first argument `test' evaluates as false,
 * the rest of the arguments are passed to fprintf */
#define dtest(test, ...)                                                       \
  do {                                                                         \
    if (!test) {                                                               \
      fprintf(stderr, "%5d %s (%s:%d)] ", getpid(), __FUNCTION__, __FILE__,    \
              __LINE__);                                                       \
      fprintf(stderr, __VA_ARGS__);                                            \
      fprintf(stderr, "\n");                                                   \
    }                                                                          \
  } while (0)

/* debug print
 * prints arguments */
#define dprintk(...)                                                           \
  do {                                                                         \
    struct timeval __tv;                                                       \
    struct tm __tm;                                                            \
    time_t __now;                                                              \
    time(&__now);                                                              \
    localtime_r(&__now, &__tm);                                                \
    gettimeofday(&__tv, nullptr);                                              \
    fprintf(stderr, "%02d%02d%02d.%08d:%5d %s (%s:%d)] ", __tm.tm_hour,        \
            __tm.tm_min, __tm.tm_sec, (int)__tv.tv_usec, getpid(),             \
            __FUNCTION__, __FILE__, __LINE__);                                 \
    fprintf(stderr, __VA_ARGS__);                                              \
    fprintf(stderr, "\n");                                                     \
  } while (0)

#else
#define dtest(...)
#define dprintk(...)
#endif /* DEBUG */

#define printk(...)                                                            \
  do {                                                                         \
    struct timeval tv;                                                         \
    struct tm tm;                                                              \
    time_t now;                                                                \
    time(&now);                                                                \
    localtime_r(&now, &tm);                                                    \
    gettimeofday(&tv, nullptr);                                                \
    fprintf(stderr, "%02d%02d%02d.%08d:%5d %s (%s:%d)] ", tm.tm_hour,          \
            tm.tm_min, tm.tm_sec, (int)tv.tv_usec, getpid(), __FUNCTION__,     \
            __FILE__, __LINE__);                                               \
    fprintf(stderr, __VA_ARGS__);                                              \
    fprintf(stderr, "\n");                                                     \
  } while (0)

#define IF_FAIL_ERRNO(condition, ...)                                          \
  do {                                                                         \
    if (!(condition)) {                                                        \
      fprintf(stderr, "%s (%s:%d)] ", __FUNCTION__, __FILE__, __LINE__);       \
      fprintf(stderr, __VA_ARGS__);                                            \
      fprintf(stderr, ": ");                                                   \
      perror(nullptr);                                                         \
      abort();                                                                 \
    }                                                                          \
  } while (0)

#define handle_errno(x)                                                        \
  if ((x) < 0)                                                                 \
    do {                                                                       \
      fprintf(stderr, "%s (%s:%d)] %s\n", __FUNCTION__, __FILE__, __LINE__,    \
              strerror(errno));                                                \
      abort();                                                                 \
  } while (0)
