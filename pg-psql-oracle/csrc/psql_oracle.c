/*-------------------------------------------------------------------------
 *
 * psql_oracle.c
 *	  An oracle for psql's own client-side SQL lexer.
 *
 * This shim drives `src/fe_utils/psqlscan.l` and `src/bin/psql/psqlscanslash.l`
 * of the pinned PostgreSQL release, and reports the facts that pg-psql
 * models: the text psql forwards to the server, where each variable
 * interpolation is, where a query buffer is submitted, and where a backslash
 * command begins and ends.
 *
 * Two adaptations of psql's own main loop are deliberate, and both follow
 * pg-psql's document model (ADR 0007):
 *
 * 1. The whole document is scanned as one input string, not one line at a
 *	  time.  The flex rules of psqlscan.l do not read line boundaries, so the
 *	  lexing is the same.  psql's line-at-a-time reading does bound how far a
 *	  backslash command's arguments can run, so this file resumes SQL lexing
 *	  at the next line break after a command name, which is where psql's own
 *	  input buffer would have ended.
 *
 * 2. The query buffer is never reset.  psql resets it after every
 *	  submission, and the `{whitespace}` rule of psqlscan.l does not echo
 *	  while the buffer is empty, so psql drops the leading trivia of every
 *	  statement.  pg-psql renders one buffer for a whole document, so the
 *	  buffer is seeded with one byte that is removed at the end; the trivia
 *	  then survives and the two texts are comparable.
 *
 *-------------------------------------------------------------------------
 */
#include "postgres_fe.h"

#include <stdlib.h>
#include <string.h>

#include "fe_utils/psqlscan.h"
#include "fe_utils/psqlscan_int.h"
#include "psqlscanslash.h"
#include "pqexpbuffer.h"
#include "mb/pg_wchar.h"

/*
 * Accessors that flex generates for psqlscan.l (`%option prefix="psql_yy"`).
 * `yytext` points into `state->scanbuf`, which psqlscan_prepare_buffer hands
 * straight to `yy_scan_buffer`, so the difference of the two pointers is a
 * byte offset into the input of the current psql_scan_setup.
 */
extern char *psql_yyget_text(yyscan_t yyscanner);
extern size_t psql_yyget_leng(yyscan_t yyscanner);

/*
 * psqlscanslash.l calls this after it has evaluated a backquoted argument.
 * Only psql_scan_slash_option reaches a backquote, and this oracle reads no
 * arguments — it calls psql_scan_slash_command alone — so no shell command is
 * ever run.  The definition only satisfies the linker; psql's own is in
 * src/bin/psql/common.c.
 */
void		SetShellResultVariables(int exit_code);

void
SetShellResultVariables(int exit_code)
{
	(void) exit_code;
}

/* Event kinds; kept in step with the Rust `EventKind`. */
#define PGPSO_INTERPOLATION 1
#define PGPSO_SEMICOLON 2
#define PGPSO_BACKSLASH 3
#define PGPSO_EOL 4
#define PGPSO_INCOMPLETE 5

typedef struct PgpsoEvent
{
	int			kind;
	/* Extent in the document, or -1 when the token came from an expansion. */
	int			source_start;
	int			source_end;
	/* Extent in the forwarded text.  `output_end` is -1 when psql rescans
	 * the substituted value, which only a `:name` substitution does. */
	int			output_start;
	int			output_end;
	/* PsqlScanQuoteType for an interpolation, promptStatus_t for a stop. */
	int			detail;
	/* 1 when the variable was bound, 0 when it was not. */
	int			bound;
	/* Variable name, or backslash command name; malloc'd, or NULL. */
	char	   *text;
} PgpsoEvent;

typedef struct PgpsoResult
{
	PgpsoEvent *events;
	int			nevents;
	int			capacity;
	/* The text psql forwards to the server, for the whole document. */
	char	   *sql;
	int			sql_len;
	/* psqlscan.l's count of `COPY ... FROM STDIN` commands. */
	int			copy_from_stdin;
} PgpsoResult;

typedef struct PgpsoContext
{
	const char *const *names;
	const char *const *values;
	int			nvars;
	PsqlScanState state;
	PQExpBuffer output;
	PgpsoResult *result;
	/* Document offset of the current psql_scan_setup. */
	int			base;
	/* Bytes of seed at the start of `output`. */
	int			seed;
} PgpsoContext;

