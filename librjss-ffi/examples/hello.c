#include "librjss.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void die(JssClient *c, int rc, const char *what) {
  fprintf(stderr, "%s failed: rc=%d, msg=%s\n", what, rc,
          jss_last_error() ? jss_last_error() : "(none)");
  if (c)
    jss_client_free(c);
  exit(1);
}

int main(void) {
  printf("librjss-ffi version: %s\n", jss_version());

  JssClientConfig cfg;
  memset(&cfg, 0, sizeof(cfg));
  cfg.base_url = getenv("JSS_BASE_URL");
  cfg.auth_kind = "session";
  cfg.principal = getenv("JSS_EMAIL");
  cfg.secret = getenv("JSS_PASSWORD");
  cfg.flags = JSS_FLAG_INSECURE_SSL;
  cfg.timeout_secs = 30;
  cfg.max_retries = 3;

  if (!cfg.base_url || !cfg.principal || !cfg.secret) {
    fprintf(stderr, "Set JSS_BASE_URL, JSS_EMAIL, JSS_PASSWORD\n");
    return 1;
  }

  JssClient *c = jss_client_new(&cfg);
  if (!c)
    die(NULL, -1, "jss_client_new");

  int rc = jss_client_authenticate(c);
  if (rc != JSS_OK)
    die(c, rc, "authenticate");

  char *sitename = NULL;
  rc = jss_client_boot_sitename(c, &sitename);
  if (rc == JSS_OK) {
    printf("sitename: %s\n", sitename);
    jss_string_free(sitename);
  }

  char *roles = NULL;
  if (jss_client_boot_user_roles(c, &roles) == JSS_OK) {
    printf("roles: %s\n", roles);
    jss_string_free(roles);
  }

  char *body = NULL;
  rc = jss_client_get(c, "/api/resource/ToDo?limit_page_length=5", &body);
  if (rc == JSS_OK) {
    printf("ToDo: %s\n", body);
    jss_string_free(body);
  } else {
    fprintf(stderr, "GET failed: %s\n", jss_last_error());
  }

  char *resp = NULL;
  rc = jss_client_call_method(c, "frappe.client.get_count",
                              "{\"doctype\":\"ToDo\"}", &resp);
  if (rc == JSS_OK) {
    printf("count: %s\n", resp);
    jss_string_free(resp);
  }

  jss_client_logout(c);
  jss_client_free(c);
  return 0;
}
