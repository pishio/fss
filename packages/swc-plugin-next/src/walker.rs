//! Chain walker — Rust port of `walkChain` from
//! `packages/parser/src/index.ts`. Walks a chain expression
//! (outermost CallExpr / MemberExpr back to the `cas()` root) and
//! produces the `Op[]` IR.
//!
//! Phase 1 scope:
//!   - Method calls with literal args (`Lit::Str`, `Lit::Num`,
//!     `Lit::Bool`, `Lit::Null`, and `Tpl` without substitutions)
//!   - `.set(prop, value)` — kebab-cased property write (RawOp)
//!   - Canonical modifier scopes (`.hover(c => ...)` etc.)
//!   - Arg modifier scopes (`.media('q', c => ...)` /
//!     `.on('sel', c => ...)`)
//!   - Trailing `.props` is peeled before walking
//!
//! Out of scope for Phase 1 (returns `None` so the host JSX spread
//! falls back to the runtime path):
//!   - Non-literal args (Phase 2 dynamic-args)
//!   - `.cond(test, t, f?)` (Phase 2 Cartesian)
//!   - `.unsafe(preset)` (preset-expansion logic)
//!   - Block-body callbacks (`.hover(c => { c.x(); c.y(); })`)
//!   - Function composition (`withCard(cas())`)
//!   - Cross-file static eval (Phase 3)

use std::collections::HashSet;

use swc_core::ecma::ast::{
    ArrowExpr, BindingIdent, BlockStmtOrExpr, CallExpr, Callee, Expr, Lit, MemberExpr, MemberProp,
    Pat, Tpl,
};
use swc_core::ecma::atoms::Atom;

#[cfg(test)]
use crate::ir::Scope;
use crate::ir::{MethodOp, Op, RawOp, ScopedOp};
use crate::modifiers::{arg_modifier, arg_modifier_scope, canonical_scope};

/// Names of root chain identifiers — i.e. local bindings that resolve
/// to the `cas` (or `css` / `cassida`) export of `@cassida/core`. The
/// visitor populates this from `ImportDeclaration` scanning.
///
/// Keyed by `Atom` rather than `String` so insertions and lookups
/// skip a heap allocation per identifier. SWC's interning makes
/// `Atom` clones cheap; the AST already carries the same `Atom`
/// instances on identifier nodes, so the lookup site (`ident.sym`)
/// hits the table directly via reference.
pub type ChainRoots = HashSet<Atom>;

/// Walk a chain root expression. Returns `Some(Vec<Op>)` on a fully
/// recognised chain or `None` if any node in the chain is unsupported
/// — in the latter case the host JSX spread should be left for the
/// runtime fallback.
///
/// `expr` is typically the argument of a JSX `{...<expr>}` spread,
/// after the trailing `.props` member access has been peeled.
pub fn walk_chain(expr: &Expr, roots: &ChainRoots) -> Option<Vec<Op>> {
    let mut ops: Vec<Op> = Vec::new();
    let mut cursor = peel_props(expr);
    // `first` distinguishes the chain root from intermediate steps.
    // A bare identifier (e.g. `{...cas}` — the user forgot the `()`)
    // would otherwise be accepted as an empty chain on the very
    // first iteration and silently rewritten to a placeholder. We
    // require either a `cas()` call or at least one method step
    // before a bare callback-param Ident is allowed.
    let mut first = true;
    loop {
        cursor = peel_parens(cursor);
        match cursor {
            Expr::Call(call) => {
                first = false;
                match step_call(call, roots, &mut ops)? {
                    StepOutcome::Continue(next) => cursor = next,
                    StepOutcome::Root => break,
                }
            }
            // Bare-identifier root — the callback-param case: a chain
            // body like `c.color('red')` walks back to just `c`, which
            // is the callback's parameter and lives in `roots`. Only
            // valid when at least one method step has been consumed.
            Expr::Ident(ident) if !first && roots.contains(&ident.sym) => break,
            _ => return None,
        }
    }
    // Walked outer → inner; reverse to source order.
    ops.reverse();
    Some(ops)
}

fn peel_parens(mut expr: &Expr) -> &Expr {
    while let Expr::Paren(p) = expr {
        expr = &p.expr;
    }
    expr
}

