#include <stdio.h>
#include <string.h>
#include <strings.h>

#include "libtelnet.h"
#include <zlib.h>

enum {
  telnet_charset_option = 42,
  telnet_charset_request = 1,
  telnet_charset_accepted = 2,
  telnet_charset_rejected = 3,
  telnet_gmcp_option = 201,
};

struct test_context {
  char data[64];
  size_t data_size;
  char sent[128];
  size_t sent_size;
  char terminal_type[16];
  int terminal_width;
  int terminal_height;
  int gmcp_enabled;
  int charset_ascii;
  int charset_request_pending;
};

static const telnet_telopt_t test_options[] = {
    {TELNET_TELOPT_TTYPE, TELNET_WONT, TELNET_DO},
    {TELNET_TELOPT_NAWS, TELNET_WONT, TELNET_DO},
    {TELNET_TELOPT_MSSP, TELNET_WILL, TELNET_DONT},
    {TELNET_TELOPT_COMPRESS2, TELNET_WILL, TELNET_DONT},
    {telnet_charset_option, TELNET_WILL, TELNET_DONT},
    {telnet_gmcp_option, TELNET_WILL, TELNET_DONT},
    {-1, 0, 0},
};

static int append(char *destination, size_t *destination_size,
                  size_t destination_capacity, const char *source,
                  size_t source_size) {
  if (source_size > destination_capacity - *destination_size)
    return 0;
  memcpy(destination + *destination_size, source, source_size);
  *destination_size += source_size;
  return 1;
}

static void test_send_mssp_pair(telnet_t *telnet, const char *name,
                                const char *value) {
  const char variable = TELNET_MSSP_VAR;
  const char mssp_value = TELNET_MSSP_VAL;

  telnet_send(telnet, &variable, sizeof(variable));
  telnet_send(telnet, name, strlen(name));
  telnet_send(telnet, &mssp_value, sizeof(mssp_value));
  telnet_send(telnet, value, strlen(value));
}

static void test_send_mssp(telnet_t *telnet) {
  telnet_begin_sb(telnet, TELNET_TELOPT_MSSP);
  test_send_mssp_pair(telnet, "NAME", "BattleTechMUX");
  telnet_finish_sb(telnet);
}

static int test_charset_is_ascii(const char *buffer, size_t size) {
  static const char ascii[] = "US-ASCII";

  return size == sizeof(ascii) - 1 &&
         strncasecmp(buffer, ascii, sizeof(ascii) - 1) == 0;
}

static void test_send_charset_rejected(telnet_t *telnet) {
  static const char rejected[] = {telnet_charset_rejected};

  telnet_subnegotiation(telnet, telnet_charset_option, rejected,
                        sizeof(rejected));
}

static void test_send_charset_accepted(telnet_t *telnet) {
  static const char accepted[] = {
      telnet_charset_accepted, 'U', 'S', '-', 'A', 'S', 'C', 'I', 'I'};

  telnet_subnegotiation(telnet, telnet_charset_option, accepted,
                        sizeof(accepted));
}

static void test_send_charset_request(telnet_t *telnet,
                                      struct test_context *context) {
  static const char request[] = {
      telnet_charset_request, ';', 'U', 'S', '-', 'A', 'S', 'C', 'I', 'I'};

  if (context->charset_request_pending)
    return;
  context->charset_request_pending = 1;
  telnet_subnegotiation(telnet, telnet_charset_option, request,
                        sizeof(request));
}

static void test_handle_charset(telnet_t *telnet, struct test_context *context,
                                const char *buffer, size_t size) {
  size_t current;
  size_t start;
  char separator;

  if (size == 0)
    return;
  if (buffer[0] == telnet_charset_accepted) {
    context->charset_request_pending = 0;
    context->charset_ascii = test_charset_is_ascii(buffer + 1, size - 1);
    return;
  }
  if (buffer[0] == telnet_charset_rejected) {
    context->charset_request_pending = 0;
    return;
  }
  if (buffer[0] != telnet_charset_request || size < 3 ||
      context->charset_request_pending) {
    test_send_charset_rejected(telnet);
    return;
  }

  separator = buffer[1];
  start = 2;
  for (current = start; current <= size; current++) {
    if (current != size && buffer[current] != separator)
      continue;
    if (test_charset_is_ascii(buffer + start, current - start)) {
      context->charset_ascii = 1;
      test_send_charset_accepted(telnet);
      return;
    }
    start = current + 1;
  }
  test_send_charset_rejected(telnet);
}

