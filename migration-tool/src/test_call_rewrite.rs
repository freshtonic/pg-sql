//! Span-based rewrites for the repetitive calls in the imported test suite.

use std::collections::BTreeSet;
use std::path::Path;

use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::ToTokens as _;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Expr, ExprCall, ExprMethodCall, ItemMod, Local, Macro, Pat, Token};

use crate::rewrite::{RewriteError, SourceRewritePass, SpanEdit};

#[derive(Debug, Default)]
pub struct TestCallRewritePass;

impl SourceRewritePass for TestCallRewritePass {
    fn edits(&self, path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        let syntax =
            syn::parse_file(source).map_err(|error| pass_error(path, error.to_string()))?;
        let line_starts = line_starts(source);
        let whole_file_is_test = path_is_test_source(path) || has_cfg_test(&syntax.attrs);
        let mut visitor = TestCallVisitor {
            source,
            line_starts: &line_starts,
            active: whole_file_is_test,
            edits: Vec::new(),
            parse_calls: BTreeSet::new(),
            recognized_parse_calls: BTreeSet::new(),
            converted_parse_calls: BTreeSet::new(),
            errors: Vec::new(),
        };
        visitor.visit_file(&syntax);

        if let Some(message) = visitor.errors.into_iter().next() {
            return Err(pass_error(path, message));
        }
        if let Some((start, end)) = visitor
            .parse_calls
            .difference(&visitor.recognized_parse_calls)
            .next()
            .copied()
        {
            return Err(pass_error(
                path,
                format!(
                    "unsupported ambiguous direct parse at byte span {start}..{end}; expected a successful unwrap/expect chain, an existing into_ast conversion, a discarded `_` binding, or an explicit error assertion"
                ),
            ));
        }

        visitor.edits.sort_by_key(|edit| (edit.start, edit.end));
        visitor.edits.dedup();
        Ok(visitor.edits)
    }
}

struct TestCallVisitor<'a> {
    source: &'a str,
    line_starts: &'a [usize],
    active: bool,
    edits: Vec<SpanEdit>,
    parse_calls: BTreeSet<(usize, usize)>,
    recognized_parse_calls: BTreeSet<(usize, usize)>,
    converted_parse_calls: BTreeSet<(usize, usize)>,
    errors: Vec<String>,
}

