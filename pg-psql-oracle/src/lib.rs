//! An oracle for psql's own client lexer.
//!
//! `pg-oracle` links PostgreSQL's raw parser so that pg-sql can be compared
//! with the server (ADR 0002). This crate does the same one layer up: it
//! links `src/fe_utils/psqlscan.l` and `src/bin/psql/psqlscanslash.l` of the
//! same pinned PostgreSQL release, so that pg-psql can be compared with psql.
//!
//! The version feature (`pg14` ... `pg19-beta`, default `pg17`) selects the
//! release from `pg-oracle/pins.tsv`, which is the one pin table of the
//! repository (ADR 0009).
//!
//! # What it reports
//!
//! [`scan`] gives, for one psql source document:
//!
//! - [`Scan::sql`], the text psql forwards to the server;
//! - one [`Event`] for each variable interpolation the lexer recognised, with
//!   its extent in the document, the quoting psql asked for, and where the
//!   value landed in the forwarded text;
//! - one event for each `;` that submits the query buffer;
//! - one event for each backslash command, with the command name exactly as
//!   `psqlscanslash.l` reads it; and
//! - a final event saying whether the document ended with a complete query
//!   buffer, and if not, which lexical state was still open.
//!
//! Those are the facts pg-psql models (`docs/adr/0007-give-psql-its-own-grammar-and-crate.md`).
//!
//! # Two adaptations of psql's main loop
//!
//! psql reads one line at a time and resets the query buffer after every
//! submission. pg-psql renders one buffer for a whole document. The shim
//! therefore scans the whole document as one input and never resets the
//! buffer, so that the two texts are comparable. `csrc/psql_oracle.c`
//! documents both, and `docs/psql-oracle.md` records what each one costs.

use std::ffi::CStr;
use std::ops::Range;
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

/// The Git ref of the PostgreSQL release that this oracle is built from.
pub const POSTGRES_REF: &str = env!("PG_PSQL_ORACLE_POSTGRES_REF");

/// The Git commit of the PostgreSQL release that this oracle is built from.
pub const POSTGRES_COMMIT: &str = env!("PG_PSQL_ORACLE_POSTGRES_COMMIT");

#[repr(C)]
struct RawEvent {
    kind: c_int,
    source_start: c_int,
    source_end: c_int,
    output_start: c_int,
    output_end: c_int,
    detail: c_int,
    bound: c_int,
    text: *mut c_char,
}

#[repr(C)]
struct RawScan {
    events: *mut RawEvent,
    nevents: c_int,
    capacity: c_int,
    sql: *mut c_char,
    sql_len: c_int,
    copy_from_stdin: c_int,
}

unsafe extern "C" {
    fn pgpso_scan(
        source: *const c_char,
        source_len: c_int,
        names: *const *const c_char,
        values: *const *const c_char,
        nvars: c_int,
    ) -> *mut RawScan;
    fn pgpso_free(scan: *mut RawScan);
}

/// psql's lexer keeps state in file-scope flex tables; serialise every call.
static LOCK: Mutex<()> = Mutex::new(());

/// How psql asked for a variable's value. These are `PsqlScanQuoteType` in
/// `src/include/fe_utils/psqlscan.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    /// `:name` and `:{?name}`.
    Plain,
    /// `:'name'`.
    Literal,
    /// `:"name"`.
    Identifier,
    /// A backslash-command argument. This oracle never reads one.
    ShellArgument,
}

/// Why psql stopped scanning at the end of the document. These are
/// `promptStatus_t` in `src/include/fe_utils/psqlscan.h`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Ready,
    Continue,
    Comment,
    SingleQuote,
    DoubleQuote,
    DollarQuote,
    Paren,
    Copy,
}

impl Prompt {
    fn from_raw(value: c_int) -> Self {
        match value {
            0 => Self::Ready,
            1 => Self::Continue,
            2 => Self::Comment,
            3 => Self::SingleQuote,
            4 => Self::DoubleQuote,
            5 => Self::DollarQuote,
            6 => Self::Paren,
            7 => Self::Copy,
            other => panic!("unknown promptStatus_t {other}"),
        }
    }