static void test_handle_gmcp(telnet_t *telnet, struct test_context *context,
                             const char *buffer, size_t size) {
  static const char core_ping[] = "Core.Ping";
  size_t package_size = sizeof(core_ping) - 1;

  if (context->gmcp_enabled && size >= package_size &&
      memcmp(buffer, core_ping, package_size) == 0 &&
      (size == package_size || buffer[package_size] == ' ')) {
    telnet_subnegotiation(telnet, telnet_gmcp_option, core_ping, package_size);
  }
}

static void test_event_handler(telnet_t *telnet, telnet_event_t *event,
                               void *user_data) {
  struct test_context *context = user_data;
  const unsigned char *buffer;

  switch (event->type) {
  case TELNET_EV_DATA:
    append(context->data, &context->data_size, sizeof(context->data),
           event->data.buffer, event->data.size);
    break;
  case TELNET_EV_SEND:
    append(context->sent, &context->sent_size, sizeof(context->sent),
           event->data.buffer, event->data.size);
    break;
  case TELNET_EV_WILL:
    if (event->neg.telopt == TELNET_TELOPT_TTYPE)
      telnet_ttype_send(telnet);
    break;
  case TELNET_EV_DO:
    if (event->neg.telopt == TELNET_TELOPT_MSSP)
      test_send_mssp(telnet);
    else if (event->neg.telopt == TELNET_TELOPT_COMPRESS2)
      telnet_begin_compress2(telnet);
    else if (event->neg.telopt == telnet_charset_option)
      test_send_charset_request(telnet, context);
    else if (event->neg.telopt == telnet_gmcp_option)
      context->gmcp_enabled = 1;
    break;
  case TELNET_EV_DONT:
    if (event->neg.telopt == telnet_charset_option)
      context->charset_request_pending = 0;
    else if (event->neg.telopt == telnet_gmcp_option)
      context->gmcp_enabled = 0;
    break;
  case TELNET_EV_TTYPE:
    if (event->ttype.cmd == TELNET_TTYPE_IS && event->ttype.name != NULL)
      snprintf(context->terminal_type, sizeof(context->terminal_type), "%s",
               event->ttype.name);
    break;
  case TELNET_EV_SUBNEGOTIATION:
    if (event->sub.telopt == telnet_charset_option) {
      test_handle_charset(telnet, context, event->sub.buffer, event->sub.size);
    } else if (event->sub.telopt == telnet_gmcp_option) {
      test_handle_gmcp(telnet, context, event->sub.buffer, event->sub.size);
    } else if (event->sub.telopt == TELNET_TELOPT_NAWS &&
               event->sub.size == 4) {
      buffer = (const unsigned char *)event->sub.buffer;
      context->terminal_width = (buffer[0] << 8) | buffer[1];
      context->terminal_height = (buffer[2] << 8) | buffer[3];
    }
    break;
  default:
    break;
  }
}

static int expect_bytes(const char *actual, size_t actual_size,
                        const char *expected, size_t expected_size,
                        const char *label) {
  if (actual_size == expected_size &&
      memcmp(actual, expected, expected_size) == 0)
    return 1;
  fprintf(stderr, "%s did not match expected bytes\n", label);
  return 0;
}

static int expect_compressed_data(const char *actual, size_t actual_size,
                                  const char *expected, size_t expected_size) {
  static const char marker[] = {TELNET_IAC, TELNET_SB, TELNET_TELOPT_COMPRESS2,
                                TELNET_IAC, TELNET_SE};
  char output[64];
  z_stream stream;
  int result;

  if (actual_size < sizeof(marker) ||
      memcmp(actual, marker, sizeof(marker)) != 0) {
    fprintf(stderr, "MCCP marker did not match expected bytes\n");
    return 0;
  }

  memset(&stream, 0, sizeof(stream));
  stream.next_in = (Bytef *)(actual + sizeof(marker));
  stream.avail_in = actual_size - sizeof(marker);
  stream.next_out = (Bytef *)output;
  stream.avail_out = sizeof(output);
  result = inflateInit(&stream) == Z_OK &&
                   inflate(&stream, Z_SYNC_FLUSH) == Z_OK &&
                   sizeof(output) - stream.avail_out == expected_size &&
                   memcmp(output, expected, expected_size) == 0
               ? 1
               : 0;
  inflateEnd(&stream);
  if (!result)
    fprintf(stderr, "MCCP data did not decompress to the expected payload\n");
  return result;
}

