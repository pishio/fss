//! JSX-spread rewrite — port of the rewrite half of
//! `packages/parser/src/index.ts`.
//!
//! For every recognised `{...cas().X().props}` spread on a JSX
//! element, the visitor:
//!   1. Walks the chain to `Op[]`.
//!   2. Serialises the IR to a JSON string.
//!   3. Replaces the spread with
//!      `className={/* @cassida-ir:<JSON> */ "__CAS_PLACEHOLDER_<N>__"}`.
//!
//! `@cassida/next-plugin`'s Node loader does the second pass: regex
//! over the comment, compile via `@cassida/compiler`, swap the
//! placeholder for the resulting class name. Keeping the compiler in
//! JS lets the Babel and SWC parsing paths share a single hashing
//! source of truth.

use std::collections::HashSet;

use swc_core::common::{comments::Comments, BytePos, Span, DUMMY_SP};
use swc_core::ecma::ast::{
    Expr, ImportDecl, ImportSpecifier, JSXAttr, JSXAttrName, JSXAttrOrSpread, JSXAttrValue,
    JSXExpr, JSXExprContainer, JSXOpeningElement, Lit, Module, ModuleItem, Program, Script,
    SpreadElement, Str,
};
use swc_core::ecma::atoms::Atom;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ir::serialize_ops;
use crate::walker::{walk_chain, ChainRoots};

/// The chain-walker tags every produced JSX `className={...}` value
/// with a comment whose body is `@cassida-ir:<json>`. The Node loader
/// finds these by exact prefix.
const IR_COMMENT_PREFIX: &str = " @cassida-ir:";

/// `className` attributes carry a string literal of this exact shape;
/// the Node loader substitutes the real hash in place. The integer
/// suffix scopes each placeholder to a single file so a regex pass
/// can match them un-greedily.
const PLACEHOLDER_PREFIX: &str = "__CAS_PLACEHOLDER_";
const PLACEHOLDER_SUFFIX: &str = "__";

/// The import sources whose `cas` / `css` / `cassida` named (or
/// default) exports root a chain. Phase 1 hard-codes `@cassida/core`;
/// the Node loader will eventually forward a user-configured value
/// from `withCassida({ importSource })`.
const DEFAULT_IMPORT_SOURCE: &str = "@cassida/core";

/// Names that `@cassida/core` exports as the chain root. The runtime
/// declares all three aliases of the same builder; a local binding
/// `import { cas } from '@cassida/core'` adds `cas` to the chain
/// roots, `import { css } from '@cassida/core'` adds `css`, etc.
const ROOT_EXPORT_NAMES: &[&str] = &["cas", "css", "cassida"];

/// Walks the program. State threads through `&mut self`:
///   - `chain_roots` accumulates local names bound to a chain-root
///     export (populated by the import-decl visitor).
///   - `placeholder_counter` mints unique IDs for the className
///     literal placeholders.
///
/// `comments` is SWC's shared comments table; the visitor attaches
/// the IR-carrying comment to the placeholder span so the codegen
/// pass writes it back into the output JS verbatim.
pub struct CassidaVisitor<C: Comments> {
    pub chain_roots: ChainRoots,
    pub placeholder_counter: u32,
    pub comments: C,
}

impl<C: Comments> CassidaVisitor<C> {
    pub fn new(comments: C) -> Self {
        Self {
            chain_roots: HashSet::new(),
            placeholder_counter: 0,
            comments,
        }
    }
}

impl<C: Comments> VisitMut for CassidaVisitor<C> {
    fn visit_mut_program(&mut self, program: &mut Program) {
        // First pass: scan import decls, populate chain_roots.
        match program {
            Program::Module(module) => {
                collect_chain_roots_from_module(module, &mut self.chain_roots)
            }
            Program::Script(script) => {
                collect_chain_roots_from_script(script, &mut self.chain_roots)
            }
        }
        // Second pass: rewrite JSX. Skip if no chain root was bound;
        // the file has no Cassida calls.
        if !self.chain_roots.is_empty() {
            program.visit_mut_children_with(self);
        }
    }

