/*
 * pgo_elog_stub.c — stripped error handling for the standalone parser.
 *
 * PostgreSQL's real elog.c pulls in logging, GUC machinery, proc_exit and
 * a great deal more. For the differential-parser oracle we need only the
 * minimum: an errstart/errfinish pair that records the most recent ERROR
 * message and, on ERROR, siglongjmp's to the caller's PG_exception_stack.
 *
 * This is modelled on libpg_query's src/postgres/src_backend_utils_error_elog.c
 * (which strips the same file down to referenced symbols). We do not copy
 * any parser logic — only build scaffolding.
 *
 * It also defines the process globals the backend headers expect. Scanner
 * configuration globals are owned by PostgreSQL's generated scan.c.
 */
#include "postgres.h"

#include <stdarg.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "miscadmin.h"
#include "utils/elog.h"
#include "parser/parser.h"
#include "nodes/pg_list.h"
#include "nodes/value.h"
#include "nodes/parsenodes.h"
#include "lib/stringinfo.h"

/* ---- process globals the backend headers expect ---------------------- */
volatile sig_atomic_t InterruptPending = false;
volatile sig_atomic_t QueryCancelPending = false;
volatile sig_atomic_t ProcDiePending = false;
volatile sig_atomic_t IdleInTransactionSessionTimeoutPending = false;
volatile sig_atomic_t IdleSessionTimeoutPending = false;
volatile uint32 InterruptHoldoffCount = 0;
volatile uint32 QueryCancelHoldoffCount = 0;
volatile uint32 CritSectionCount = 0;

/* ---- elog.c globals -------------------------------------------------- */
ErrorContextCallback *error_context_stack = NULL;
sigjmp_buf *PG_exception_stack = NULL;

/* ---- captured error message ----------------------------------------- */
static char pgo_error_buf[1024];
static int  pgo_have_error = 0;

const char *pgo_last_error_message(void) {
    return pgo_have_error ? pgo_error_buf : NULL;
}

/* ---------------------------------------------------------------------- *
 * errstart / errfinish — minimal.                                        *
 *                                                                        *
 * The error API is a sequence of macro calls:                            *
 *   ereport(level, (errcode(...), errmsg(...), ...))                      *
 * which expands to:                                                      *
 *   if (errstart(level, domain))                                         *
 *       (errcode(...), errmsg(...), ...), errfinish(file, line, func);    *
 * The errcode()/errmsg()/... helpers run FIRST (they are the leading      *
 * operands of the comma expression) to fill the current error record,    *
 * and errfinish() runs LAST. For levels >= ERROR, errstart returns true   *
 * and errfinish never returns.                                           *
 * ---------------------------------------------------------------------- */

#define PGO_MAXSTACK 8
static int  pgo_stack_depth = -1;          /* -1 == empty */
static int  pgo_cur_level[PGO_MAXSTACK];

bool
errstart(int elevel, const char *domain)
{
    /* Push a record onto our tiny stack. */
    if (pgo_stack_depth + 1 >= PGO_MAXSTACK) {
        /* recursion overflow — just abort, this should never happen */
        write_stderr("pgo_elog_stub: error stack overflow\n");
        abort();
    }
    pgo_stack_depth++;
    pgo_cur_level[pgo_stack_depth] = elevel;

    if (elevel >= ERROR) {
        /* fresh message for this error */
        pgo_error_buf[0] = '\0';
        pgo_have_error = 1;
    }
    /* Returning true means "build the message and call errfinish". We do
     * this for every level so that errmsg() etc. run; for sub-ERROR levels
     * errfinish simply pops without longjmp. */
    return true;
}

bool
errstart_cold(int elevel, const char *domain)
{
    return errstart(elevel, domain);
}