static PgpsoEvent *
push_event(PgpsoResult *result, int kind)
{
	PgpsoEvent *event;

	if (result->nevents == result->capacity)
	{
		result->capacity = result->capacity ? result->capacity * 2 : 16;
		result->events = realloc(result->events,
								 (size_t) result->capacity * sizeof(PgpsoEvent));
	}
	event = &result->events[result->nevents++];
	memset(event, 0, sizeof(*event));
	event->kind = kind;
	event->source_start = -1;
	event->source_end = -1;
	event->output_start = -1;
	event->output_end = -1;
	event->detail = -1;
	return event;
}

/*
 * The extent of the token flex has just matched, in document offsets.
 * Returns 0 when the token is not in the outer buffer, which happens while a
 * `:name` substitution is being rescanned.
 */
static int
current_token(PgpsoContext *context, int *start, int *end)
{
	const char *text;

	if (context->state->buffer_stack != NULL)
		return 0;
	text = psql_yyget_text(context->state->scanner);
	*start = context->base + (int) (text - context->state->scanbuf);
	*end = *start + (int) psql_yyget_leng(context->state->scanner);
	return 1;
}

/*
 * Quote `value` as a SQL string literal, as libpq's PQescapeLiteral does
 * (`src/interfaces/libpq/fe-exec.c`, PQescapeInternal): every `'` and every
 * `\` is doubled, and a value holding a backslash is written as ` E'...'` so
 * that it means the same under either standard_conforming_strings.
 *
 * psql itself calls PQescapeLiteral, which needs the client encoding and the
 * standard_conforming_strings of a live connection.  This oracle has no
 * connection, so it applies the same algorithm for the UTF-8, standard-strings
 * case that pg-psql assumes.
 */
static char *
escape_literal(const char *value)
{
	size_t		length = strlen(value);
	size_t		specials = 0;
	size_t		backslashes = 0;
	char	   *out;
	char	   *write;
	size_t		i;

	for (i = 0; i < length; i++)
	{
		if (value[i] == '\'')
			specials++;
		else if (value[i] == '\\')
		{
			specials++;
			backslashes++;
		}
	}

	out = malloc(length + specials + 5);
	write = out;
	if (backslashes > 0)
	{
		*write++ = ' ';
		*write++ = 'E';
	}
	*write++ = '\'';
	for (i = 0; i < length; i++)
	{
		if (value[i] == '\'' || value[i] == '\\')
			*write++ = value[i];
		*write++ = value[i];
	}
	*write++ = '\'';
	*write = '\0';
	return out;
}

/* Quote `value` as a quoted identifier, as PQescapeIdentifier does. */
static char *
escape_identifier(const char *value)
{
	size_t		length = strlen(value);
	size_t		quotes = 0;
	char	   *out;
	char	   *write;
	size_t		i;

	for (i = 0; i < length; i++)
		if (value[i] == '"')
			quotes++;

	out = malloc(length + quotes + 3);
	write = out;
	*write++ = '"';
	for (i = 0; i < length; i++)
	{
		if (value[i] == '"')
			*write++ = '"';
		*write++ = value[i];
	}
	*write++ = '"';
	*write = '\0';
	return out;
}

/*
 * The lexer's `get_variable` callback.  It is the one place psqlscan.l tells
 * a client that it has recognised an interpolation, so every interpolation
 * event is recorded here rather than inferred from the text.
 */
static char *
pgpso_get_variable(const char *varname, PsqlScanQuoteType quote,
				   void *passthrough)
{
	PgpsoContext *context = (PgpsoContext *) passthrough;
	PgpsoEvent *event;
	const char *value = NULL;
	char	   *result = NULL;
	int			start;
	int			end;
	int			i;

	for (i = 0; i < context->nvars; i++)
		if (strcmp(context->names[i], varname) == 0)
		{
			value = context->values[i];
			break;
		}

	event = push_event(context->result, PGPSO_INTERPOLATION);
	event->detail = (int) quote;
	event->bound = value != NULL;
	event->text = strdup(varname);
	event->output_start = (int) context->output->len - context->seed;
	if (current_token(context, &start, &end))
	{
		event->source_start = start;
		event->source_end = end;
	}

	if (value == NULL)
		return NULL;

	switch (quote)
	{
		case PQUOTE_PLAIN:
			result = strdup(value);
			break;
		case PQUOTE_SQL_LITERAL:
			result = escape_literal(value);
			break;
		case PQUOTE_SQL_IDENT:
			result = escape_identifier(value);
			break;
		case PQUOTE_SHELL_ARG:
			/* Only backslash-command arguments ask for this, and this oracle
			 * never reads them. */
			result = strdup(value);
			break;
	}

	/*
	 * A quoted form is emitted in place, so its extent in the forwarded text
	 * is known now.  A `:name` value is pushed back into the lexer and
	 * rescanned, so its extent is not.
	 */
	if (quote != PQUOTE_PLAIN)
		event->output_end = event->output_start + (int) strlen(result);
	return result;
}