    fn visit_mut_jsx_opening_element(&mut self, opening: &mut JSXOpeningElement) {
        // Descend first so any nested elements see the same rewrite
        // logic. JSX rewriting is local to each element; processing
        // order doesn't change the result.
        opening.visit_mut_children_with(self);

        // Find every `{...cas()...}` spread, walk it, replace it.
        // Multiple Cassida spreads on the same element are not allowed
        // — the Single Class Principle requires exactly one. The error
        // path lands in a follow-up commit.
        let mut new_attrs: Vec<JSXAttrOrSpread> = Vec::with_capacity(opening.attrs.len());
        for attr in std::mem::take(&mut opening.attrs) {
            match attr {
                JSXAttrOrSpread::SpreadElement(spread) => {
                    if let Some(replacement) = try_rewrite_spread(self, &spread) {
                        new_attrs.push(JSXAttrOrSpread::JSXAttr(replacement));
                    } else {
                        new_attrs.push(JSXAttrOrSpread::SpreadElement(spread));
                    }
                }
                other => new_attrs.push(other),
            }
        }
        opening.attrs = new_attrs;
    }
}

/// Try to walk the spread argument as a Cassida chain. Returns the
/// replacement `className=...` attribute on a successful walk;
/// `None` if the spread isn't a Cassida chain (leave it for the
/// runtime fallback).
fn try_rewrite_spread<C: Comments>(
    state: &mut CassidaVisitor<C>,
    spread: &SpreadElement,
) -> Option<JSXAttr> {
    let expr: &Expr = &spread.expr;
    let ops = walk_chain(expr, &state.chain_roots)?;
    let ir_json = serialize_ops(&ops);

    let counter = state.placeholder_counter;
    state.placeholder_counter += 1;

    let placeholder_value = format!("{PLACEHOLDER_PREFIX}{counter}{PLACEHOLDER_SUFFIX}");

    // Re-use the spread's `dot3_token` span — a real source position
    // already registered in the SourceMap. This satisfies the three
    // constraints in one shot:
    //   1. Unique per spread (each `...` token has its own byte range).
    //   2. Monotonically increasing in source-visit order (the parser
    //      assigns positions left-to-right, top-to-bottom).
    //   3. In-bounds — synthetic out-of-source BytePos values can
    //      panic downstream sourcemap consumers.
    // SWC's `emit_leading_comments` drains every pending comment
    // whose BytePos is `<= current_node.lo`; the monotonic property
    // is what keeps each placeholder's IR comment paired with its
    // own placeholder literal.
    //
    // Synthetic ASTs from other plugins / macros sometimes carry
    // `DUMMY_SP` for inserted nodes. Falling through with a dummy
    // span would land every placeholder on `BytePos(0)` and merge
    // all the IR comments together — fall back to a high-range
    // synthetic position keyed by counter (still monotonic) in
    // that case.
    let (placeholder_span, placeholder_lo) = if spread.dot3_token.is_dummy() {
        let lo = BytePos(0x8000_0000u32.saturating_add(counter));
        (Span::new(lo, lo), lo)
    } else {
        (spread.dot3_token, spread.dot3_token.lo)
    };
    let placeholder_lit = Str {
        span: placeholder_span,
        // `Str::value` is a Wtf8Atom; convert from the regular Atom
        // via `Into` since our string is plain ASCII.
        value: Atom::from(placeholder_value.as_str()).into(),
        raw: None,
    };

    // Attach the IR as a leading block comment anchored at the
    // placeholder's BytePos. SWC's codegen writes block comments
    // back inline before the token they precede, which gives us
    // `className={/* @cassida-ir:[...] */ "__CAS_PLACEHOLDER_0__"}`.
    state.comments.add_leading(
        placeholder_lo,
        swc_core::common::comments::Comment {
            kind: swc_core::common::comments::CommentKind::Block,
            span: DUMMY_SP,
            text: format!("{IR_COMMENT_PREFIX}{ir_json}").into(),
        },
    );

    Some(JSXAttr {
        span: DUMMY_SP,
        name: JSXAttrName::Ident(swc_core::ecma::ast::IdentName {
            span: DUMMY_SP,
            sym: Atom::from("className"),
        }),
        value: Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
            span: DUMMY_SP,
            expr: JSXExpr::Expr(Box::new(Expr::Lit(Lit::Str(placeholder_lit)))),
        })),
    })
}

/// Scan top-level `ImportDeclaration`s for bindings to `@cassida/core`'s
/// root exports. Each local binding name (default, named, or aliased)
/// becomes a chain root.
fn collect_chain_roots_from_module(module: &Module, roots: &mut ChainRoots) {
    for item in &module.body {
        if let ModuleItem::ModuleDecl(swc_core::ecma::ast::ModuleDecl::Import(decl)) = item {
            collect_chain_roots_from_import(decl, roots);
        }
    }
}