/// Result of processing one CallExpr in the chain walk.
enum StepOutcome<'a> {
    /// Recognised a method call; continue from the receiver expression.
    Continue(&'a Expr),
    /// Recognised the `cas()` (or alias) root call — chain walk done.
    Root,
}

fn step_call<'a>(
    call: &'a CallExpr,
    roots: &ChainRoots,
    ops: &mut Vec<Op>,
) -> Option<StepOutcome<'a>> {
    match &call.callee {
        Callee::Expr(callee) => match &**callee {
            // `obj.method(...args)` — the "method" branch.
            Expr::Member(member) => step_method(member, call, roots, ops),
            // `cas()` — root call without member access.
            Expr::Ident(ident) if roots.contains(&ident.sym) => {
                // `cas()` accepts an optional preset arg in TS; Phase 1
                // only handles the zero-arg form. Preset support is a
                // later Phase 1 follow-up.
                if call.args.is_empty() {
                    Some(StepOutcome::Root)
                } else {
                    None
                }
            }
            _ => None,
        },
        // Super / import calls aren't chain shapes.
        _ => None,
    }
}

/// Handle one `obj.method(args)` step. Pushes an Op into `ops` and
/// returns the receiver expression to continue from. Returns `None`
/// for any shape Phase 1 doesn't recognise.
fn step_method<'a>(
    member: &'a MemberExpr,
    call: &'a CallExpr,
    roots: &ChainRoots,
    ops: &mut Vec<Op>,
) -> Option<StepOutcome<'a>> {
    let method_name = member_ident(&member.prop)?;
    let args: Vec<&Expr> = call
        .args
        .iter()
        .map(|arg| {
            // Spread args (`...rest`) aren't supported in chain method
            // calls. Bail.
            if arg.spread.is_some() {
                None
            } else {
                // Peel parens so `cas().color(('red'))` doesn't bail
                // — Babel's `path.evaluate()` walks past parenthesised
                // wrappers transparently, and we mirror that for
                // Babel/SWC parity.
                Some(peel_parens(&arg.expr))
            }
        })
        .collect::<Option<_>>()?;

    // 1. `.set(prop, value)` — RawOp, bypasses the registry.
    if method_name == "set" {
        if args.len() != 2 {
            return None;
        }
        let prop = literal_string(args[0])?;
        let value = literal_string_or_number(args[1])?;
        ops.push(Op::Raw(RawOp {
            property: camel_to_kebab(&prop),
            value,
        }));
        return Some(StepOutcome::Continue(&member.obj));
    }

    // 2. Canonical zero-arg modifier (`.hover(c => ...)` etc.) — the
    // single arg is an arrow function with one param.
    if let Some(scope) = canonical_scope(method_name) {
        if args.len() != 1 {
            return None;
        }
        let inner = walk_callback(args[0], roots)?;
        ops.push(Op::Scoped(ScopedOp { scope, ops: inner }));
        return Some(StepOutcome::Continue(&member.obj));
    }

    // 3. Arg modifier (`.media('q', c => ...)` / `.on('s', c => ...)`).
    if let Some(arg_mod) = arg_modifier(method_name) {
        if args.len() != 2 {
            return None;
        }
        let arg_value = literal_string(args[0])?;
        let inner = walk_callback(args[1], roots)?;
        ops.push(Op::Scoped(ScopedOp {
            scope: arg_modifier_scope(arg_mod, &arg_value),
            ops: inner,
        }));
        return Some(StepOutcome::Continue(&member.obj));
    }

    // 4. Plain method op with literal args. Any arg that isn't
    // literal-evaluable bails Phase 1.
    let mut json_args: Vec<serde_json::Value> = Vec::with_capacity(args.len());
    for arg in args {
        let v = literal_to_json(arg)?;
        json_args.push(v);
    }
    ops.push(Op::Method(MethodOp {
        // `method_name` is borrowed from the AST; allocate once at
        // capture into the long-lived IR.
        method: method_name.to_string(),
        args: json_args,
    }));
    Some(StepOutcome::Continue(&member.obj))
}

