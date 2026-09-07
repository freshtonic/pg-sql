# Use scoped LR conflict resolutions

Status: implemented

pg-sql used explicit, scoped resolutions for recursa #132's affected LALR action
conflicts: ten shift/reduce cells and two reduce/reduce cells. A declaration must identify conflicts
through stable grammar-source identities and lookaheads, never generated state
numbers; name the intended action or reducing source rule; match exactly the
conflicts it claims; and
be recorded in the automaton dump. A stale, missing, ambiguous, or over-broad
declaration is a build error, and every unresolved conflict remains a hard
build error.

The alternatives were runtime backtracking in recursive descent and a new
balanced-dispatch analysis that distinguishes languages inside a balanced
enclosure. Backtracking does not yet have defined bounds or complete rollback
semantics for this arbitrarily nested prefix. Interior balanced dispatch has no
designed algorithm or complexity bound. The scoped LR resolution is the
narrowest change, has no runtime parsing cost, and uses Recursa's existing
conflict-resolver seam.

The original 12 cells and the remaining grammar work are resolved: the current
pg-sql LALR dump has zero conflicts. The parser surface is table-driven-only
(`parser_style = table_driven`), so pg-sql does not emit predictive
recursive-descent statics. Recursa still supports recursive descent and applies
its predictive ambiguity checks whenever a grammar selects that style.
