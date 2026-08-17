char *checked_mutable_string_suffix(char *text);

char *finish_text(char *cursor) {
  *cursor = 0;
  return checked_mutable_string_suffix(cursor);
}