/// Strip leading parens and a trailing `.props` access so the rest
/// of the walker only deals with the call-chain. `(cas().X().props)`,
/// `cas().X().props`, and `cas().X()` all reduce to the same starting
/// `Expr::Call` here.
fn peel_props(expr: &Expr) -> &Expr {
    let mut cursor = expr;
    // Strip any number of parenthesised wrappers — the Babel parser
    // path does the same, and the SWC parser hands them to us
    // verbatim when JSX or test fixtures wrap in extra parens.
    while let Expr::Paren(p) = cursor {
        cursor = &p.expr;
    }
    if let Expr::Member(member) = cursor {
        if let MemberProp::Ident(ident) = &member.prop {
            if ident.sym.as_str() == "props" {
                cursor = &member.obj;
                while let Expr::Paren(p) = cursor {
                    cursor = &p.expr;
                }
            }
        }
    }
    cursor
}

/// Extract the `name` of an `obj.<name>` member access. Returns `None`
/// for computed accesses (`obj[expr]`) or non-identifier props.
///
/// Returns a borrowed `&str` so the chain-walk hot path doesn't
/// allocate per step — only the final `MethodOp.method` field
/// allocates, at the moment the op is captured into the IR.
fn member_ident(prop: &MemberProp) -> Option<&str> {
    match prop {
        MemberProp::Ident(ident) => Some(ident.sym.as_str()),
        _ => None,
    }
}

/// Resolve an arrow-function callback to the Op[] of its body chain.
///
/// Phase 1 only handles expression-body arrows with a single
/// identifier param. Block bodies and rest-param shapes return `None`.
///
/// The inner walk uses a FRESH root set containing ONLY the callback
/// parameter — outer roots (`cas`) are deliberately NOT inherited.
/// This mirrors `collectFromCallback` in
/// `packages/parser/src/index.ts:1269` (which constructs
/// `new Set([param.name])`). Inheriting the outer roots would let
/// nested-`cas()` shapes like `cas().hover(c => cas().color('red'))`
/// compile under SWC while still bailing under Babel — silently
/// producing different class hashes across the two pipelines.
fn walk_callback(expr: &Expr, _outer_roots: &ChainRoots) -> Option<Vec<Op>> {
    let arrow = match expr {
        Expr::Arrow(a) => a,
        _ => return None,
    };
    let inner_root_name = arrow_param_root(arrow)?;
    let mut inner_roots = ChainRoots::new();
    inner_roots.insert(inner_root_name);
    match &*arrow.body {
        BlockStmtOrExpr::Expr(body) => walk_chain(body, &inner_roots),
        // Block bodies (`.hover(c => { c.x(); c.y(); })`) are a
        // follow-up — they require recursing into each ExpressionStatement
        // / ReturnStatement separately. Phase 1 declines for now.
        BlockStmtOrExpr::BlockStmt(_) => None,
    }
}

/// The arrow's single parameter, as the local binding name to use
/// when matching chain roots inside the body. Phase 1 requires
/// exactly one Ident-pattern parameter. Returns a clone of the SWC
/// `Atom` so the new inner-root set can own it without allocating a
/// `String`.
fn arrow_param_root(arrow: &ArrowExpr) -> Option<Atom> {
    if arrow.params.len() != 1 {
        return None;
    }
    match &arrow.params[0] {
        Pat::Ident(BindingIdent { id, .. }) => Some(id.sym.clone()),
        _ => None,
    }
}

/// Extract a string from a literal expression — either `'foo'` or
/// a substitution-free `` `foo` ``. `Wtf8Atom::as_str` returns `None`
/// on lone surrogates; we bail in that case (caller falls back to the
/// runtime path).
fn literal_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(Lit::Str(s)) => s.value.as_str().map(String::from),
        Expr::Tpl(tpl) => tpl_to_static_string(tpl),
        _ => None,
    }
}