    /// Whether the document ended inside a string, a comment, a dollar-quoted
    /// body or a quoted identifier.
    ///
    /// psql tolerates this and asks for another line; pg-psql refuses the
    /// document instead (ADR 0007, freshtonic/recursa#131), so this is the
    /// oracle's side of that deliberate divergence.
    pub fn is_open_lexical_region(self) -> bool {
        matches!(
            self,
            Self::Comment | Self::SingleQuote | Self::DoubleQuote | Self::DollarQuote
        )
    }
}

/// One fact psql's lexer reported about a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A variable interpolation the lexer recognised.
    Interpolation {
        /// The variable name, without the interpolation's punctuation.
        name: String,
        /// The interpolation's extent in the document. `None` when psql was
        /// rescanning a substituted value, which has no place in the source.
        source: Option<Range<usize>>,
        /// The quoting psql asked for.
        quote: Quote,
        /// Whether the variable was bound. An unbound one is forwarded as it
        /// stands, except `:{?name}`, which answers `FALSE`.
        bound: bool,
        /// Where the substituted value starts in the forwarded text.
        output_start: usize,
        /// Where it ends, when psql emits it in place. `None` for `:name`,
        /// whose value psql pushes back into the lexer and rescans.
        output_end: Option<usize>,
    },
    /// A `;` that submits the query buffer.
    Terminator {
        /// The semicolon's extent in the document.
        source: Range<usize>,
        /// Its extent in the forwarded text. psql forwards the `;`.
        output: Range<usize>,
    },
    /// A backslash command.
    Backslash {
        /// The command name, without the backslash, exactly as
        /// `psqlscanslash.l` reads it.
        name: String,
        /// The whole command: the backslash, the name, and the arguments,
        /// which reach the next line break because psql reads one line at a
        /// time.
        source: Range<usize>,
        /// The backslash and the name alone.
        command: Range<usize>,
        /// Where it sits in the forwarded text. psql forwards none of it, so
        /// the range is empty.
        output: usize,
    },
    /// The end of the document, with a complete query buffer.
    End { prompt: Prompt },
    /// The end of the document, with an incomplete query buffer.
    Incomplete { prompt: Prompt },
}

/// What psql's lexer made of one document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scan {
    sql: String,
    events: Vec<Event>,
    copy_from_stdin: usize,
}

impl Scan {
    /// The text psql forwards to the server, for the whole document.
    ///
    /// A backslash command contributes nothing: psql sends the query buffer
    /// and the command never reaches the server. Use [`Scan::events`] to see
    /// where the boundaries are.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Every fact the lexer reported, in document order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// How many `COPY ... FROM STDIN` commands psqlscan.l counted. psql reads
    /// the data lines that follow one of these itself; this oracle and
    /// pg-psql both lex them as ordinary text.
    pub fn copy_from_stdin(&self) -> usize {
        self.copy_from_stdin
    }

    /// Whether the document ended with a complete query buffer.
    pub fn is_complete(&self) -> bool {
        matches!(self.events.last(), Some(Event::End { .. }))
    }

    /// The prompt psql would show after the document.
    pub fn final_prompt(&self) -> Prompt {
        match self.events.last() {
            Some(Event::End { prompt } | Event::Incomplete { prompt }) => *prompt,
            _ => panic!("a scan always ends with an End or Incomplete event"),
        }
    }

    /// The backslash command names, in order.
    pub fn backslash_commands(&self) -> impl Iterator<Item = &str> {
        self.events.iter().filter_map(|event| match event {
            Event::Backslash { name, .. } => Some(name.as_str()),
            _ => None,
        })
    }
}