impl<'ast> Visit<'ast> for TestCallVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let previous = self.active;
        self.active |= node.ident == "tests" || has_cfg_test(&node.attrs);
        syn::visit::visit_item_mod(self, node);
        self.active = previous;
    }

    fn visit_local(&mut self, node: &'ast Local) {
        if self.active {
            self.rewrite_test_input(node);
            self.recognize_discarded_parse_result(node);
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if self.active {
            self.inspect_method_call(node);
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if self.active
            && is_direct_parse(node)
            && let Some(range) = self.range(node.span())
        {
            self.parse_calls.insert(range);
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        if !self.active {
            return;
        }
        self.rewrite_macro_eof_checks(node.tokens.clone());
        let name = node.path.segments.last().map(|segment| &segment.ident);
        if name.is_some_and(|name| {
            matches!(
                name.to_string().as_str(),
                "assert" | "assert_eq" | "assert_ne"
            )
        }) {
            let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
            if let Ok(expressions) = parser.parse2(node.tokens.clone()) {
                for expression in &expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }
}

impl TestCallVisitor<'_> {
    fn recognize_discarded_parse_result(&mut self, local: &Local) {
        let Pat::Ident(binding) = &local.pat else {
            return;
        };
        if !binding.ident.to_string().starts_with('_') {
            return;
        }
        let Some(init) = &local.init else {
            return;
        };
        let Expr::Call(parse) = init.expr.as_ref() else {
            return;
        };
        if is_direct_parse(parse)
            && let Some(range) = self.range(parse.span())
        {
            self.recognized_parse_calls.insert(range);
        }
    }

    fn rewrite_test_input(&mut self, local: &Local) {
        let Pat::Ident(binding) = &local.pat else {
            return;
        };
        if binding.mutability.is_none() {
            return;
        }
        let Some(init) = &local.init else {
            return;
        };
        let Expr::Call(call) = init.expr.as_ref() else {
            return;
        };
        if !call_named(call, "test_input") {
            return;
        }
        if call.args.len() != 1 {
            self.errors.push(format!(
                "test_input at {:?} must have exactly one source argument",
                call.span().start()
            ));
            return;
        }
        let Some((start, end)) = self.range(local.span()) else {
            return;
        };
        let argument = call.args.first().expect("one argument");
        let Some((argument_start, argument_end)) = self.range(argument.span()) else {
            return;
        };
        let input_name = binding.ident.to_string();
        let lexed_name = if input_name == "input" {
            "lexed".to_owned()
        } else if let Some(stem) = input_name.strip_suffix("input") {
            format!("{stem}lexed")
        } else {
            format!("{input_name}_lexed")
        };
        let indent = " ".repeat(local.span().start().column);
        let argument = &self.source[argument_start..argument_end];
        let replacement = format!(
            "let {lexed_name} = crate::tokens::lex({argument});\n{indent}assert_eq!({lexed_name}.errors().count(), 0, \"lex errors in {input_name}\");\n{indent}let mut {input_name} = {lexed_name}.input();"
        );
        self.edits.push(SpanEdit {
            start,
            end,
            replacement,
        });
    }

    fn inspect_method_call(&mut self, node: &ExprMethodCall) {
        let method = node.method.to_string();

        if method == "is_empty" && is_input_expression(&node.receiver) && node.args.is_empty() {
            if let Some((start, end)) = self.range(node.method.span()) {
                self.edits.push(SpanEdit {
                    start,
                    end,
                    replacement: "is_eof".to_owned(),
                });
            }
            return;
        }

        if method == "into_ast" && node.args.is_empty() {
            if let Some(parse) = successful_parse_receiver(&node.receiver)
                && let Some(range) = self.range(parse.span())
            {
                self.recognized_parse_calls.insert(range);
                self.converted_parse_calls.insert(range);
            }
            return;
        }

        if matches!(method.as_str(), "unwrap" | "expect" | "unwrap_or_else") {
            if let Expr::Call(parse) = node.receiver.as_ref()
                && is_direct_parse(parse)
            {
                let Some(range) = self.range(parse.span()) else {
                    return;
                };
                self.recognized_parse_calls.insert(range);
                if !self.converted_parse_calls.contains(&range)
                    && let Some((_, end)) = self.range(node.span())
                {
                    self.edits.push(SpanEdit {
                        start: end,
                        end,
                        replacement: ".into_ast()".to_owned(),
                    });
                }
            }
            return;
        }

        if matches!(
            method.as_str(),
            "is_err" | "is_ok" | "unwrap_err" | "expect_err"
        ) && let Expr::Call(parse) = node.receiver.as_ref()
            && is_direct_parse(parse)
            && let Some(range) = self.range(parse.span())
        {
            self.recognized_parse_calls.insert(range);
        }
    }

    fn rewrite_macro_eof_checks(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.rewrite_macro_eof_checks(group.stream());
            }
            let TokenTree::Ident(method) = token else {
                continue;
            };
            if method != "is_empty" || index < 2 {
                continue;
            }
            let (TokenTree::Ident(receiver), TokenTree::Punct(dot)) =
                (&tokens[index - 2], &tokens[index - 1])
            else {
                continue;
            };
            let receiver = receiver.to_string();
            if dot.as_char() != '.' || !(receiver == "input" || receiver.ends_with("input")) {
                continue;
            }
            if let Some((start, end)) = self.range(method.span()) {
                self.edits.push(SpanEdit {
                    start,
                    end,
                    replacement: "is_eof".to_owned(),
                });
            }
        }
    }

    fn range(&mut self, span: Span) -> Option<(usize, usize)> {
        match (
            offset(self.source, self.line_starts, span.start()),
            offset(self.source, self.line_starts, span.end()),
        ) {
            (Some(start), Some(end)) => Some((start, end)),
            _ => {
                self.errors.push(format!(
                    "cannot map source span {:?}..{:?} to UTF-8 byte offsets",
                    span.start(),
                    span.end()
                ));
                None
            }
        }
    }
}

fn successful_parse_receiver(expression: &Expr) -> Option<&ExprCall> {
    let Expr::MethodCall(method) = expression else {
        return None;
    };
    if !matches!(
        method.method.to_string().as_str(),
        "unwrap" | "expect" | "unwrap_or_else"
    ) {
        return None;
    }
    let Expr::Call(call) = method.receiver.as_ref() else {
        return None;
    };
    is_direct_parse(call).then_some(call)
}

fn is_direct_parse(call: &ExprCall) -> bool {
    call_named(call, "parse")
        && call.args.len() == 1
        && matches!(call.args.first(), Some(Expr::Reference(reference)) if reference.mutability.is_some())
}

fn call_named(call: &ExprCall, expected: &str) -> bool {
    matches!(call.func.as_ref(), Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn is_input_expression(expression: &Expr) -> bool {
    matches!(expression, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| {
        let name = segment.ident.to_string();
        name == "input" || name.ends_with("input")
    }))
}

fn path_is_test_source(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "tests")
        || path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| stem == "test_support" || stem.starts_with("test_"))
}

fn has_cfg_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute.meta.to_token_stream().to_string() == "cfg (test)"
    })
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    starts
}

fn offset(source: &str, starts: &[usize], location: LineColumn) -> Option<usize> {
    let line = location.line.checked_sub(1)?;
    let start = *starts.get(line)?;
    let offset = start.checked_add(location.column)?;
    (offset <= source.len() && source.is_char_boundary(offset)).then_some(offset)
}

fn pass_error(path: &Path, message: String) -> RewriteError {
    RewriteError::Pass {
        path: path.to_path_buf(),
        message,
    }
}