/// `.set(key, value)` accepts a stringy value or a number that the
/// runtime stringifies — mirror the Babel parser's behaviour.
///
/// For numeric values, route whole numbers inside the JS safe-integer
/// range through `(_ as i64).to_string()` so the output matches the
/// JS path's `Number.prototype.toString` (e.g. `8` not `8.0`,
/// matching `(8).toString()`). Out-of-range or fractional values fall
/// back to `f64::to_string()` — same loss-of-precision territory
/// where `JSON.stringify` itself isn't byte-stable either.
fn literal_string_or_number(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(Lit::Str(s)) => s.value.as_str().map(String::from),
        Expr::Tpl(tpl) => tpl_to_static_string(tpl),
        Expr::Lit(Lit::Num(n)) => {
            let v = n.value;
            if v.is_finite()
                && v.fract() == 0.0
                && (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&v)
            {
                Some((v as i64).to_string())
            } else {
                Some(v.to_string())
            }
        }
        _ => None,
    }
}

/// Resolve a template literal to its static string content when it
/// has no embedded substitutions. Returns `None` for templates with
/// `${...}` expressions — those are dynamic and Phase 1 declines.
///
/// Only uses the `cooked` form (the value JS would produce at
/// runtime). Falling back to `raw` would smuggle literal escape
/// sequences like `\uD800` into the IR — Babel's path uses the
/// evaluated `cooked` value, so a raw fallback here would silently
/// produce different class hashes between the two compilation
/// pipelines. Cooked unavailable (parse-time invalid escapes) or
/// non-UTF-8 (lone surrogates) → bail, host JSX falls back to the
/// runtime path.
fn tpl_to_static_string(tpl: &Tpl) -> Option<String> {
    if !tpl.exprs.is_empty() || tpl.quasis.len() != 1 {
        return None;
    }
    let cooked = tpl.quasis[0].cooked.as_ref()?;
    cooked.as_str().map(String::from)
}

