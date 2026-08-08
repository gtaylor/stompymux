/*
 * file_cache.c -- File cache management
 */

#include "mux/network/descriptor.h"
#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/file_cache.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/utf8.h"

typedef struct filecache_hdr FCACHE;
typedef struct filecache_block_hdr FBLKHDR;
typedef struct filecache_block FBLOCK;

struct filecache_hdr {
  const char *filename;
  FBLOCK *fileblock;
  const char *desc;
};

struct filecache_block {
  struct filecache_block_hdr {
    struct filecache_block *nxt;
    int nchars;
  } hdr;
  char data[MBUF_SIZE - sizeof(FBLKHDR)];
};

struct FileCache {
  FCACHE entries[FC_LAST + 1];
  FCACHE connection_entries[100];
  int connection_count;
  const ServerConfiguration *configuration;
  DescriptorRegistry *descriptors;
};

constexpr int MAX_CONN = 100;

static FCACHE *fcache_entry_at(FCACHE *entries, size_t count, size_t index) {
  return checked_storage_at(entries, count, sizeof(*entries), index);
}

static const FCACHE *fcache_entry_at_const(const FCACHE *entries, size_t count,
                                           size_t index) {
  return checked_storage_at_const(entries, count, sizeof(*entries), index);
}

static FBLOCK *fcache_fill(FBLOCK *fp, char ch) {
  FBLOCK *tfp;

  if ((size_t)fp->hdr.nchars >= (MBUF_SIZE - sizeof(FBLKHDR))) {

    /*
     * We filled the current buffer.  Go get a new one.
     */

    tfp = fp;
    fp = (FBLOCK *)alloc_mbuf("fcache_fill");
    fp->hdr.nxt = nullptr;
    fp->hdr.nchars = 0;
    tfp->hdr.nxt = fp;
  }
  *(char *)checked_storage_at(fp->data, sizeof(fp->data), sizeof(char),
                              (size_t)fp->hdr.nchars++) = ch;
  return fp;
}

static int fcache_read(EvaluationContext *evaluation, FBLOCK **cp,
                       const char *filename) {
  int n, nmax, tchars, fd;
  char *buff;
  FBLOCK *fp, *tfp;
  Utf8ValidatorState utf8;
  bool valid_utf8 = true;

  /*
   * Free a prior buffer chain
   */

  fp = *cp;
  while (fp != nullptr) {
    tfp = fp->hdr.nxt;
    free_mbuf(fp);
    fp = tfp;
  }
  *cp = nullptr;

  /*
   * Read the text file into a new chain
   */

  if ((fd = open(filename, O_RDONLY)) == -1) {

    /*
     * Failure: log the event
     */

    log_error(evaluation->log, LOG_PROBLEMS, "FIL", "OPEN",
              "Couldn't open file '%s'.", filename);

    return -1;
  }
  buff = alloc_lbuf("fcache_read.temp");

  /*
   * Set up the initial cache buffer to make things easier
   */

  fp = (FBLOCK *)alloc_mbuf("fcache_read.first");
  fp->hdr.nxt = nullptr;
  fp->hdr.nchars = 0;
  *cp = fp;
  tchars = 0;
  utf8_validator_initialize(&utf8);

  /*
   * Process the file, one lbuf at a time
   */

  nmax = (int)read(fd, buff, LBUF_SIZE);
  while (nmax > 0) {

    if (valid_utf8 && !utf8_validator_feed(&utf8, buff, (size_t)nmax))
      valid_utf8 = false;

    for (n = 0; valid_utf8 && n < nmax; n++) {
      char character = *(const char *)checked_storage_at_const(
          buff, (size_t)nmax, sizeof(char), (size_t)n);
      switch (character) {
      case '\n':
        fp = fcache_fill(fp, '\r');
        fp = fcache_fill(fp, '\n');
        tchars += 2;
        [[fallthrough]];
      case '\0':
      case '\r':
        break;
      default:
        fp = fcache_fill(fp, character);
        tchars++;
      }
    }
    nmax = (int)read(fd, buff, LBUF_SIZE);
  }
  free_lbuf(buff);
  close(fd);

  if (!valid_utf8 || !utf8_validator_is_complete(&utf8)) {
    fp = *cp;
    while (fp != nullptr) {
      tfp = fp->hdr.nxt;
      free_mbuf(fp);
      fp = tfp;
    }
    *cp = nullptr;
    log_error(evaluation->log, LOG_PROBLEMS, "FIL", "UTF8",
              "File '%s' is not valid UTF-8.", filename);
    return -1;
  }

  /*
   * If we didn't read anything in, toss the initial buffer
   */

  if (fp->hdr.nchars == 0) {
    *cp = nullptr;
    free_mbuf(fp);
  }
  return tchars;
}

static void fcache_clear_entry(FCACHE *entry) {
  FBLOCK *block = entry->fileblock;

  while (block != nullptr) {
    FBLOCK *next = block->hdr.nxt;

    free_mbuf(block);
    block = next;
  }
  entry->fileblock = nullptr;
}

