/* log.h - Server logging and ANSI-stripping interface. */

#pragma once

#include <stddef.h>

#include "mux/database/db.h"

char *strip_ansi_r(char *destination, const char *source, size_t size);
int start_log(const char *primary, const char *secondary);
void end_log(void);
void log_perror(const char *primary, const char *secondary, const char *name,
                const char *error);
void log_error(int key, const char *primary, const char *secondary,
               const char *format, ...) __attribute__((format(printf, 4, 5)));
void log_text(const char *text);
void log_simple(int key, const char *primary, const char *secondary,
                const char *message);
void log_number(int number);
void log_name(DbRef thing);
void log_name_and_loc(DbRef thing);
const char *OBJTYP(DbRef thing);
void log_type_and_name(DbRef thing);
#ifdef ARBITRARY_LOGFILES
int log_to_file(DbRef thing, const char *logfile, const char *message);
#endif
