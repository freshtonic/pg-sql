#include "postgres.h"
#include "parser/parser.h"
#include "nodes/pg_list.h"
#include "nodes/nodes.h"
#include "utils/memutils.h"
#include <setjmp.h>
#include <string.h>
#include <stdlib.h>

/* Defined in pgo_elog_stub.c: most-recent error message, or NULL. */
extern const char *pgo_last_error_message(void);

static void ensure_init(void) {
    static int inited = 0;
    if (!inited) { MemoryContextInit(); inited = 1; }
}

/* 0 = parsed OK; 1 = syntax error (*errmsg set to a malloc'd copy or NULL). */
int pgo_raw_parse_check(const char *sql, char **errmsg) {
    ensure_init();
    if (errmsg) *errmsg = NULL;
    MemoryContext ctx = AllocSetContextCreate(
        TopMemoryContext, "pgo", ALLOCSET_DEFAULT_SIZES);
    MemoryContext old = MemoryContextSwitchTo(ctx);

    int rc;
    sigjmp_buf jmp;
    sigjmp_buf *saved = PG_exception_stack;
    PG_exception_stack = &jmp;
    if (sigsetjmp(jmp, 1) == 0) {
        raw_parser(sql, RAW_PARSE_DEFAULT);
        rc = 0;
    } else {
        rc = 1;
        const char *m = pgo_last_error_message();
        if (errmsg && m) *errmsg = strdup(m);
    }
    PG_exception_stack = saved;

    MemoryContextSwitchTo(old);
    MemoryContextDelete(ctx);
    return rc;
}

/* Both parse OK + trees equal -> 0; trees differ -> 1; a invalid -> 2;
 * b invalid -> 3. equal() ignores `location` fields by construction. */
int pgo_raw_parse_equal(const char *a, const char *b) {
    ensure_init();
    MemoryContext ctx = AllocSetContextCreate(
        TopMemoryContext, "pgo_eq", ALLOCSET_DEFAULT_SIZES);
    MemoryContext old = MemoryContextSwitchTo(ctx);

    int rc;
    sigjmp_buf jmp;
    sigjmp_buf *saved = PG_exception_stack;
    PG_exception_stack = &jmp;
    if (sigsetjmp(jmp, 1) == 0) {
        List *ta = raw_parser(a, RAW_PARSE_DEFAULT);
        List *tb = raw_parser(b, RAW_PARSE_DEFAULT);
        rc = equal(ta, tb) ? 0 : 1;
    } else {
        /* One of the two failed; re-run singly to attribute the error. */
        rc = -1;
    }
    PG_exception_stack = saved;
    MemoryContextSwitchTo(old);
    MemoryContextDelete(ctx);

    if (rc == -1) {
        rc = (pgo_raw_parse_check(a, NULL) != 0) ? 2 : 3;
    }
    return rc;
}

/* nodeToString of raw_parser(sql); malloc'd, caller frees. NULL on error. */
char *pgo_node_to_string(const char *sql) {
    ensure_init();
    MemoryContext ctx = AllocSetContextCreate(
        TopMemoryContext, "pgo_str", ALLOCSET_DEFAULT_SIZES);
    MemoryContext old = MemoryContextSwitchTo(ctx);

    char *out = NULL;
    sigjmp_buf jmp;
    sigjmp_buf *saved = PG_exception_stack;
    PG_exception_stack = &jmp;
    if (sigsetjmp(jmp, 1) == 0) {
        List *t = raw_parser(sql, RAW_PARSE_DEFAULT);
        char *s = nodeToString(t);
        out = strdup(s);
    }
    PG_exception_stack = saved;
    MemoryContextSwitchTo(old);
    MemoryContextDelete(ctx);
    return out;
}

void pgo_free(void *p) { free(p); }