int main(void) {
  static const char do_options[] = {
      TELNET_IAC, TELNET_DO,   TELNET_TELOPT_TTYPE,
      TELNET_IAC, TELNET_DO,   TELNET_TELOPT_NAWS,
      TELNET_IAC, TELNET_WILL, TELNET_TELOPT_MSSP,
      TELNET_IAC, TELNET_WILL, TELNET_TELOPT_COMPRESS2,
      TELNET_IAC, TELNET_WILL, telnet_charset_option,
      TELNET_IAC, TELNET_WILL, telnet_gmcp_option};
  static const char ttype_will[] = {TELNET_IAC, TELNET_WILL,
                                    TELNET_TELOPT_TTYPE};
  static const char ttype_send[] = {TELNET_IAC,          TELNET_SB,
                                    TELNET_TELOPT_TTYPE, TELNET_TTYPE_SEND,
                                    TELNET_IAC,          TELNET_SE};
  static const char ttype_is[] = {TELNET_IAC,
                                  TELNET_SB,
                                  TELNET_TELOPT_TTYPE,
                                  TELNET_TTYPE_IS,
                                  'x',
                                  't',
                                  'e',
                                  'r',
                                  'm',
                                  TELNET_IAC,
                                  TELNET_SE};
  static const char naws[] = {TELNET_IAC, TELNET_SB,  TELNET_TELOPT_NAWS,
                              0,          120,        0,
                              40,         TELNET_IAC, TELNET_SE};
  static const char escaped_data[] = {'a', TELNET_IAC, 'b'};
  static const char escaped_wire[] = {'a', TELNET_IAC, TELNET_IAC, 'b'};
  static const char mssp_do[] = {TELNET_IAC, TELNET_DO, TELNET_TELOPT_MSSP};
  static const char compress2_do[] = {TELNET_IAC, TELNET_DO,
                                      TELNET_TELOPT_COMPRESS2};
  static const char charset_do[] = {TELNET_IAC, TELNET_DO,
                                    telnet_charset_option};
  static const char charset_request_wire[] = {TELNET_IAC,
                                              TELNET_SB,
                                              telnet_charset_option,
                                              telnet_charset_request,
                                              ';',
                                              'U',
                                              'S',
                                              '-',
                                              'A',
                                              'S',
                                              'C',
                                              'I',
                                              'I',
                                              TELNET_IAC,
                                              TELNET_SE};
  static const char charset_accepted_wire[] = {TELNET_IAC,
                                               TELNET_SB,
                                               telnet_charset_option,
                                               telnet_charset_accepted,
                                               'U',
                                               'S',
                                               '-',
                                               'A',
                                               'S',
                                               'C',
                                               'I',
                                               'I',
                                               TELNET_IAC,
                                               TELNET_SE};
  static const char charset_unsupported_request[] = {TELNET_IAC,
                                                     TELNET_SB,
                                                     telnet_charset_option,
                                                     telnet_charset_request,
                                                     ';',
                                                     'U',
                                                     'T',
                                                     'F',
                                                     '-',
                                                     '8',
                                                     TELNET_IAC,
                                                     TELNET_SE};
  static const char charset_ascii_request[] = {TELNET_IAC,
                                               TELNET_SB,
                                               telnet_charset_option,
                                               telnet_charset_request,
                                               ';',
                                               'U',
                                               'S',
                                               '-',
                                               'A',
                                               'S',
                                               'C',
                                               'I',
                                               'I',
                                               TELNET_IAC,
                                               TELNET_SE};
  static const char charset_rejected_wire[] = {
      TELNET_IAC, TELNET_SB, telnet_charset_option, telnet_charset_rejected,
      TELNET_IAC, TELNET_SE};
  static const char gmcp_do[] = {TELNET_IAC, TELNET_DO, telnet_gmcp_option};
  static const char gmcp_ping[] = {TELNET_IAC, TELNET_SB, telnet_gmcp_option,
                                   'C',        'o',       'r',
                                   'e',        '.',       'P',
                                   'i',        'n',       'g',
                                   TELNET_IAC, TELNET_SE};
  static const char mssp_data[] = {TELNET_IAC,
                                   TELNET_SB,
                                   TELNET_TELOPT_MSSP,
                                   TELNET_MSSP_VAR,
                                   'N',
                                   'A',
                                   'M',
                                   'E',
                                   TELNET_MSSP_VAL,
                                   'B',
                                   'a',
                                   't',
                                   't',
                                   'l',
                                   'e',
                                   'T',
                                   'e',
                                   'c',
                                   'h',
                                   'M',
                                   'U',
                                   'X',
                                   TELNET_IAC,
                                   TELNET_SE};
  struct test_context context;
  telnet_t *telnet;
  int result = 1;

  memset(&context, 0, sizeof(context));
  context.charset_ascii = 1;
  telnet = telnet_init(test_options, test_event_handler, TELNET_FLAG_NVT_EOL,
                       &context);
  if (telnet == NULL) {
    fprintf(stderr, "telnet_init failed\n");
    return 1;
  }

  telnet_negotiate(telnet, TELNET_DO, TELNET_TELOPT_TTYPE);
  telnet_negotiate(telnet, TELNET_DO, TELNET_TELOPT_NAWS);
  telnet_negotiate(telnet, TELNET_WILL, TELNET_TELOPT_MSSP);
  telnet_negotiate(telnet, TELNET_WILL, TELNET_TELOPT_COMPRESS2);
  telnet_negotiate(telnet, TELNET_WILL, telnet_charset_option);
  telnet_negotiate(telnet, TELNET_WILL, telnet_gmcp_option);
  result &= expect_bytes(context.sent, context.sent_size, do_options,
                         sizeof(do_options), "initial negotiation");

  context.sent_size = 0;
  telnet_recv(telnet, ttype_will, 1);
  telnet_recv(telnet, ttype_will + 1, sizeof(ttype_will) - 1);
  result &= expect_bytes(context.sent, context.sent_size, ttype_send,
                         sizeof(ttype_send), "terminal type request");

  context.sent_size = 0;
  telnet_recv(telnet, mssp_do, sizeof(mssp_do));
  result &= expect_bytes(context.sent, context.sent_size, mssp_data,
                         sizeof(mssp_data), "MSSP response");

  context.sent_size = 0;
  telnet_recv(telnet, charset_do, sizeof(charset_do));
  result &= expect_bytes(context.sent, context.sent_size, charset_request_wire,
                         sizeof(charset_request_wire), "CHARSET request");

  context.sent_size = 0;
  telnet_recv(telnet, charset_accepted_wire, sizeof(charset_accepted_wire));
  result &= context.charset_ascii && !context.charset_request_pending;
  telnet_recv(telnet, charset_unsupported_request,
              sizeof(charset_unsupported_request));
  result &= expect_bytes(context.sent, context.sent_size, charset_rejected_wire,
                         sizeof(charset_rejected_wire), "CHARSET rejection");

  context.sent_size = 0;
  telnet_recv(telnet, charset_ascii_request, sizeof(charset_ascii_request));
  result &= expect_bytes(context.sent, context.sent_size, charset_accepted_wire,
                         sizeof(charset_accepted_wire), "CHARSET acceptance");

  context.sent_size = 0;
  telnet_recv(telnet, gmcp_do, sizeof(gmcp_do));
  telnet_recv(telnet, gmcp_ping, sizeof(gmcp_ping));
  result &= expect_bytes(context.sent, context.sent_size, gmcp_ping,
                         sizeof(gmcp_ping), "GMCP ping response");

  context.sent_size = 0;
  telnet_send(telnet, escaped_data, sizeof(escaped_data));
  result &= expect_bytes(context.sent, context.sent_size, escaped_wire,
                         sizeof(escaped_wire), "escaped output");

  context.sent_size = 0;
  telnet_recv(telnet, compress2_do, sizeof(compress2_do));
  telnet_send(telnet, "compressed", 10);
  result &=
      expect_compressed_data(context.sent, context.sent_size, "compressed", 10);

  telnet_recv(telnet, ttype_is, sizeof(ttype_is));
  telnet_recv(telnet, naws, sizeof(naws));
  result &= strcmp(context.terminal_type, "xterm") == 0;
  result &= context.terminal_width == 120 && context.terminal_height == 40;

  telnet_recv(telnet, "look\r\n", 6);
  result &= expect_bytes(context.data, context.data_size, "look\n", 5,
                         "ordinary input");
  telnet_free(telnet);

  if (!result)
    fprintf(stderr, "libtelnet integration test failed\n");
  return result ? 0 : 1;
}
