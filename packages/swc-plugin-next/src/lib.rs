//! Cassida SWC plugin — Next.js-targeted build (swc_core 57.0.0).
//!
//! Identical transform to the sibling `cassida_swc_plugin` crate
//! (which sits on swc_core 66.x for Rspack / @swc/core /
//! plugin-react-swc consumers). The split exists because the SWC
//! plugin ABI is version-bound: Next.js 16.2.x ships `@next/swc`
//! linked against swc_core 57.0.0, so a plugin compiled against a
//! different major panics with "failed to invoke plugin" on every
//! file. We rebuild the same source against 57.0.0 here.
//!
//! As of the Next.js 16 bump the two crates' transform sources
//! (`ir.rs`, `modifiers.rs`, `visitor.rs`, `walker.rs`) are
//! byte-identical: swc_core 57 already uses the `Wtf8Atom` string type
//! (`Atom::as_str()` returns `Option<&str>`), so the swc_core-35-era
//! adapter sites are gone and both crates share one form. Only this
//! module's doc comment differs from the sibling `lib.rs`.
//!
//! Next.js 15.x (swc_core 35.0.0) is no longer targeted by this crate.
//! Next.js 16.3 preview moved to swc_core 69.0.0 — re-pin the
//! `Cargo.toml` dependency and rebuild if/when that line stabilises.

use swc_core::{
    ecma::ast::Program,
    ecma::visit::VisitMutWith,
    plugin::{plugin_transform, proxies::TransformPluginProgramMetadata},
};

mod ir;
mod modifiers;
mod visitor;
mod walker;

use visitor::CassidaVisitor;

#[plugin_transform]
pub fn process_transform(
    mut program: Program,
    metadata: TransformPluginProgramMetadata,
) -> Program {
    // `metadata.comments` is `Option<PluginCommentsProxy>`. Next.js's
    // production SWC pipeline strips comments from internal files
    // (`node_modules/next/dist/**/*.js`), and the plugin runs on
    // every file in the graph — including those. Passing `None` is
    // legitimate in that path; we simply skip the transform for
    // those files (there are no `cas()` chains inside Next.js's
    // internal sources anyway, so there's nothing to lift).
    //
    // The earlier behaviour (panicking via `.expect()`) was meant
    // to surface "consumer disabled comments globally" — but it
    // can't distinguish that footgun from Next.js's normal
    // file-by-file stripping. The right place for the footgun
    // warning is `@cassida/next-plugin`'s `withCassida` wrapper,
    // which controls the SWC config at the consumer level.
    let Some(comments) = metadata.comments else {
        return program;
    };
    let mut visitor = CassidaVisitor::new(comments);
    program.visit_mut_with(&mut visitor);
    program
}
