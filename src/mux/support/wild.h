/* wild.h - Wildcard matching interface. */

#pragma once

int wild(const char *pattern, const char *text, char *arguments[],
         int argument_count);
int wild_match(const char *pattern, const char *text);
int quick_wild(const char *pattern, const char *text);
