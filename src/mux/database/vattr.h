
/* vattr.h - Definitions and management declarations for user-defined
 * attributes. */

#pragma once
constexpr int VNAME_SIZE = 32;

typedef struct user_attribute VATTR;
typedef struct GameDatabase GameDatabase;
typedef struct VattrStore VattrStore;
struct user_attribute {
  char *name; /* Name of user attribute */
  int number; /* Assigned attribute number */
  int flags;  /* Attribute flags */
};

VattrStore *vattr_store_create(GameDatabase *database);
void vattr_store_destroy(VattrStore *store);
int vattr_store_next_number(const VattrStore *store);
void vattr_store_set_next_number(VattrStore *store, int number);
extern VATTR *vattr_rename(VattrStore *, char *, char *);
extern VATTR *vattr_find(VattrStore *, char *);
extern VATTR *vattr_nfind(VattrStore *, int);
extern VATTR *vattr_alloc(VattrStore *, char *, int);
extern VATTR *vattr_define(VattrStore *, char *, int, int);
extern void vattr_delete(VattrStore *, char *);
extern VATTR *attr_rename(VattrStore *, char *, char *);
extern VATTR *vattr_first(VattrStore *);
extern VATTR *vattr_next(VattrStore *, VATTR *);
extern void list_vhashstats(DbRef);