void
errfinish(const char *filename, int lineno, const char *funcname)
{
    int elevel;

    (void) filename;
    (void) lineno;
    (void) funcname;

    Assert(pgo_stack_depth >= 0);
    elevel = pgo_cur_level[pgo_stack_depth];
    pgo_stack_depth--;

    if (elevel >= ERROR) {
        /* Unwind to the caller's setjmp point. */
        if (PG_exception_stack != NULL)
            siglongjmp(*PG_exception_stack, 1);
        /* No handler installed: fatal. */
        write_stderr("pgo_elog_stub: ERROR with no PG_exception_stack: %s\n",
                     pgo_error_buf);
        abort();
    }
    /* sub-ERROR levels: nothing to report, just return */
}

/* ---- message-building helpers --------------------------------------- *
 * They all return 0 (or void) — the ereport() macro just needs them to   *
 * be callable inside a comma expression. We capture errmsg's text.       *
 * ---------------------------------------------------------------------- */

static void
pgo_capture(const char *fmt, va_list args)
{
    /* Only capture if we are inside an ERROR (errstart set pgo_have_error
     * and cleared the buffer). Don't clobber on sub-ERROR messages. */
    if (pgo_stack_depth >= 0 && pgo_cur_level[pgo_stack_depth] >= ERROR) {
        vsnprintf(pgo_error_buf, sizeof(pgo_error_buf), fmt, args);
    }
}

int
errmsg(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    pgo_capture(fmt, args);
    va_end(args);
    return 0;
}

int
errmsg_internal(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    pgo_capture(fmt, args);
    va_end(args);
    return 0;
}

int
errmsg_plural(const char *fmt_singular, const char *fmt_plural,
              unsigned long n, ...)
{
    va_list args;
    va_start(args, n);
    pgo_capture(n == 1 ? fmt_singular : fmt_plural, args);
    va_end(args);
    return 0;
}

int errcode(int sqlerrcode) { (void) sqlerrcode; return 0; }
int errcode_for_file_access(void) { return 0; }
int errcode_for_socket_access(void) { return 0; }

int
errdetail(const char *fmt, ...)
{
    (void) fmt;
    return 0;
}

int
errdetail_internal(const char *fmt, ...)
{
    (void) fmt;
    return 0;
}

int
errdetail_log(const char *fmt, ...)
{
    (void) fmt;
    return 0;
}

int
errdetail_plural(const char *fmt_singular, const char *fmt_plural,
                 unsigned long n, ...)
{
    (void) fmt_singular;
    (void) fmt_plural;
    (void) n;
    return 0;
}

int
errhint(const char *fmt, ...)
{
    (void) fmt;
    return 0;
}

int
errhint_plural(const char *fmt_singular, const char *fmt_plural,
               unsigned long n, ...)
{
    (void) fmt_singular;
    (void) fmt_plural;
    (void) n;
    return 0;
}

int errposition(int cursorpos) { (void) cursorpos; return 0; }
int internalerrposition(int cursorpos) { (void) cursorpos; return 0; }
int internalerrquery(const char *query) { (void) query; return 0; }
int err_generic_string(int field, const char *str) { (void) field; (void) str; return 0; }
int errhidestmt(bool hide_stmt) { (void) hide_stmt; return 0; }
int errhidecontext(bool hide_ctx) { (void) hide_ctx; return 0; }

int
errcontext_msg(const char *fmt, ...)
{
    (void) fmt;
    return 0;
}

int set_errcontext_domain(const char *domain) { (void) domain; return 0; }

/* geterrcode — the SQLSTATE of the error currently being built. We do not
 * track errcodes; return a generic syntax-error code (anything other than
 * ERRCODE_QUERY_CANCELED, which the scanner special-cases). */
int
geterrcode(void)
{
    return ERRCODE_SYNTAX_ERROR;
}

int
errbacktrace(void)
{
    return 0;
}

/* errsave_start / errsave_finish — the "soft error" API. With no soft-error
 * context we treat every soft error as a hard ERROR. */
bool
errsave_start(struct Node *context, const char *domain)
{
    (void) context;
    return errstart(ERROR, domain);
}