/*
 * Scan one psql document.
 *
 * `names` and `values` are `nvars` parallel arrays of NUL-terminated strings:
 * the psql variables that are set.  The caller owns them.
 */
PgpsoResult *pgpso_scan(const char *source, int source_len,
						const char *const *names, const char *const *values,
						int nvars);

PgpsoResult *
pgpso_scan(const char *source, int source_len,
		   const char *const *names, const char *const *values, int nvars)
{
	PsqlScanCallbacks callbacks;
	PQExpBufferData output;
	PgpsoContext context;
	PgpsoResult *result = calloc(1, sizeof(PgpsoResult));
	PsqlScanState state;
	int			base = 0;
	int			finished = 0;

	callbacks.get_variable = pgpso_get_variable;
	state = psql_scan_create(&callbacks);

	initPQExpBuffer(&output);
	/* See the file header, adaptation 2. */
	appendPQExpBufferChar(&output, ' ');

	context.names = names;
	context.values = values;
	context.nvars = nvars;
	context.state = state;
	context.output = &output;
	context.result = result;
	context.base = 0;
	context.seed = 1;
	psql_scan_set_passthrough(state, &context);

	while (!finished)
	{
		psql_scan_setup(state, source + base, source_len - base,
						PG_UTF8, true);
		context.base = base;

		for (;;)
		{
			promptStatus_t prompt = PROMPT_READY;
			PsqlScanResult scanned = psql_scan(state, &output, &prompt);
			PgpsoEvent *event;
			int			start;
			int			end;

			if (scanned == PSCAN_SEMICOLON)
			{
				event = push_event(result, PGPSO_SEMICOLON);
				if (current_token(&context, &start, &end))
				{
					event->source_start = start;
					event->source_end = end;
				}
				event->output_end = (int) output.len - context.seed;
				event->output_start = event->output_end - 1;
				continue;
			}

			if (scanned == PSCAN_BACKSLASH)
			{
				char	   *name;
				int			at = -1;
				int			after;
				int			line_end;

				if (current_token(&context, &start, &end))
					at = start;
				else
				{
					/*
					 * The backslash came from a substituted value, which has
					 * no place in the document.  Attribute it to the start of
					 * the current input so the scan still advances.
					 */
					at = base;
				}
				name = psql_scan_slash_command(state);

				/*
				 * The command is the backslash plus the name psqlscanslash.l
				 * read.  The encoding is UTF-8, so psqlscan_prepare_buffer
				 * copies the input unchanged and the name has its source
				 * length.
				 */
				after = at + 1 + (int) strlen(name);
				line_end = after;
				while (line_end < source_len && source[line_end] != '\n')
					line_end++;

				/*
				 * The extent is the whole command: the backslash, the name
				 * psqlscanslash.l read, and the arguments, which reach the
				 * line break because psql's own input buffer would have
				 * ended there.  `text` is the name, so the caller can split
				 * the two.  psql forwards none of it to the server, so both
				 * output offsets are the current end of the forwarded text.
				 */
				event = push_event(result, PGPSO_BACKSLASH);
				event->source_start = at;
				event->source_end = line_end;
				event->detail = 1 + (int) strlen(name);
				event->output_start = (int) output.len - context.seed;
				event->output_end = event->output_start;
				event->text = name;

				/* Resume SQL lexing at the line break (adaptation 1). */
				psql_scan_finish(state);
				base = line_end;
				break;
			}

			event = push_event(result,
							   scanned == PSCAN_EOL ? PGPSO_EOL : PGPSO_INCOMPLETE);
			event->detail = (int) prompt;
			event->source_start = source_len;
			event->source_end = source_len;
			event->output_start = (int) output.len - context.seed;
			event->output_end = event->output_start;
			psql_scan_finish(state);
			finished = 1;
			break;
		}
	}

	result->copy_from_stdin = psql_scan_count_copy_from_stdin(state);
	result->sql_len = (int) output.len - context.seed;
	result->sql = malloc((size_t) result->sql_len + 1);
	memcpy(result->sql, output.data + context.seed, (size_t) result->sql_len);
	result->sql[result->sql_len] = '\0';

	termPQExpBuffer(&output);
	psql_scan_destroy(state);
	return result;
}

void		pgpso_free(PgpsoResult *result);

void
pgpso_free(PgpsoResult *result)
{
	int			i;

	if (result == NULL)
		return;
	for (i = 0; i < result->nevents; i++)
		free(result->events[i].text);
	free(result->events);
	free(result->sql);
	free(result);
}
