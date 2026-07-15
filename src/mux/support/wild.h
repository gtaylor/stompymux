/* wild.h - Wildcard matching interface. */

#pragma once

int wild(char *pattern, char *text, char *arguments[], int argument_count);
int wild_match(char *pattern, char *text);
int quick_wild(char *pattern, char *text);