void
errsave_finish(struct Node *context, const char *filename, int lineno,
               const char *funcname)
{
    (void) context;
    errfinish(filename, lineno, funcname);
}

/* pg_re_throw — re-raise the current error (used by PG_RE_THROW). */
void
pg_re_throw(void)
{
    if (PG_exception_stack != NULL)
        siglongjmp(*PG_exception_stack, 1);
    write_stderr("pgo_elog_stub: pg_re_throw with no handler\n");
    abort();
}

/* ---- elog (the variadic logging entry point) ------------------------- */
void
elog_start(const char *filename, int lineno, const char *funcname)
{
    (void) filename;
    (void) lineno;
    (void) funcname;
}

void
elog_finish(int elevel, const char *fmt, ...)
{
    if (elevel >= ERROR) {
        va_list args;
        pgo_error_buf[0] = '\0';
        pgo_have_error = 1;
        va_start(args, fmt);
        vsnprintf(pgo_error_buf, sizeof(pgo_error_buf), fmt, args);
        va_end(args);
        if (PG_exception_stack != NULL)
            siglongjmp(*PG_exception_stack, 1);
        write_stderr("pgo_elog_stub: elog ERROR with no handler: %s\n",
                     pgo_error_buf);
        abort();
    }
}

bool
message_level_is_interesting(int elevel)
{
    return elevel >= ERROR;
}

/* write_stderr — used by Assert / fatal paths. */
void
write_stderr(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    vfprintf(stderr, fmt, args);
    va_end(args);
    fflush(stderr);
}

/* ---- ProcessInterrupts — no signals here, so a no-op. ---------------- */
void
ProcessInterrupts(void)
{
}

/* ---- stack-depth check ----------------------------------------------- *
 * The backend uses stack_base_ptr / max_stack_depth to guard against     *
 * runaway recursion. We never set up the stack base, so just report      *
 * "not too deep" — corpus statements are not pathologically nested.      *
 * ---------------------------------------------------------------------- */
char *stack_base_ptr = NULL;
char *register_stack_base_ptr = NULL;
int   max_stack_depth = 100;

bool
stack_is_too_deep(void)
{
    return false;
}

void
check_stack_depth(void)
{
}

/* ---------------------------------------------------------------------- *
 * A few catalog/commands helpers the grammar reaches into. These have    *
 * no catalog dependency in practice — we provide the leaf logic so we    *
 * don't have to drag in namespace.c / define.c.                          *
 * ---------------------------------------------------------------------- */

/* NameListToString — dotted-join a list of String/A_Star nodes. */
char *
NameListToString(const List *names)
{
    StringInfoData string;
    ListCell   *l;

    initStringInfo(&string);

    foreach(l, names) {
        Node *name = (Node *) lfirst(l);

        if (l != list_head(names))
            appendStringInfoChar(&string, '.');

        if (IsA(name, String))
            appendStringInfoString(&string, strVal(name));
        else if (IsA(name, A_Star))
            appendStringInfoChar(&string, '*');
        else
            elog(ERROR, "unexpected node type in name list: %d",
                 (int) nodeTag(name));
    }

    return string.data;
}

/* defGetInt32 — extract an int32 from a DefElem holding an Integer. */
int32
defGetInt32(DefElem *def)
{
    if (def->arg == NULL)
        ereport(ERROR,
                (errcode(ERRCODE_SYNTAX_ERROR),
                 errmsg("%s requires an integer value", def->defname)));
    if (nodeTag(def->arg) == T_Integer)
        return (int32) intVal(def->arg);
    ereport(ERROR,
            (errcode(ERRCODE_SYNTAX_ERROR),
             errmsg("%s requires an integer value", def->defname)));
    return 0;
}

