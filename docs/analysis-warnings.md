# Recursa analysis diagnostics

The PostgreSQL grammar must generate without unresolved LALR conflicts.
Conflict resolutions are source-scoped declarations that name the intended
action and lookahead and must match exactly the conflict cells they claim.
The parser reports an unmatched or over-broad resolution as a generation
error. Former optional-viability warnings and their acceptance annotations
are no longer part of the grammar surface.
