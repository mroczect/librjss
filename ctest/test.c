#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef void *(*ConfigNew)(void);
typedef void (*ConfigFree)(void *);
typedef void (*ConfigSetCookieName)(void *, const char *);
typedef void (*ConfigSetSessionLifetime)(void *, int64_t);
typedef void *(*AuthManagerNewWithMemoryStore)(
    void *, void *,
    int32_t (*)(void *, const char *, const char *, char **, char **,
                int32_t *),
    int32_t (*)(void *, const char *, char **, char **, int32_t *));
typedef void (*AuthManagerFree)(void *);
typedef void *(*Login)(void *, const char *, const char *, char **);
typedef void *(*Logout)(void *, const char *, char **);
typedef int (*CreateSession)(void *, const char *, const char *, char **,
                             void **, char **);
typedef void *(*ValidateSession)(void *, const char *, char **);
typedef uint16_t (*ResponseStatus)(void *);
typedef char *(*ResponseBody)(void *);
typedef void (*ResponseFree)(void *);
typedef char *(*SessionInfoUserId)(void *);
typedef char *(*SessionInfoData)(void *);
typedef void (*SessionInfoFree)(void *);
typedef char *(*ErrorMessage)(int);
typedef void (*FreeString)(char *);

static int32_t auth_cb(void *user_data, const char *username,
                       const char *password, char **out_user_id,
                       char **out_extra_json, int32_t *out_err) {
  if (strcmp(username, "admin") == 0 && strcmp(password, "secret") == 0) {
    *out_user_id = strdup("admin");
    *out_extra_json = strdup("{\"role\":\"admin\"}");
    *out_err = 0;
    return 0;
  }
  *out_err = 2;
  return -1;
}

static int32_t get_user_cb(void *user_data, const char *user_id,
                           char **out_user_id, char **out_extra_json,
                           int32_t *out_err) {
  if (strcmp(user_id, "admin") == 0) {
    *out_user_id = strdup("admin");
    *out_extra_json = strdup("{\"role\":\"admin\"}");
    *out_err = 0;
    return 0;
  }
  *out_user_id = NULL;
  *out_extra_json = NULL;
  *out_err = 0;
  return 0;
}

int main(void) {
  void *lib = dlopen("../target/release/liblibrjss.so", RTLD_NOW);
  if (!lib) {
    fprintf(stderr, "dlopen: %s\n", dlerror());
    return 1;
  }

  ConfigNew cfg_new = (ConfigNew)dlsym(lib, "rjss_config_new_default");
  ConfigFree cfg_free = (ConfigFree)dlsym(lib, "rjss_config_free");
  ConfigSetCookieName cfg_cookie =
      (ConfigSetCookieName)dlsym(lib, "rjss_config_set_cookie_name");
  ConfigSetSessionLifetime cfg_lt =
      (ConfigSetSessionLifetime)dlsym(lib, "rjss_config_set_session_lifetime");
  AuthManagerNewWithMemoryStore mgr_new = (AuthManagerNewWithMemoryStore)dlsym(
      lib, "rjss_auth_manager_new_with_memory_store");
  AuthManagerFree mgr_free =
      (AuthManagerFree)dlsym(lib, "rjss_auth_manager_free");
  Login login = (Login)dlsym(lib, "rjss_login");
  Logout logout = (Logout)dlsym(lib, "rjss_logout");
  CreateSession create_sess = (CreateSession)dlsym(lib, "rjss_create_session");
  ValidateSession validate =
      (ValidateSession)dlsym(lib, "rjss_validate_session");
  ResponseStatus resp_status =
      (ResponseStatus)dlsym(lib, "rjss_response_status");
  ResponseBody resp_body = (ResponseBody)dlsym(lib, "rjss_response_body");
  ResponseFree resp_free = (ResponseFree)dlsym(lib, "rjss_response_free");
  SessionInfoUserId sess_uid =
      (SessionInfoUserId)dlsym(lib, "rjss_session_info_user_id");
  SessionInfoData sess_data =
      (SessionInfoData)dlsym(lib, "rjss_session_info_data");
  SessionInfoFree sess_free =
      (SessionInfoFree)dlsym(lib, "rjss_session_info_free");
  ErrorMessage err_msg = (ErrorMessage)dlsym(lib, "rjss_error_message");
  FreeString free_str = (FreeString)dlsym(lib, "rjss_free_string");

  if (!cfg_new || !cfg_free || !cfg_cookie || !cfg_lt || !mgr_new ||
      !mgr_free || !login || !logout || !create_sess || !validate ||
      !resp_status || !resp_body || !resp_free || !sess_uid || !sess_data ||
      !sess_free || !err_msg || !free_str) {
    fprintf(stderr, "Missing symbols\n");
    dlclose(lib);
    return 1;
  }

  void *config = cfg_new();
  cfg_cookie(config, "sid");
  cfg_lt(config, 3600);

  void *mgr = mgr_new(config, NULL, auth_cb, get_user_cb);
  if (!mgr) {
    fprintf(stderr, "Failed to create AuthManager\n");
    dlclose(lib);
    return 1;
  }

  char *err = NULL;
  void *resp = login(mgr, "admin", "secret", &err);
  if (err) {
    printf("Login error: %s\n", err);
    free_str(err);
  }
  if (resp) {
    printf("Login: status=%u\n", resp_status(resp));
    char *body = resp_body(resp);
    printf("Body: %s\n", body);
    free_str(body);
    resp_free(resp);
  }

  char *sess_id_str = NULL;
  void *sess_info = NULL;
  err = NULL;
  int ret = create_sess(mgr, "admin", "{\"role\":\"admin\"}", &sess_id_str,
                        &sess_info, &err);
  if (ret == 0 && sess_id_str && sess_info) {
    printf("Session created: id=%s\n", sess_id_str);
    printf("  user_id: %s\n", sess_uid(sess_info));
    printf("  data: %s\n", sess_data(sess_info));

    char *val_err = NULL;
    void *val_info = validate(mgr, sess_id_str, &val_err);
    if (val_err) {
      printf("Validate error: %s\n", val_err);
      free_str(val_err);
    }
    if (val_info) {
      printf("Session valid! user_id=%s\n", sess_uid(val_info));
      sess_free(val_info);
    } else {
      printf("Session invalid\n");
    }

    free_str(sess_id_str);
    sess_free(sess_info);
  } else {
    if (err) {
      printf("Create session error: %s\n", err);
      free_str(err);
    }
  }

  err = NULL;
  void *logout_resp = logout(mgr, NULL, &err);
  if (err) {
    printf("Logout error: %s\n", err);
    free_str(err);
  }
  if (logout_resp) {
    printf("Logout: status=%u\n", resp_status(logout_resp));
    char *body = resp_body(logout_resp);
    printf("Body: %s\n", body);
    free_str(body);
    resp_free(logout_resp);
  }

  mgr_free(mgr);
  dlclose(lib);
  printf("\nServer simulation completed successfully.\n");
  return 0;
}
