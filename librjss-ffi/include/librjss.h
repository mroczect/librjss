#ifndef LIBRJSS_FFI_H
#define LIBRJSS_FFI_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define JSS_OK 0

#define JSS_ERR_CONFIG -1

#define JSS_ERR_VALIDATION -2

#define JSS_ERR_NETWORK -3

#define JSS_ERR_HTTP -4

#define JSS_ERR_API -5

#define JSS_ERR_AUTH -6

#define JSS_ERR_CSRF -7

#define JSS_ERR_PERMISSION -8

#define JSS_ERR_SITENAME_MISMATCH -9

#define JSS_ERR_NOT_AUTHENTICATED -10

#define JSS_ERR_RATE_LIMITED -11

#define JSS_ERR_PARSE -12

#define JSS_ERR_EXPIRED -13

#define JSS_ERR_CANCELLED -14

#define JSS_ERR_FILE_OPERATION -15

#define JSS_ERR_INTERNAL -16

#define JSS_ERR_NULL_POINTER -17

#define JSS_ERR_UTF8 -18

#define JSS_ERR_PANIC -19

#define JSS_FLAG_INSECURE_SSL (1 << 0)

#define JSS_FLAG_NO_READONLY_GUARD (1 << 1)

typedef struct JssClient JssClient;

typedef struct JssClientConfig {
  const char *base_url;
  const char *auth_kind;
  const char *principal;
  const char *secret;
  const char *expected_sitename;
  uint32_t flags;
  uint32_t _reserved;
  uint64_t timeout_secs;
  uint32_t max_retries;
  uint32_t _reserved2;
} JssClientConfig;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

const char *jss_last_error(void);

const char *jss_version(void);

struct JssClient *jss_client_new(const struct JssClientConfig *cfg);

void jss_client_free(struct JssClient *c);

int32_t jss_client_trace_id(struct JssClient *c, char **out);

int32_t jss_client_is_authenticated(struct JssClient *c);

int32_t jss_client_authenticate(struct JssClient *c);

int32_t jss_client_logout(struct JssClient *c);

int32_t jss_client_ensure_session(struct JssClient *c);

int32_t jss_client_get(struct JssClient *c, const char *path, char **out_body);

int32_t jss_client_delete(struct JssClient *c, const char *path, char **out_body);

int32_t jss_client_post(struct JssClient *c,
                        const char *path,
                        const char *body_json,
                        char **out_body);

int32_t jss_client_put(struct JssClient *c,
                       const char *path,
                       const char *body_json,
                       char **out_body);

int32_t jss_client_post_form(struct JssClient *c,
                             const char *path,
                             const char *const *keys,
                             const char *const *values,
                             uintptr_t n_pairs,
                             char **out_body);

int32_t jss_client_call_method(struct JssClient *c,
                               const char *method,
                               const char *args_json,
                               char **out_body);

int32_t jss_client_get_doc(struct JssClient *c,
                           const char *doctype,
                           const char *name,
                           char **out_body);

int32_t jss_client_create_doc(struct JssClient *c,
                              const char *doctype,
                              const char *data_json,
                              char **out_body);

int32_t jss_client_update_doc(struct JssClient *c,
                              const char *doctype,
                              const char *name,
                              const char *data_json,
                              char **out_body);

int32_t jss_client_delete_doc(struct JssClient *c,
                              const char *doctype,
                              const char *name,
                              char **out_body);

int32_t jss_client_upload_file(struct JssClient *c,
                               const char *file_name,
                               const uint8_t *content,
                               uintptr_t content_len,
                               const char *doctype,
                               const char *docname,
                               const char *fieldname,
                               char **out_body);

int32_t jss_client_download_file(struct JssClient *c,
                                 const char *file_url,
                                 uint8_t **out_data,
                                 uintptr_t *out_len);

int32_t jss_client_download_pdf_kartu_piutang(struct JssClient *c,
                                              const char *doctype,
                                              const char *name,
                                              const char *format,
                                              int32_t no_letterhead,
                                              uint8_t **out_data,
                                              uintptr_t *out_len);

int32_t jss_client_run_report(struct JssClient *c,
                              const char *report_name,
                              const char *filters_json,
                              char **out_body);

int32_t jss_client_global_search(struct JssClient *c,
                                 const char *query,
                                 uint32_t limit,
                                 const char *doctype,
                                 char **out_body);

int32_t jss_client_boot_sitename(struct JssClient *c, char **out);

int32_t jss_client_boot_user_name(struct JssClient *c, char **out);

int32_t jss_client_boot_user_full_name(struct JssClient *c, char **out);

int32_t jss_client_boot_user_roles(struct JssClient *c, char **out_json);

int32_t jss_client_accessible_doctypes(struct JssClient *c, char **out_json);

int32_t jss_client_is_developer_mode(struct JssClient *c);

int32_t jss_client_is_read_only(struct JssClient *c);

int32_t jss_client_can_read(struct JssClient *c, const char *doctype);

int32_t jss_client_can_write(struct JssClient *c, const char *doctype);

int32_t jss_client_can_create(struct JssClient *c, const char *doctype);

int32_t jss_client_can_submit(struct JssClient *c, const char *doctype);

int32_t jss_client_can_delete(struct JssClient *c, const char *doctype);

void jss_string_free(char *s);

void jss_bytes_free(uint8_t *p, uintptr_t len);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* LIBRJSS_FFI_H */
