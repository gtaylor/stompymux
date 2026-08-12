/* wild.h - Wildcard matching interface. */

#pragma once

int wild(const char *tstr, const char *dstr, char *args[], int nargs);
int wild_match(const char *tstr, const char *dstr);
int quick_wild(const char *tstr, const char *dstr);
