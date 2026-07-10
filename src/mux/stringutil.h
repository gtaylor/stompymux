#pragma once

char *munge_space(char *string);
char *trim_spaces(char *string);
char *grabto(char **str, char targ);
int string_compare(const char *s1, const char *s2);
int string_prefix(const char *string, const char *prefix);
const char *string_match(const char *src, const char *sub);
char *dollar_to_space(const char *str);
char *replace_string(const char *old, const char *new, const char *string);
char *replace_string_inplace(const char *old, const char *new, char *string);
char *skip_space(const char *s);
char *seek_char(const char *s, char c);
int prefix_match(const char *s1, const char *s2);
int minmatch(char *str, char *target, int min);
char *strsave(const char *s);
int safe_copy_str(char *src, char *buff, char **bufp, int max);
int safe_copy_chr(char src, char *buff, char **bufp, int max);
int matches_exit_from_list(char *str, char *pattern);
char *translate_string(const char *str, int type);
char *upcasestr(char *s);
int count_chars(const char *str, char c);