static void fcache_read_dir(EvaluationContext *evaluation, const char *dir,
                            FCACHE foo[], int *cnt, int max) {
  DIR *d;
  struct dirent *de;
  char buf[LBUF_SIZE] = {0};

  for (int index = 0; index < *cnt; index++)
    fcache_clear_entry(fcache_entry_at(foo, (size_t)max, (size_t)index));
  memset(foo, 0, sizeof(FCACHE) * (size_t)max);
  if (!(d = opendir(dir)))
    return;
  for (*cnt = 0; *cnt < max;) {
    if (!(de = readdir(d)))
      break;
    if (de->d_name[0] == '.')
      continue;
    if (!strstr(de->d_name, ".txt"))
      continue;
    snprintf(buf, sizeof(buf), "%s/%s", dir, de->d_name);
    fcache_read(evaluation,
                &fcache_entry_at(foo, (size_t)max, (size_t)*cnt)->fileblock,
                buf);
    (*cnt)++;
  }
  closedir(d);
}

void fcache_rawdump(const FileCache *cache, int fd, int num) {
  int cnt, remaining;
  FBLOCK *fp;

  if ((num < 0) || (num > FC_LAST))
    return;
  fp = fcache_entry_at_const(cache->entries, FC_LAST + 1, (size_t)num)
           ->fileblock;

  while (fp != nullptr) {
    remaining = fp->hdr.nchars;
    size_t offset = 0;
    while (remaining > 0) {
      cnt = (int)write(fd,
                       checked_storage_region_const(fp->data, sizeof(fp->data),
                                                    offset, (size_t)remaining),
                       (size_t)remaining);
      if (cnt < 0)
        return;
      remaining -= cnt;
      offset += (size_t)cnt;
    }
    fp = fp->hdr.nxt;
  }
  return;
}

static void fcache_dumpbase(Descriptor *d, const FCACHE fc[], size_t count,
                            int num) {
  FBLOCK *fp;

  fp = fcache_entry_at_const(fc, count, (size_t)num)->fileblock;

  while (fp != nullptr) {
    descriptor_queue_write(d, fp->data, fp->hdr.nchars);
    fp = fp->hdr.nxt;
  }
}

void fcache_dump(const FileCache *cache, Descriptor *d, int num) {
  if ((num < 0) || (num > FC_LAST))
    return;
  fcache_dumpbase(d, cache->entries, FC_LAST + 1, num);
}

void fcache_dump_conn(const FileCache *cache, Descriptor *d, int num) {
  if (num < 0 || num >= cache->connection_count)
    return;
  fcache_dumpbase(d, cache->connection_entries, MAX_CONN, num);
}

void fcache_send(FileCache *cache, DbRef player, int num) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_player(cache->descriptors, player);

  while ((d = descriptor_iterator_next(&iterator)) != nullptr)
    fcache_dump(cache, d, num);
}

void fcache_load(EvaluationContext *evaluation, FileCache *cache,
                 DbRef player) {
  char *buff, *bufc;
  char sbuf[SBUF_SIZE];
  int i;

  buff = bufc = alloc_lbuf("fcache_load.lbuf");
  for (int index = 0; index <= FC_LAST; index++) {
    FCACHE *fp = fcache_entry_at(cache->entries, FC_LAST + 1, (size_t)index);
    i = fcache_read(evaluation, &fp->fileblock, fp->filename);
    if (player != NOTHING) {
      snprintf(sbuf, SBUF_SIZE, "%d", i);
      if (index == 0)
        safe_str("File sizes: ", buff, &bufc);
      else
        safe_str("  ", buff, &bufc);
      safe_str(fp->desc, buff, &bufc);
      safe_str("...", buff, &bufc);
      safe_str(sbuf, buff, &bufc);
    }
  }
  *bufc = '\0';
  if (*cache->configuration->conn_dir)
    fcache_read_dir(evaluation, cache->configuration->conn_dir,
                    cache->connection_entries, &cache->connection_count,
                    MAX_CONN);
  if (player != NOTHING) {
    notify_checked(evaluation, player, player, buff, MSG_ME_ALL | MSG_F_DOWN);
  }
  free_lbuf(buff);
}

FileCache *file_cache_create(EvaluationContext *evaluation,
                             const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors) {
  FileCache *cache = calloc(1, sizeof(*cache));

  if (cache == nullptr)
    return nullptr;
  cache->configuration = configuration;
  cache->descriptors = descriptors;
  *fcache_entry_at(cache->entries, FC_LAST + 1, FC_CONN) =
      (FCACHE){configuration->conn_file, nullptr, "Conn"};
  *fcache_entry_at(cache->entries, FC_LAST + 1, FC_CONN_SITE) =
      (FCACHE){configuration->site_file, nullptr, "Conn/Badsite"};
  *fcache_entry_at(cache->entries, FC_LAST + 1, FC_CONN_DOWN) =
      (FCACHE){configuration->down_file, nullptr, "Conn/Down"};
  *fcache_entry_at(cache->entries, FC_LAST + 1, FC_CONN_FULL) =
      (FCACHE){configuration->full_file, nullptr, "Conn/Full"};
  *fcache_entry_at(cache->entries, FC_LAST + 1, FC_QUIT) =
      (FCACHE){configuration->quit_file, nullptr, "Quit"};
  fcache_load(evaluation, cache, NOTHING);
  return cache;
}

void file_cache_destroy(FileCache *cache) {
  if (cache == nullptr)
    return;
  for (int index = 0; index <= FC_LAST; index++)
    fcache_clear_entry(
        fcache_entry_at(cache->entries, FC_LAST + 1, (size_t)index));
  for (int index = 0; index < cache->connection_count; index++)
    fcache_clear_entry(
        fcache_entry_at(cache->connection_entries, MAX_CONN, (size_t)index));
  free(cache);
}

int file_cache_connection_count(const FileCache *cache) {
  return cache->connection_count;
}