/* ---------------------------------------------------------------------- *
 * Multibyte/encoding stubs.                                              *
 *                                                                        *
 * The full backend mbutils.c depends on the catalog/fmgr subsystem       *
 * (FunctionCall6Coll etc.). The raw parser needs only a few mb functions *
 * and they delegate to encoding-agnostic helpers in common/wchar.c. We   *
 * hard-wire the database encoding to UTF-8 — the parser is encoding-      *
 * insensitive for the corpus we test, and UTF-8 is PostgreSQL's default. *
 * Modelled on libpg_query's stripped src_backend_utils_mb_mbutils.c.     *
 * ---------------------------------------------------------------------- */
#include "mb/pg_wchar.h"

int
GetDatabaseEncoding(void)
{
    return PG_UTF8;
}

int
pg_get_client_encoding(void)
{
    return PG_UTF8;
}

int
pg_database_encoding_max_length(void)
{
    return pg_encoding_max_length(PG_UTF8);
}

int
pg_mblen(const char *mbstr)
{
    return pg_encoding_mblen(PG_UTF8, mbstr);
}

int
pg_mbstrlen_with_len(const char *mbstr, int limit)
{
    int len = 0;

    while (limit > 0 && *mbstr) {
        int l = pg_mblen(mbstr);
        limit -= l;
        mbstr += l;
        len++;
    }
    return len;
}

int
pg_mbstrlen(const char *mbstr)
{
    int len = 0;

    while (*mbstr) {
        mbstr += pg_mblen(mbstr);
        len++;
    }
    return len;
}

int
pg_mbcliplen(const char *mbstr, int len, int limit)
{
    return pg_encoding_mbcliplen(PG_UTF8, mbstr, len, limit);
}

/* pg_encoding_mbcliplen — encoding-table-driven, copied verbatim from the
 * backend mbutils.c (it touches only pg_wchar_table from common/wchar.c, no
 * catalog state). */
int
pg_encoding_mbcliplen(int encoding, const char *mbstr, int len, int limit)
{
    mblen_converter mblen_fn;
    int clen = 0;
    int l;

    if (pg_encoding_max_length(encoding) == 1) {
        if (len > limit)
            len = limit;
        return len;
    }

    mblen_fn = pg_wchar_table[encoding].mblen;

    while (len > 0 && *mbstr) {
        l = (*mblen_fn)((const unsigned char *) mbstr);
        if ((clen + l) > limit)
            break;
        clen += l;
        if (clen == limit)
            break;
        len -= l;
        mbstr += l;
    }
    return clen;
}

bool
pg_verifymbstr(const char *mbstr, int len, bool noError)
{
    int oklen = pg_encoding_verifymbstr(PG_UTF8, mbstr, len);

    if (oklen != len) {
        if (noError)
            return false;
        ereport(ERROR,
                (errcode(ERRCODE_CHARACTER_NOT_IN_REPERTOIRE),
                 errmsg("invalid byte sequence for encoding \"UTF8\"")));
    }
    return true;
}

/* UTF-8-only Unicode->server conversion. With the database encoding fixed
 * to UTF-8 there is no conversion function to invoke — we just reformat the
 * code point. This is the PG_UTF8 fast path of the real mbutils.c. */
void
pg_unicode_to_server(pg_wchar c, unsigned char *s)
{
    if (!is_valid_unicode_codepoint(c))
        ereport(ERROR,
                (errcode(ERRCODE_SYNTAX_ERROR),
                 errmsg("invalid Unicode code point")));

    if (c <= 0x7F) {
        s[0] = (unsigned char) c;
        s[1] = '\0';
        return;
    }
    unicode_to_utf8(c, s);
    s[pg_utf_mblen(s)] = '\0';
}

bool
pg_unicode_to_server_noerror(pg_wchar c, unsigned char *s)
{
    if (!is_valid_unicode_codepoint(c))
        return false;

    if (c <= 0x7F) {
        s[0] = (unsigned char) c;
        s[1] = '\0';
        return true;
    }
    unicode_to_utf8(c, s);
    s[pg_utf_mblen(s)] = '\0';
    return true;
}