/// Scans `source` as psql would, with `variables` bound.
///
/// `variables` is a list of `(name, value)` pairs. A name that is not in the
/// list is unbound, and psql forwards its interpolation unchanged.
///
/// # Panics
///
/// If `source` holds a NUL byte, or is longer than `i32::MAX`.
pub fn scan(source: &str, variables: &[(&str, &str)]) -> Scan {
    assert!(
        !source.as_bytes().contains(&0),
        "a psql document must not hold a NUL byte"
    );
    let length = c_int::try_from(source.len()).expect("the document fits in an int");

    let names = variables
        .iter()
        .map(|(name, _)| std::ffi::CString::new(*name).expect("NUL in a variable name"))
        .collect::<Vec<_>>();
    let values = variables
        .iter()
        .map(|(_, value)| std::ffi::CString::new(*value).expect("NUL in a variable value"))
        .collect::<Vec<_>>();
    let name_pointers = names.iter().map(|name| name.as_ptr()).collect::<Vec<_>>();
    let value_pointers = values.iter().map(|value| value.as_ptr()).collect::<Vec<_>>();

    let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: the pointers live for the call, and the lengths agree with them.
    unsafe {
        let raw = pgpso_scan(
            source.as_ptr() as *const c_char,
            length,
            name_pointers.as_ptr(),
            value_pointers.as_ptr(),
            c_int::try_from(variables.len()).expect("the variable count fits in an int"),
        );
        assert!(!raw.is_null(), "pgpso_scan returned no result");
        let scan = read_scan(&*raw, source.len());
        pgpso_free(raw);
        scan
    }
}

/// Reads the C result. `source_len` bounds every source offset, so a bad
/// offset stops the test rather than producing a silent mismatch.
unsafe fn read_scan(raw: &RawScan, source_len: usize) -> Scan {
    let sql = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(
            raw.sql as *const u8,
            raw.sql_len as usize,
        ))
    }
    .expect("psql forwards the document's own bytes, so the text is UTF-8")
    .to_owned();

    let raw_events =
        unsafe { std::slice::from_raw_parts(raw.events as *const RawEvent, raw.nevents as usize) };
    let events = raw_events
        .iter()
        .map(|event| read_event(event, source_len))
        .collect();

    Scan {
        sql,
        events,
        copy_from_stdin: raw.copy_from_stdin as usize,
    }
}

fn read_event(event: &RawEvent, source_len: usize) -> Event {
    let text = || {
        assert!(!event.text.is_null(), "this event kind carries text");
        unsafe { CStr::from_ptr(event.text) }
            .to_str()
            .expect("psql reads the document's own bytes, so the text is UTF-8")
            .to_owned()
    };
    let source = |start: c_int, end: c_int| {
        let (start, end) = (start as usize, end as usize);
        assert!(
            start <= end && end <= source_len,
            "the oracle reported the source range {start}..{end} \
             outside a document of {source_len} bytes",
        );
        start..end
    };

    match event.kind {
        1 => Event::Interpolation {
            name: text(),
            source: (event.source_start >= 0)
                .then(|| source(event.source_start, event.source_end)),
            quote: match event.detail {
                0 => Quote::Plain,
                1 => Quote::Literal,
                2 => Quote::Identifier,
                3 => Quote::ShellArgument,
                other => panic!("unknown PsqlScanQuoteType {other}"),
            },
            bound: event.bound != 0,
            output_start: event.output_start as usize,
            output_end: (event.output_end >= 0).then(|| event.output_end as usize),
        },
        2 => Event::Terminator {
            source: source(event.source_start, event.source_end),
            output: (event.output_start as usize)..(event.output_end as usize),
        },
        3 => {
            let name = text();
            let whole = source(event.source_start, event.source_end);
            let command = whole.start..(whole.start + event.detail as usize);
            Event::Backslash {
                name,
                source: whole,
                command,
                output: event.output_start as usize,
            }
        }
        4 => Event::End {
            prompt: Prompt::from_raw(event.detail),
        },
        5 => Event::Incomplete {
            prompt: Prompt::from_raw(event.detail),
        },
        other => panic!("unknown pgpso event kind {other}"),
    }
}