/// Project a literal AST node into a JSON value matching the
/// argument as it would appear in the TS Op[] shape. `None` for
/// anything Phase 1 can't statically fold.
fn literal_to_json(expr: &Expr) -> Option<serde_json::Value> {
    match expr {
        Expr::Lit(Lit::Str(s)) => s
            .value
            .as_str()
            .map(|s| serde_json::Value::String(s.to_string())),
        Expr::Lit(Lit::Num(n)) => Some(serde_json::Value::Number(num_to_json(n.value)?)),
        Expr::Lit(Lit::Bool(b)) => Some(serde_json::Value::Bool(b.value)),
        Expr::Lit(Lit::Null(_)) => Some(serde_json::Value::Null),
        Expr::Tpl(tpl) => tpl_to_static_string(tpl).map(serde_json::Value::String),
        // Unary expressions on literal operands: Babel's `path.evaluate()`
        // folds `-42`, `+42`, and `!true` into their evaluated form. We
        // mirror that here so equivalent source spellings don't bail
        // under SWC while compiling under Babel. Peel parens from the
        // arg first so `-(8)` / `!(true)` also fold.
        Expr::Unary(u) => {
            let arg = peel_parens(&u.arg);
            match u.op {
                swc_core::ecma::ast::UnaryOp::Minus => {
                    if let Expr::Lit(Lit::Num(n)) = arg {
                        Some(serde_json::Value::Number(num_to_json(-n.value)?))
                    } else {
                        None
                    }
                }
                swc_core::ecma::ast::UnaryOp::Plus => {
                    if let Expr::Lit(Lit::Num(n)) = arg {
                        Some(serde_json::Value::Number(num_to_json(n.value)?))
                    } else {
                        None
                    }
                }
                swc_core::ecma::ast::UnaryOp::Bang => {
                    if let Expr::Lit(Lit::Bool(b)) = arg {
                        Some(serde_json::Value::Bool(!b.value))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Match `JSON.stringify` semantics for numeric round-trip: whole
/// f64 values fit through `Number::from(i64)` (serialise as `8`, not
/// `8.0`); fractional values stay floats. NaN / Inf bail because TS
/// JSON literals also can't carry them.
///
/// The integer fast-path is gated on the JavaScript safe-integer
/// range (`[-2^53 + 1, 2^53 - 1]`). Outside that range f64 can no
/// longer represent consecutive integers, and Rust's `f64 as i64`
/// saturates at `i64::MAX` while `i64::MAX as f64` rounds up — so
/// `2^63` would `==`-compare equal to `i64::MAX` and silently
/// serialise as `2^63 - 1`. JS itself wouldn't preserve those
/// integers either; keeping the path bounded by `MAX_SAFE_INTEGER`
/// is the only way to stay in sync with `JSON.stringify`.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0; // 2^53 - 1
const MIN_SAFE_INTEGER: f64 = -9_007_199_254_740_991.0;

fn num_to_json(v: f64) -> Option<serde_json::Number> {
    if v.is_finite() && v.fract() == 0.0 && (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&v) {
        return Some(serde_json::Number::from(v as i64));
    }
    serde_json::Number::from_f64(v)
}

/// `paddingTop` → `padding-top`. Mirrors `camelToKebab` in
/// `packages/parser/src/index.ts:1457`:
///
///   - If the input already contains a `-`, it's treated as already
///     kebab (or vendor-prefixed like `-webkit-foo`) and returned
///     unchanged.
///   - Otherwise, every uppercase letter — *including the first one*
///     — is replaced with a leading `-` plus its lowercase form,
///     so `WebkitBorderRadius` → `-webkit-border-radius`.
///
/// Diverging from this would silently produce different class hashes
/// in the Babel vs SWC paths.
fn camel_to_kebab(name: &str) -> String {
    if name.contains('-') {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::{sync::Lrc, FileName, SourceMap};
    use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

    fn parse_expr(source: &str) -> Expr {
        // Wrap with `;` so the parser produces an ExpressionStatement
        // we can pull the inner Expr out of.
        let cm: Lrc<SourceMap> = Default::default();
        let fm = cm.new_source_file(Lrc::new(FileName::Anon), format!("({source});"));
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax {
                tsx: true,
                ..Default::default()
            }),
            Default::default(),
            StringInput::from(&*fm),
            None,
        );
        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().expect("parse failed");
        let first = module.body.into_iter().next().expect("empty module");
        let stmt = first.expect_stmt();
        *stmt.expect_expr().expr
    }

    fn cas_roots() -> ChainRoots {
        let mut r = ChainRoots::new();
        r.insert("cas".into());
        r
    }

    #[test]
    fn single_method_with_string_arg() {
        let expr = parse_expr("cas().color('red')");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Method(MethodOp {
                method: "color".into(),
                args: vec![serde_json::Value::String("red".into())],
            })]
        );
    }

    #[test]
    fn ops_are_in_source_order() {
        let expr = parse_expr("cas().color('red').padding(8)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![
                Op::Method(MethodOp {
                    method: "color".into(),
                    args: vec![serde_json::Value::String("red".into())],
                }),
                Op::Method(MethodOp {
                    method: "padding".into(),
                    args: vec![serde_json::Value::Number(serde_json::Number::from(8))],
                }),
            ]
        );
    }

    #[test]
    fn props_terminator_is_peeled() {
        let with_props = walk_chain(&parse_expr("cas().color('red').props"), &cas_roots());
        let without_props = walk_chain(&parse_expr("cas().color('red')"), &cas_roots());
        assert_eq!(with_props, without_props);
        assert!(with_props.is_some());
    }

    #[test]
    fn hover_modifier_wraps_inner_chain_in_pseudo_scope() {
        let expr = parse_expr("cas().hover(c => c.color('red'))");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Scoped(ScopedOp {
                scope: Scope::Pseudo {
                    selector: ":hover".into()
                },
                ops: vec![Op::Method(MethodOp {
                    method: "color".into(),
                    args: vec![serde_json::Value::String("red".into())],
                })],
            })]
        );
    }

    #[test]
    fn before_pseudo_element_wraps_content() {
        let expr = parse_expr(r#"cas().before(c => c.content('""'))"#);
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Scoped(ScopedOp {
                scope: Scope::Pseudo {
                    selector: "::before".into()
                },
                ops: vec![Op::Method(MethodOp {
                    method: "content".into(),
                    args: vec![serde_json::Value::String("\"\"".into())],
                })],
            })]
        );
    }

    #[test]
    fn media_arg_modifier_strips_at_media_and_builds_media_scope() {
        let expr = parse_expr("cas().media('(min-width: 640px)', c => c.padding(16))");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Scoped(ScopedOp { scope: Scope::Media { query }, .. })) if query == "(min-width: 640px)"
        ));
    }

    #[test]
    fn on_with_pseudo_arg_routes_to_pseudo_scope() {
        let expr = parse_expr(r#"cas().on(':hover', c => c.color('red'))"#);
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Scoped(ScopedOp { scope: Scope::Pseudo { selector }, .. })) if selector == ":hover"
        ));
    }

    #[test]
    fn set_emits_a_raw_op_with_kebab_property() {
        let expr = parse_expr(r#"cas().set('paddingTop', '10px')"#);
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Raw(RawOp {
                property: "padding-top".into(),
                value: "10px".into(),
            })]
        );
    }

    /// Match the Babel parser's behaviour: an uppercase first
    /// character also gets the leading hyphen, so
    /// `WebkitBorderRadius` produces the proper vendor-prefixed
    /// CSS property `-webkit-border-radius`. Diverging here would
    /// drift class hashes between the two compilation paths.
    #[test]
    fn set_prefixes_uppercase_first_character_with_hyphen() {
        let expr = parse_expr(r#"cas().set('WebkitBorderRadius', '4px')"#);
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Raw(RawOp {
                property: "-webkit-border-radius".into(),
                value: "4px".into(),
            })]
        );
    }

    /// A property that already contains a hyphen (the user typed the
    /// kebab form directly, or it's a vendor-prefixed name) passes
    /// through unchanged. Same rule the Babel parser applies.
    #[test]
    fn set_leaves_already_kebab_property_alone() {
        let expr = parse_expr(r#"cas().set('-webkit-tap-highlight-color', 'transparent')"#);
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert_eq!(
            ops,
            vec![Op::Raw(RawOp {
                property: "-webkit-tap-highlight-color".into(),
                value: "transparent".into(),
            })]
        );
    }

    #[test]
    fn dynamic_arg_bails_to_none() {
        let expr = parse_expr("cas().color(theme.brand)");
        assert!(walk_chain(&expr, &cas_roots()).is_none());
    }

    /// `{...cas}` (the user forgot the `()`) used to be accepted as
    /// an empty chain because the loop matched the Ident-root branch
    /// on the first iteration and broke immediately. Reject bare
    /// identifiers so the spread falls through to runtime and the
    /// chain never gets a className placeholder.
    #[test]
    fn bare_chain_root_identifier_without_call_bails() {
        let expr = parse_expr("cas");
        assert!(walk_chain(&expr, &cas_roots()).is_none());
    }

    #[test]
    fn unknown_chain_root_identifier_bails() {
        let expr = parse_expr("notCas().color('red')");
        assert!(walk_chain(&expr, &cas_roots()).is_none());
    }

    #[test]
    fn aliased_root_via_chain_roots_set_works() {
        let mut roots = ChainRoots::new();
        roots.insert("styled".into());
        let expr = parse_expr("styled().color('red')");
        let ops = walk_chain(&expr, &roots).expect("recognised");
        assert_eq!(ops.len(), 1);
    }

    /// Parity guard: Babel's `path.evaluate()` resolves
    /// parenthesised expressions transparently. The walker must too,
    /// or chains with redundant parens around args (rare but legal)
    /// would silently bail under SWC while compiling under Babel.
    #[test]
    fn args_with_redundant_parens_resolve() {
        let expr = parse_expr("cas().color(('red'))");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { method, args })) if method == "color" && args[0] == serde_json::Value::String("red".into())
        ));
    }

    #[test]
    fn template_literal_without_substitutions_is_a_string() {
        let expr = parse_expr("cas().color(`hsl(0 0% 0%)`)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { method, args })) if method == "color" && args[0] == serde_json::Value::String("hsl(0 0% 0%)".into())
        ));
    }

    #[test]
    fn template_literal_with_substitution_bails() {
        let expr = parse_expr("cas().color(`hsl(${h} 0% 0%)`)");
        assert!(walk_chain(&expr, &cas_roots()).is_none());
    }

    /// Beyond the JS safe-integer range (2^53 - 1), f64 can no
    /// longer represent consecutive integers. JS's `JSON.stringify`
    /// would also lose precision here, so we follow suit and keep
    /// the value as a float instead of letting `f64 as i64` saturate
    /// at `i64::MAX` — which would silently rewrite the user's
    /// number.
    #[test]
    fn unsafe_integer_arg_stays_as_float() {
        // 2^53 = 9007199254740992 — one past the safe range.
        let expr = parse_expr("cas().zIndex(9007199254740992)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        let arg = match &ops[0] {
            Op::Method(m) => m.args[0].clone(),
            _ => panic!("expected MethodOp"),
        };
        // serde_json::Number::from(i64) and Number::from_f64 hash
        // differently; what we want is the float branch (so we
        // don't truncate to i64::MAX or any other lossy form).
        assert!(
            arg.is_f64(),
            "expected float branch for unsafe integer, got {arg:?}"
        );
    }

    /// `+42` parses as `UnaryExpr(Plus, NumericLit(42))` — Babel's
    /// `path.evaluate()` folds it to `42`, and we mirror that.
    #[test]
    fn unary_plus_on_literal_folds() {
        let expr = parse_expr("cas().fontSize(+14)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { args, .. })) if args[0] == serde_json::Value::Number(serde_json::Number::from(14))
        ));
    }

    /// Parens inside the unary operand (`-(8)`, `!(true)`) must also
    /// fold — Babel's `path.evaluate()` walks past them transparently.
    #[test]
    fn unary_on_parenthesised_literal_folds() {
        let neg = parse_expr("cas().marginTop(-(8))");
        let ops = walk_chain(&neg, &cas_roots()).expect("neg recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { args, .. })) if args[0] == serde_json::Value::Number(serde_json::Number::from(-8))
        ));

        let bang = parse_expr("cas().userSelect(!(true))");
        let ops = walk_chain(&bang, &cas_roots()).expect("bang recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { args, .. })) if args[0] == serde_json::Value::Bool(false)
        ));
    }

    /// `!true` / `!false` fold to their boolean negation — same
    /// Babel parity, so a chain whose method takes a boolean
    /// argument (`.someFlag(!true)`) doesn't bail. The method name
    /// used here is unimportant — the walker only emits IR; the
    /// canonicalizer validates against the registry downstream.
    #[test]
    fn unary_bang_on_literal_bool_folds() {
        let expr = parse_expr("cas().userSelect(!true)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { args, .. })) if args[0] == serde_json::Value::Bool(false)
        ));
    }

    #[test]
    fn negative_number_arg_is_supported() {
        let expr = parse_expr("cas().marginTop(-8)");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        assert!(matches!(
            ops.first(),
            Some(Op::Method(MethodOp { args, .. })) if args[0] == serde_json::Value::Number(serde_json::Number::from(-8))
        ));
    }

    /// Parity guard: inside a modifier callback the outer chain root
    /// (`cas`) must NOT be in scope. Babel's `collectFromCallback`
    /// uses a fresh `Set([param.name])`. If the Rust walker leaked
    /// `cas` into the inner set, the chain below would compile to
    /// classes under SWC while Babel bails — class-hash divergence
    /// across the two parsing paths.
    #[test]
    fn nested_cas_call_inside_callback_bails() {
        let expr = parse_expr("cas().hover(c => cas().color('red'))");
        assert!(walk_chain(&expr, &cas_roots()).is_none());
    }

    /// The companion: the legitimate form `c.color(...)` inside the
    /// callback still works because the callback's param is its own
    /// fresh inner root.
    #[test]
    fn callback_param_root_works_independently_of_outer_roots() {
        let expr = parse_expr("cas().hover(c => c.color('red'))");
        assert!(walk_chain(&expr, &cas_roots()).is_some());
    }

    #[test]
    fn nested_modifier_scopes_walk_recursively() {
        let expr = parse_expr("cas().hover(c => c.darkMode(d => d.color('white')))");
        let ops = walk_chain(&expr, &cas_roots()).expect("recognised");
        let hover_scoped = match &ops[0] {
            Op::Scoped(s) => s,
            _ => panic!("expected outer scope"),
        };
        assert!(matches!(
            hover_scoped.scope,
            Scope::Pseudo { ref selector } if selector == ":hover"
        ));
        let inner_scoped = match &hover_scoped.ops[0] {
            Op::Scoped(s) => s,
            _ => panic!("expected inner scope"),
        };
        assert!(matches!(
            inner_scoped.scope,
            Scope::Media { ref query } if query == "(prefers-color-scheme: dark)"
        ));
    }
}