fn collect_chain_roots_from_script(_script: &Script, _roots: &mut ChainRoots) {
    // ESM-only world; non-module scripts can't import the chain root.
    // Reserved for future support of CommonJS via `require(...)` if
    // we ever expand scope.
}

fn collect_chain_roots_from_import(decl: &ImportDecl, roots: &mut ChainRoots) {
    // `import type { ... } from '...'` is erased at runtime — even if
    // the user wrote `import type { cas } from '@cassida/core'`, the
    // identifier doesn't exist when JSX runs. Bail before scanning
    // specifiers so we don't pollute `chain_roots` with type-only
    // names.
    if decl.type_only {
        return;
    }
    if decl.src.value.as_str() != Some(DEFAULT_IMPORT_SOURCE) {
        return;
    }
    for spec in &decl.specifiers {
        match spec {
            ImportSpecifier::Default(default) => {
                // `import cas from '@cassida/core'` binds the default
                // export. The runtime's default export is the same
                // builder as the named `cas` export. Atom clone (no
                // allocation — SWC's atoms are interned).
                roots.insert(default.local.sym.clone());
            }
            ImportSpecifier::Named(named) => {
                // Inline `import { type cas }` is also runtime-erased.
                if named.is_type_only {
                    continue;
                }
                // `imported_name` is just compared to a static `&str`
                // list, so keep it as a borrow — no per-import String
                // allocation.
                let imported_name: &str = match &named.imported {
                    Some(imported) => match imported {
                        swc_core::ecma::ast::ModuleExportName::Ident(id) => id.sym.as_str(),
                        swc_core::ecma::ast::ModuleExportName::Str(s) => match s.value.as_str() {
                            Some(v) => v,
                            None => continue,
                        },
                    },
                    None => named.local.sym.as_str(),
                };
                if ROOT_EXPORT_NAMES.contains(&imported_name) {
                    roots.insert(named.local.sym.clone());
                }
            }
            ImportSpecifier::Namespace(_) => {
                // `import * as ns from '@cassida/core'` requires
                // member-access detection (`ns.cas(...)`) which Phase
                // 1 doesn't cover. Skip.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::{comments::SingleThreadedComments, sync::Lrc, FileName, SourceMap};
    use swc_core::ecma::ast::EsVersion;
    use swc_core::ecma::codegen::{text_writer::JsWriter, Config, Emitter};
    use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
    use swc_core::ecma::visit::VisitMutWith;

    fn transform(source: &str) -> String {
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(Lrc::new(FileName::Anon), source.to_string());
        let comments: SingleThreadedComments = Default::default();
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax {
                tsx: true,
                ..Default::default()
            }),
            EsVersion::EsNext,
            StringInput::from(&*fm),
            Some(&comments),
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("parse failed");
        let mut program = Program::Module(module);

        let mut visitor = CassidaVisitor::new(comments.clone());
        program.visit_mut_with(&mut visitor);

        let mut out = Vec::new();
        {
            let mut emitter = Emitter {
                cfg: Config::default(),
                cm: cm.clone(),
                comments: Some(&comments),
                wr: JsWriter::new(cm, "\n", &mut out, None),
            };
            emitter.emit_program(&program).expect("emit failed");
        }
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn rewrites_single_chain_spread_to_classname_with_ir_comment() {
        let out = transform(
            r#"
import { cas } from '@cassida/core';
const X = () => <div {...cas().color('red').props} />;
            "#,
        );
        assert!(
            out.contains(r#"className={/* @cassida-ir:[{"method":"color","args":["red"]}]*/ "__CAS_PLACEHOLDER_0__"}"#),
            "output was:\n{out}"
        );
    }

    #[test]
    fn leaves_unrelated_jsx_spreads_alone() {
        let out = transform(
            r#"
const X = (props) => <div {...props} />;
            "#,
        );
        assert!(out.contains("{...props}"), "output was:\n{out}");
    }

    #[test]
    fn leaves_jsx_when_no_cassida_import_present() {
        // Without an import from @cassida/core, the chain-root set is
        // empty and the visitor short-circuits — even an expression
        // that *looks* like cas() is left alone.
        let out = transform(
            r#"
const cas = () => ({});
const X = () => <div {...cas().color('red')} />;
            "#,
        );
        assert!(out.contains("{...cas()"), "output was:\n{out}");
    }

    #[test]
    fn aliased_named_import_works_as_chain_root() {
        let out = transform(
            r#"
import { cas as styled } from '@cassida/core';
const X = () => <div {...styled().color('blue').props} />;
            "#,
        );
        assert!(
            out.contains(r#"@cassida-ir:[{"method":"color","args":["blue"]}]"#),
            "output was:\n{out}"
        );
        assert!(out.contains("__CAS_PLACEHOLDER_0__"));
    }

    #[test]
    fn default_import_works_as_chain_root() {
        let out = transform(
            r#"
import cas from '@cassida/core';
const X = () => <div {...cas().color('green').props} />;
            "#,
        );
        assert!(
            out.contains(r#"@cassida-ir:[{"method":"color","args":["green"]}]"#),
            "output was:\n{out}"
        );
    }

    #[test]
    fn modifier_scope_serialises_in_ir() {
        let out = transform(
            r#"
import { cas } from '@cassida/core';
const X = () => <div {...cas().hover(c => c.color('red')).props} />;
            "#,
        );
        assert!(
            out.contains(
                r#"@cassida-ir:[{"scope":{"kind":"pseudo","selector":":hover"},"ops":[{"method":"color","args":["red"]}]}]"#
            ),
            "output was:\n{out}"
        );
    }

    #[test]
    fn before_pseudo_element_serialises_in_ir() {
        let out = transform(
            r#"
import { cas } from '@cassida/core';
const X = () => <div {...cas().before(c => c.content('"."')).props} />;
            "#,
        );
        assert!(
            out.contains(r#""selector":"::before""#),
            "output was:\n{out}"
        );
    }

    /// Type-only imports don't exist at runtime; even if a user
    /// writes `import type { cas } from '@cassida/core'`, the
    /// resulting JSX still uses some other binding for `cas` (or
    /// none — TS would have erased the import entirely). The
    /// visitor must not register the type-only name as a chain root.
    #[test]
    fn type_only_imports_are_ignored() {
        let out = transform(
            r#"
import type { cas } from '@cassida/core';
const cas = () => ({});
const X = () => <div {...cas().color('red')} />;
            "#,
        );
        // No chain root → spread left alone, no IR comment emitted.
        assert!(out.contains("{...cas()"), "output was:\n{out}");
        assert!(
            !out.contains("@cassida-ir"),
            "type-only import should not register a chain root; output was:\n{out}"
        );
    }

    /// Inline `import { type cas } from '@cassida/core'` should be
    /// skipped at the specifier level even when the import decl
    /// itself isn't type-only overall.
    #[test]
    fn inline_type_specifier_is_ignored() {
        let out = transform(
            r#"
import { type cas, default as runtime } from '@cassida/core';
const X = () => <div {...cas().color('red')} />;
            "#,
        );
        assert!(out.contains("{...cas()"), "output was:\n{out}");
        assert!(
            !out.contains("@cassida-ir"),
            "inline `type` specifier should be ignored; output was:\n{out}"
        );
    }

    #[test]
    fn multiple_spreads_get_distinct_placeholder_indices() {
        let out = transform(
            r#"
import { cas } from '@cassida/core';
const X = () => (
  <div>
    <span {...cas().color('red').props} />
    <span {...cas().color('blue').props} />
  </div>
);
            "#,
        );
        assert!(
            out.contains(r#""__CAS_PLACEHOLDER_0__""#)
                && out.contains(r#""__CAS_PLACEHOLDER_1__""#),
            "output was:\n{out}"
        );
    }

    /// Each placeholder must keep its OWN IR comment attached.
    /// SWC's comment emitter drains all comments with BytePos <=
    /// node.lo, so synthetic placeholder positions have to ascend
    /// with the source-order visit. A previous revision used
    /// `u32::MAX - counter` (descending) and lost every comment
    /// past the first — this test guards against that regression.
    #[test]
    fn each_placeholder_carries_its_own_ir_comment() {
        let out = transform(
            r#"
import { cas } from '@cassida/core';
const X = () => (
  <div>
    <span {...cas().color('red').props} />
    <span {...cas().color('blue').props} />
  </div>
);
            "#,
        );
        // The red IR must appear adjacent to placeholder 0.
        assert!(
            out.contains(
                r#"@cassida-ir:[{"method":"color","args":["red"]}]*/ "__CAS_PLACEHOLDER_0__""#
            ),
            "placeholder 0 should carry its red IR; output was:\n{out}"
        );
        // The blue IR must appear adjacent to placeholder 1.
        assert!(
            out.contains(
                r#"@cassida-ir:[{"method":"color","args":["blue"]}]*/ "__CAS_PLACEHOLDER_1__""#
            ),
            "placeholder 1 should carry its blue IR; output was:\n{out}"
        );
    }
}
