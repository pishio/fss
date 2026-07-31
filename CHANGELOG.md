# Changelog

All notable changes to Cassida are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). All packages under the `@cassida/` scope are versioned in lockstep.

## [Unreleased]

### Added

- **`.cond()` now works inside same-file function composition** — the `withCard(cas())` mixin idiom previously bailed to the runtime fallback whenever a `.cond()` branch appeared anywhere in the mixin body or in the fed-in argument. Both positions are now lifted to the same build-time Cartesian expansion the JSX-spread path already used, so `const withState = (c) => c.cond(a, x => x.color('red'), y => y.color('blue'))` applied to `cas().padding(8)` compiles to one class per leaf plus the className ternary — no runtime `cas(` call. Branches nested inside a modifier scope in the mixin body (`c.hover(h => h.cond(...))`) are handled too, and the composed output is class-identical to the equivalent inline chain. This lands in the Babel/Vite transform (`@cassida/parser`); the SWC/Next.js path (`@cassida/swc-plugin`) still falls back to the runtime for `.cond()` and function composition, as it did before. Multi-param mixins, imperative control flow (`if`/loops), and cross-file imports still fall through to the runtime as before.
- **`@cassida/next-plugin` now fails fast under Turbopack** instead of silently shipping a page with no Cassida CSS. Cassida's CSS bridge is webpack-only, and Turbopack (the default bundler for `next dev` / `next build` since Next.js 15) never runs the webpack plugin that emits the styles. When Turbopack is detected at config time (via `process.env.TURBOPACK`), `withCassida` throws a clear error pointing at `next dev --webpack` / `next build --webpack`. The guard is best-effort and never false-positives on a real webpack run; set `CASSIDA_ALLOW_TURBOPACK=1` to bypass it. Full Turbopack support is tracked for a later release.

### Changed

- **`@cassida/swc-plugin`'s Next.js-targeted WASM (`.next.wasm`) now targets Next.js 16.2.x (swc_core 57.0.0)** instead of Next.js 15.x (swc_core 35.0.0). The SWC plugin ABI is version-bound to `@next/swc`'s embedded swc_core, and Next.js 16.2 (stable) moved it to 57.0.0; a plugin built against a different major fails to load with "failed to invoke plugin". **This drops Next.js 15 support from the next-targeted build** — Next.js 15 users must stay on the previous Cassida release until a dual-target build is restored. The modern `.wasm` (swc_core 66.x, for Rspack / `@swc/core` / `plugin-react-swc`) is unchanged. Both crates' transform sources are now byte-identical, since swc_core 57 already uses the `Wtf8Atom` string type and the swc_core-35-era adapter sites are gone. Cassida's CSS bridge remains webpack-only, so Next.js 16 requires `next build --webpack` (Turbopack is still unsupported). Next.js 16.3 preview moved swc_core to 69.0.0 — a further re-pin will be needed if/when that line stabilises.
- **`@cassida/next-plugin` now rejects the unimplemented `options.plugins.{conditional,print,globalCss}` flags at config time** instead of logging a warning and silently doing nothing. Each flag was recognised but inert in the Next.js path (the `conditional` parser-plugin API, `print` preflight wiring, and the `globalCss` webpack integration are still follow-ups), and a recognised-but-inert option is a semver trap. Enabling one now throws a clear, actionable error naming the reason. `options.plugins.hoverFix` is unaffected; if you set one of the three, remove it — it was not doing anything.

## [0.12.0] — 2026-06-15

### Added

- **`shared-by-declaration` CSS emit mode** — opt in with `{ css: { mode: 'shared-by-declaration' } }` (default stays `'rule-per-class'`). In this mode every class's **root-scope** declarations are folded into a `(property:value) → classes` inverted index and emitted as grouped selectors (`.a,.b,.c{color:red}`), cutting duplicate declarations when many elements share styling. Modifier scopes (`:hover`, `@media`, ...) stay per-class. The `className` hash is derived from the canonical bag and is identical in both modes, so the bijection contract is preserved — only the emitted CSS shape changes. Exposed on `CssEmitterOptions.mode` and wired through `@cassida/vite-plugin` and `@cassida/next-plugin` from the resolved `css.mode` config.
- **`defineMacro` API + three built-in macros** — new public exports from `@cassida/compiler`: the `defineMacro({ name, trigger, fills, skipIfAnyPresent?, skipIfTriggerValueIn? })` factory plus three macros (`zIndexMacro`, `transformMacro`, `positionStickyMacro`) and `defaultMacros` / `resolveMacros` helpers. A macro is a small `CassPlugin` that runs BEFORE user plugins in `compileOps`, fills property defaults the user did not write explicitly, and never overrides an explicit value. The optional `skipIfTriggerValueIn` field accepts a list of CSS-wide keyword values (`auto`, `none`, `unset`, `initial`, `inherit`, `revert`, `revert-layer`) that suppress the macro entirely — used by the built-ins to ignore `z-index: auto`, `transform: none`, and similar no-op writes. Macros only fire on the root scope; modifier-scope writes (`:hover`, `@media`, ...) are unchanged. New `CassConfig.macros.disable` field lets users disable specific built-in macros by name (`{ macros: { disable: ['zIndex'] } }`); typo'd names emit a warning on `process.stderr` (silence via `CASSIDA_QUIET_MACRO_TYPO_WARNING=1`). Register custom macros through the inline plugin option on `@cassida/vite-plugin` / `@cassida/next-plugin`. For convenience the package also exports `CssGlobalKeyword` (a union of the five CSS-wide keywords) and the matching frozen `CSS_GLOBAL_KEYWORDS` array, usable as a typed seed for `skipIfTriggerValueIn`.

### Changed

- **`.zIndex(n)` / `.transform(...)` / `.position('sticky')` now imply property defaults at build time** — when the user did not write an explicit `position` / `will-change` / first inset side (respectively), the compiler now fills in `position: relative` / `will-change: transform` / `top: 0` via the new built-in macros (above). Existing chains that wrote those properties explicitly are unchanged: explicit user values always win. The `positionSticky` macro respects logical-property writes too — `inset-block-start: 10px` suppresses the `top: 0` fill, same as the physical `top`. CSS-wide keywords (`auto` / `none` / `unset` / `initial` / `inherit` / `revert` / `revert-layer`) on the trigger property suppress the macro — `cas.zIndex('auto')` does not fill `position`, `cas.transform('none')` does not fill `will-change`. Disable the macros individually via `cassida.config.json` → `{ macros: { disable: ['zIndex', 'transform', 'positionSticky'] } }`. Consumers running CSS snapshot tests against v0.11.x output should expect additional declarations on `.zIndex` / `.transform` / `.position('sticky')` chains; either accept the snapshot update or disable the macros to preserve the v0.11 shape.

- **`lightningcss` post-processing is now enabled by default** (`defaultConfig.css.lightningcss.enabled = true`). Emitted CSS is run through `lightningcss` for vendor prefixing and (`minify: true` by default) minification. `@layer cas` and `@property` rules are preserved across the pass — lightningcss 1.28+ understands both natively. Disable with `{ css: { lightningcss: { enabled: false }}}` in `cassida.config.json` or inline plugin options. Behaviour change: production CSS bundles will now contain `-webkit-` / `-ms-` prefixes on properties whose browserslist target needs them. Adjust your snapshots accordingly.
- **`lightningcss` post-processing ported to `@cassida/next-plugin`** — the implementation was previously bound to `@cassida/vite-plugin`. It now lives in `@cassida/compiler/internal` (`postProcessLightningCss` / `resolveTargets`) and both plugins consume it from there. The Next.js webpack plugin (`CassidaWebpackPlugin`) runs the same pass on the emitted `virtual.css`, resolving browserslist targets from the webpack `compiler.context` (project root).
- **`@cassida/compiler` peer deps for `lightningcss` + `browserslist`** — both are declared as optional peer dependencies on `@cassida/compiler`. The vite-plugin / next-plugin pull them in as runtime deps; standalone CLI consumers can opt out (lightningcss is skipped when the peer is absent, the emitter still produces unprocessed CSS).

### Fixed

- **`@cassida/next-plugin` now honours `media.sort`** — the Next.js plugin was always emitting media queries mobile-first, ignoring a configured `{ media: { sort: 'desktop-first' } }` (it never forwarded `resolved.media.sort` to the emitter, unlike `@cassida/vite-plugin`). The setting now propagates correctly.

## [0.11.0] — 2026-06-11

### Added

- **Supported Next.js versions policy** — `@cassida/next-plugin`'s README now documents that only the current Next.js LTS release is supported; older majors aren't tested in CI and may break silently when `@next/swc` bumps its embedded `swc_core`.
- **swc_core drift monitoring** — a weekly GitHub Actions cron (`.github/workflows/swc-core-drift.yml`) compares `@cassida/swc-plugin-next`'s `swc_core` pin against the latest Next.js LTS release and auto-opens a tracking issue when they diverge. Enforces the Supported Next.js versions policy added in the previous changelog entry.
- **`@cassida/next-plugin` README guidance for monorepo + `output: 'standalone'`** — documents the `outputFileTracingRoot` footgun where setting it to the app directory silences Next.js 15's "multiple lockfiles" warning but silently drops `@cassida/*` from the standalone bundle's `node_modules` tree.
- **`PHILOSOPHY.md` at repo root + `/philosophy` page on the docs site** — bilingual (EN / 日本語) write-up of the five principles (Single Class · Zero-runtime · LIFO Collapse · Bijection · Shorthand Policy) and the *toge-ari toge-nashi toge-toge* → *Cassida* (Latin *cassis*, helmet) origin. README links out to it; the docs sidebar gets a Philosophy entry between Home and Install. The LIFO description is phrased as "last write wins, the earlier value is dropped by the compiler" (rather than the more abstract "collapse the IR"); bijection is spelled out as a three-way correspondence between chain shape, emitted CSS rule body, and class hash.
- **Docs site Frameworks section** — three new pages under `/:locale/frameworks/*`: Next.js setup (Quick Start: install + `withCassida()` + `import '@cassida/next-plugin/virtual.css'`, plus the App Router / Next.js 15 LTS / webpack-only constraints and the monorepo + `output: 'standalone'` footgun), `@cassida/swc-plugin` (Rust → WASM, dual-WASM with `swc_core` 66.x for Rspack / `@swc/core` and 35.0.0 for `@next/swc` 15.x, ABI version-bound, three install routes: Next.js transitive vs Rspack direct vs `@swc/core` direct), `@cassida/next-plugin` (`withCassida()` entry, `/virtual.css` placeholder + `writeModule` lifecycle, `CassidaWebpackPlugin` hooks — `thisCompilation` / `processAssets` at stage `PROCESS_ASSETS_STAGE_PRE_PROCESS` (-1000) / `beforeRun` / `watchRun` for HMR namespace clearing, cross-compiler bridge via `(compilerName, filename)` two-level keying, store API: `allRules` / `allRulesForCompiler` / `subscribe`). Sidebar picks up a Frameworks section between API and Plugins. README gains a "Next.js Quick Start" section directly under the Philosophy link with the three minimum-viable code blocks.

### Changed

- **`tsconfig.typecheck.json` adoption standardised across all public packages** — five plugins / `@cassida/recommended` previously used `tsconfig.test.json` (or no typecheck config at all). They now use `tsconfig.typecheck.json` consistently with the other packages, and a CI guard fails the build if a public `@cassida/*` package is missing the file or its `scripts.typecheck` doesn't route through it.
- **Docs site Japanese prose rewritten for plain-language readability** — Landing, `cas() chain`, and Property registry pages get full rewrites; Modifiers, Configuration, Unsafe surface, `@cassida/recommended`, and `@cassida/plugin-conditional` get targeted edits. Replaces translation-shaped sentences, literary verbs ("畳み込む" outside the now-locked LIFO term, "刈り込む", "昇華"), abstract math vocabulary ("正準なバッグ", "全単射 (bijection) が保証される"), and the internal-IR name ("ScopedOp") that had leaked into the public docs surface. A second pass also rewrote Install / `@cassida/plugin-hover-fix` / `@cassida/plugin-global-css` / `@cassida/plugin-print` for the same plain-language pattern. The Landing English copy is updated to match the Philosophy page voice (no more "BEM in three hyphens" / "garlands" / "fight is over by the time the page paints"); the rest of the English copy is unchanged.
- **Docs site engineer-scaffolding additions** — Install page leads with a five-minute Quick Start (complete `package.json` / `vite.config.ts` / `App.tsx` / `main.tsx` scaffold + run command). New `/:locale/glossary` page collects every recurring term (Single Class Principle, Zero-runtime, LIFO Collapse, Bijection, canonical, longhand, shorthand-policy, modifier scope, `@layer cas`, `.props` terminator, cross-compiler bridge) under stable anchors so other pages can link directly to a definition. Modifiers / `@cassida/recommended` / `@cassida/plugin-hover-fix` / `@cassida/plugin-global-css` / `@cassida/plugin-print` / Unsafe / `@cassida/plugin-conditional` pages now lead with a runnable code block before any prose — the call-site-first reading order favoured by the engineer-as-reader audit.

### Fixed

- **`.border(number)` / `.outline(number)` typecheck-runtime mismatch fixed** — the csstype `Border<TLength>` / `Outline<TLength>` generics widened the input union to include `number`, but the runtime passthrough emitted invalid CSS (`border: 1`). Dropped the generic so only strings (`'1px solid red'`) typecheck; numeric width goes through `.borderWidth(1)` / `.outlineWidth(1)`.

## [0.10.0] — 2026-06-09

### Added

- **`@cassida/swc-plugin` and `@cassida/next-plugin` published to npm** — the two scaffolds introduced in v0.8.0 and hardened through v0.8.x / v0.9.x are now public packages. `npm install @cassida/next-plugin` from a Next.js 15 App Router project + `withCassida(nextConfig)` in `next.config.mjs` is the supported entry. `@cassida/swc-plugin` ships the dual-WASM build (modern `swc_core` 66.x for Rspack / @swc/core, next-targeted `swc_core` 35.0.0 for `@next/swc` 15.x) and is published primarily as a transitive dependency of `@cassida/next-plugin`. Phase 1 closing — multi-compiler bridge resolution (v0.9.0), `<link rel="stylesheet">` injection assert, and the e2e `consumer · next 15` matrix leg — all give us the observation window the v0.8.0 release notes promised.
- **Hand-curated CSS shorthands `border`, `font`, `flex`, `grid`, `outline`** — five chain methods added to `packages/compiler/src/property-spec.ts`, restoring the documented "the entire surface of CSS" claim. Each is a passthrough-string formatter typed via csstype (`CSS.Property.Border<LenArg>`, `CSS.Property.Font`, ...), so call-sites get IDE autocomplete for canonical CSS values. The shorthand-policy guard is wired across each family: `border ↔ borderWidth / borderStyle / borderColor`, `font ↔ fontFamily / fontSize / fontWeight / lineHeight / fontStyle / fontVariant / fontStretch`, `flex ↔ flexGrow / flexShrink / flexBasis`, `outline ↔ outlineWidth / outlineStyle / outlineColor`, all erroring under `'strict'` policy when mixed with a longhand in the same scope. `.flex(1)` passes through to the browser's well-defined `1 1 0%` resolution rather than pre-expanding, preserving devtools readability of the source value. `flexGrow` / `flexShrink` / `flexBasis`, `fontStyle` / `fontVariant` / `fontStretch`, and `outlineWidth` / `outlineStyle` / `outlineColor` are promoted from the generated set into the hand-curated spec so their `longhandFamily` registration takes effect. `border` and `outline` shorthands are declared `animatable: false` — the alternation grammar `<line-width> || <line-style> || <color>` is not a valid `@property` syntax descriptor, so emitting an `@property` rule for the shorthand would be silently dropped by the CSS parser and the dynamic-value interpolation path would break; the longhands keep their per-property `animatable: true`. `grid` stays `animatable: false` and its `shorthandFamily` declaration is forward-looking — the guard activates once `gridTemplateColumns` / `gridTemplateRows` / etc. are promoted into the hand-curated set with `longhandFamily: 'grid'`. `e2e/next-app/app/server-only.tsx` is reverted to the `.border('1px solid #ddd')` form the API now supports.
- **Type-level test baseline lifted across four packages** — new `.test-d.ts` files in `@cassida/compiler`, `@cassida/parser`, `@cassida/next-plugin`, and `@cassida/vite-plugin` lock in consumer-facing type contracts (`Op` / `Scope` discriminated unions, `CassConfig` strictness, `TransformOptions` / `PathAliases`, `NextCassidaOptions['plugins']` per-key shapes, `withCassida` overload narrowing, the Vite plugin factory's `CassPluginOptions`). Each file pairs `@ts-expect-error` negative cases with positive narrowing assertions. The previous coverage was a single file in `@cassida/core`.
- **Standardised `tsconfig.typecheck.json` across the typecheck path** — `@cassida/compiler`, `@cassida/parser`, `@cassida/next-plugin`, and `@cassida/vite-plugin` now ship a `tsconfig.typecheck.json` (mirroring the shape `@cassida/core` already used) and their `pnpm typecheck` scripts route through it. `tsc -p tsconfig.json --noEmit` skipped `test/**/*` and missed regressions; the typecheck config includes both `src/` and `test/`, so `.test-d.ts` files participate in CI's `pnpm -r typecheck` gate.

### Changed

- **`e2e` CI workflow hardened** — every job carries an explicit `timeout-minutes` (pack: 20, consumer-next: 15, consumer-vite / consumer-bun: 10) so a wedged Cargo or Next build no longer holds a runner for the GitHub Actions 6-hour default. The `pack` job now layers `Swatinem/rust-cache@v2` keyed by Cargo lock + toolchain, dropping wasm32-wasip1 builds from ~3-4 min cold to seconds when warm. `concurrency.cancel-in-progress` is now gated on `github.event_name == 'pull_request'`, so consecutive commits on `main` (including release commits) no longer kill each other's e2e mid-pack. `e2e/next-app/next.config.mjs` sets `outputFileTracingRoot` to silence Next.js 15's "multiple lockfiles" warning when the consumer dir's `package-lock.json` coexists with the repo root's `pnpm-lock.yaml`.

## [0.9.0] — 2026-06-01

### Changed

- **`@cassida/next-plugin` store rebuilt as a cross-compiler bridge** — the module-singleton rule store in `packages/next-plugin/src/store.ts` is now keyed first by webpack compiler namespace (Next.js sets `'client'` / `'server'` / `'edge'` / `'middleware'`), then by file. `allRules()` still merges every namespace on the read path — that merge is the architectural mechanism by which a `cas()` chain inside a Server Component (Server compiler only) flows into the Client compiler's `virtual.css`, which the RSC-serialised className in the browser then resolves against. The previous "multi-compiler race" framing in the v0.8.0 inline docs was inverted: the cross-compiler share is the feature, not the bug. New `allRulesForCompiler()` exposes per-namespace reads for tooling.
- **`@cassida/next-plugin` lifecycle hooks for HMR hygiene** — `CassidaWebpackPlugin` now taps `compiler.hooks.beforeRun` (production) and `compiler.hooks.watchRun` (dev), clearing only THAT compiler's namespace before its loaders re-run. Stale rules from since-deleted source files no longer accumulate across `next dev` HMR passes, and the OTHER compiler's namespace is preserved across each clear so the cross-compiler bridge survives every compiler's independent lifecycle.
- **`@cassida/next-plugin` race detection replaces the v0.8.0 heuristic** — the empty-store stderr heads-up gated on `seen.length === 0` produced false positives on legitimately empty fixtures (test apps, pages with no `cas()` chains). The probe now requires a peer compiler to have actually written into the store before — "peer wrote at T, but my read sees zero from their namespace" is the real race signature. Suppression knob (`CASSIDA_QUIET_RACE_WARNING=1`) and production gating (`NODE_ENV=production`) are unchanged.
- **`e2e/next-app` asserts the `<link rel="stylesheet">` injection contract** — the post-build assert now verifies that the rendered HTML / RSC payload actually carries a `<link rel="stylesheet" href="/_next/static/css/...">` referencing an on-disk CSS asset. The previous six-check pass could miss a regression where `virtual.css` is emitted to disk but Next.js drops the stylesheet link injection (exports-map drift, symlink mismatch, virtual-module path collision) — the browser would then paint unstyled markup despite a "successful" build. Adapted from the Browser API persona's parallel multi-compiler-architecture pass.

## [0.8.0] — 2026-05-29

### Added

- **`@cassida/swc-plugin` and `@cassida/next-plugin` scaffolds** — two new workspace packages laying the groundwork for Next.js (App Router included) integration via a Rust-authored SWC plugin. Phase 1 ships the no-op plugin transform and the `withCassida()` config wrapper API surface; subsequent commits fill in the chain walker, IR-comment loader, and CSS pipeline. Architecturally the plugin emits the existing `Op[]` IR as a comment annotation that the Node-side loader feeds to `@cassida/compiler`, so Babel-parsed and SWC-parsed files hash to identical class names. Both packages are marked private until Phase 1 is feature-complete.
- **`@cassida/swc-plugin` chain walker + JSX rewrite** — the Rust SWC plugin now lifts `{...cas().X().props}` JSX spreads. The walker handles literal-arg method ops, canonical modifier scopes (`.hover`, `.before`, etc.), arg modifiers (`.media`, `.on`), `.set(prop, value)`, the trailing `.props` terminator, default / named / aliased imports of `@cassida/core`, and a broad parity surface with the Babel parser (`UnaryOp::Minus`/`Plus`/`Bang` on literal operands, parens at every step, `tpl` cooked strings, JS safe-integer range for numeric coercion, kebab-case `camelToKebab` matching Babel exactly). 48 unit tests cover the IR JSON shape, modifier table, walker, and visitor.
- **`@cassida/next-plugin` IR-comment loader + virtual CSS** — first end-to-end wiring of the Next.js path. `withCassida(nextConfig, options)` registers `@cassida/swc-plugin`'s WASM in `experimental.swcPlugins`, injects a webpack loader that scans `/* @cassida-ir:JSON */ "__CAS_PLACEHOLDER_N__"` pairs in transformed sources, calls `compileOps` to mint real class names, accumulates the rules in a module-singleton store, and exposes the aggregated `@layer cas` CSS via `buildVirtualCss()`. Function-form `next.config.js` is supported by wrapping the user function so options apply to its resolved config.
- **`CassidaWebpackPlugin` virtual CSS module** — consumers can now `import '@cassida/next-plugin/virtual.css'` from `app/layout.tsx`. The plugin registers the path via `webpack-virtual-modules` and rewrites its content twice per compilation: a placeholder seed at `compiler.hooks.thisCompilation`, then the real `buildVirtualCss()` output at `compilation.hooks.processAssets` (stage `PRE_PROCESS`, after the IR loader has populated the store and before CSS minifiers run). Dev / HMR re-writes via a `store.subscribe()` listener attached on `watchRun`. Next.js's CSS pipeline handles chunking, minification, and Server / Client Component delivery.
- **`e2e/next-app/` fixture + `consumer-next` CI job** — Phase 1 closing signal. Mirrors the existing Vite consumer: a minimal Next.js 15 App Router app with `withCassida` configured, a Server Component (`server-only.tsx`) and a `'use client'` page exercising static chains, modifier scopes, and dynamic values. The post-build `assert.mjs` verifies six contracts on the `.next/` output (CSS `@layer cas`, rendered `class="cas-XXXXXXXX"` attributes, no `cas(` runtime calls in client chunks, no placeholder leakage, no compiler runtime leak). The CI workflow installs the cassida tarballs (including the WASM-bundled `@cassida/swc-plugin`) and runs `next build` + assertions in matrix leg `next: 15`. Phase 1.5 will add Turbopack and the previous Next.js 14 leg back.
- **`@cassida/swc-plugin` dual-WASM build** — ships two ABI-pinned WASM artefacts side by side. `wasmPath` / `wasmPathModern` (`dist/cassida_swc_plugin.wasm`) is built against swc_core 66.x for Rspack, @swc/core mainline, swc-loader, swc-node, and @vitejs/plugin-react-swc. `wasmPathNext` (`dist/cassida_swc_plugin.next.wasm`, exported at the `./next` subpath) is built against swc_core 35.0.0 for the `@next/swc` binary shipped in Next.js 15.x. `@cassida/next-plugin` consumes the Next-targeted build. The split exists because the SWC plugin ABI is version-bound to the host's swc_core, and shipping only the modern build manifested as `failed to invoke plugin` on every file inside Next.js 15. A new `cassida_swc_plugin_next` Rust crate mirrors the source of `cassida_swc_plugin`; six `Atom::as_str()` call sites are adapted because the return type widened from `&str` to `Option<&str>` between swc_core 35 and 66.
- **`CassidaWebpackPlugin` empty-store heads-up** — in production builds the plugin now writes a stderr line when `processAssets` fires with no rules registered. This is the user-visible signal that the documented Phase 1.x multi-compiler race (Server compiler populates the store after the Client compiler's `processAssets`) actually fired in a real build — without it Server-only styles silently fall out of the Client bundle. The check is gated on `NODE_ENV === 'production'` so dev / test runs stay quiet; consumers with legitimately empty fixtures can mute via `CASSIDA_QUIET_RACE_WARNING=1`.

## [0.7.0] — 2026-05-21

### Added

- **TypeScript path-alias resolution in cross-file eval** — `import { theme } from '@/tokens'` now folds at build time when the project declares `compilerOptions.paths` in `tsconfig.json`. The Vite plugin auto-discovers paths via the new `loadTsconfigPaths(projectRoot)` helper; `extends` chains are followed the whole way up (cycles guarded), with each config's paths anchored against its own `baseUrl`. Override or disable through the plugin's new `pathAliases` option (`PathAliases | false`). Standalone consumers of `@cassida/parser` can pass `pathAliases` directly to `transform()` or call `loadTsconfigPaths` themselves.

## [0.6.0] — 2026-05-20

### Changed

- **`.cond()` lifted inside modifier scopes** — chains like `cas().hover(c => c.cond(active, t => t.bg('red'), f => f.bg('blue')))` now expand to two build-time classes (one for each branch) instead of bailing to the runtime. The Cartesian expansion descends into scoped ops, and `buildBranchedExpr` handles leaves with variable conditions length when a `.cond()` lives inside only one branch of an outer `.cond()`. The `[Limitations]` entry from `0.4.0` is resolved; `.cond()` inside function composition (`withCard(cas())`) still bails.
- **Dynamic args on multi-property utilities** — `cas().px(theme.spacing)` (and `.py` / `.mx` / `.my`) now compile at build time. The canonicalizer seeds every longhand the entry expands to with the same source id, so the parser emits one CSS variable per longhand bound to the same source expression — `style={{ '--cas-X-padding-inline-start': theme.spacing, '--cas-X-padding-inline-end': theme.spacing }}`. `defaultPropertyMeta` now also seeds the longhands of every multi-property canonical entry so `@property` descriptors emit for the CSS variables on both halves. The `[Limitations]` entry from `0.4.0` is resolved.

## [0.5.0] — 2026-05-14

### Added

- **`@cassida/plugin-print`** — `printPreflight()` returns a CSS string of conservative `@media print` defaults. Ink-saving black-on-white, external link URL expansion with `overflow-wrap: break-word`, abbreviation expansion via `<abbr title>`, media width clamp covering `img` / `svg` / `video` / `canvas` with `height: auto`, page-break hygiene for `pre` / `blockquote` / `tr` / `h1`–`h6`, `p` / `li` orphans + widows: 2, and `thead` / `tfoot` repetition across page boundaries. Adapted from the HTML5 Boilerplate print subset (MIT). Cascade-layer-aware (no `!important`), so Cassida classes in `@layer cas` override the defaults cleanly.

## [0.4.0] — 2026-05-14

### Added

- **`@cassida/plugin-global-css`** — first-party Vite plugin that serves arbitrary global CSS (preflight, resets, body / tag-selector rules) through a virtual module, wrapped in a configurable cascade `@layer`. Pairs with the `@layer base, cas;` declaration so Cassida classes win without specificity tricks.
- **Multi-property utilities** — hand-crafted `px`, `py`, `mx`, `my` chain methods write two logical longhands per call (`padding-inline-start/end`, `padding-block-start/end`, `margin-inline-start/end`, `margin-block-start/end`). Per-longhand LIFO collapse and full participation in the `shorthand-policy` guard as longhands of the parent `padding` / `margin` family.
- **`.cond(test, truthy, falsy?)` chain method** — chain-internal branching. At build time the Cartesian product of branches materializes into N classes (cap 32); the JSX `className` becomes a balanced nested ternary. Runtime evaluates `test` eagerly and inlines the picked branch's ops so its hash matches the corresponding build-time leaf byte-for-byte.

### Changed

- **`@cassida/plugin-conditional` v2: dynamic-slot branches** — earlier versions bailed when either branch carried a dynamic slot. v0.4 keeps the build-time path active: dynamic branches emit a parallel branch-conditional `style={...}` ternary that mirrors the className shape. Branches without dynamics contribute `void 0`.
- **Registry contract widened** — `Formatter` now returns `string | StyleBag`; `RegistryEntry` gains optional `properties: readonly string[]` for multi-property entries. Existing single-property entries are byte-identical.
- **Parser-plugin helpers expanded** — `getDynamicSource(sourceId)` exposes the source AST behind a compiled `DynamicSlot`; `mergeStyleExpression(existing, pluginExpr, casWins)` is a generalized counterpart to `makeStyleAttr` that accepts any expression. `ExistingHostAttrs` gains `casWins: boolean` so plugin-emitted style merges honor JSX source-order precedence.

### Limitations

- `.cond()` inside modifier scopes (`.hover(c => c.cond(...))`) currently bails to runtime — Phase 2 will lift this.
- Dynamic args on multi-property entries (`cas().px(theme.spacing)`) bail at build time; pass a literal or use the underlying `.paddingInline(...)` / `.paddingBlock(...)` for dynamic values.

## [0.3.0] — 2026-05-12

### Added

- **`.props` JSX terminator** — every chain ends with `.props` and yields `{ className: string; style: Readonly<CSS.Properties> }`. Strips the chain's ~460 method handles from the spread shape, so React's strict JSX typings (`translate`, `disabled`, `color`, `width`, `height`, ...) accept the spread under `tsc --noEmit`. End-to-end CI now runs `tsc --noEmit` against the consumer fixture on every Vite leg.

### Changed

- `CassChainProps` is the new exported type for the spread shape.
- `CassChainTerminus` surfaces `.props` instead of bare `style` / `className`.
- `.props` is defined on every chain (root and inner) and memoized by `ops.length`, so repeated reads on the same finalized chain skip the canonicalize pass and preserve identity for `React.memo` comparisons.
- Parser peels a trailing `.props` member access via a `peelPropsAccess` helper; output is byte-identical to the bare-chain form, so v0.2 codebases continue to compile while they migrate.

## [0.2.0] — 2026-05-11

### Added

- **Cross-file static evaluation** — design tokens defined in a separate module or `.json` file fold into static class hashes at build time. The parser walks `import` declarations from the file at `filename`, evaluates the import chain (supports JSON, destructured exports, re-export chains), and inlines the resolved value into the chain's canonical bag.
- **`createModuleCache()`** for amortizing parse work across many files during a Vite build. The vite-plugin wires this in automatically.

## [0.1.1] — 2026-05-06

### Fixed

- **Universal resolver hardening** — `sideEffects: false` on every published package so bundlers can drop unused entries. Pure-JS MurmurHash3 implementation (no `node:crypto` dependency) for browser / Bun / Edge runtime compatibility. `exports` map cleanup so consumers can import without resolution surprises.
- e2e CI builds a real consumer against the tarballs on Vite 5 / 6 / 7 and Bun.

## [0.1.0] — 2026-04-30

### Added

- Initial public release of `@cassida/core`, `@cassida/compiler`, `@cassida/parser`, `@cassida/vite-plugin`, `@cassida/recommended`, `@cassida/plugin-hover-fix`, `@cassida/plugin-conditional`. Single Class Principle, LIFO collapse, build-time class table, deterministic hashing, mobile-first media sort, `@layer cas` cascade-layer wrapping, csstype-typed canonical chain methods + mdn-data-derived generated chain methods, `cas.unsafe` / `.set` escape paths, `shorthand-policy` guard.

[Unreleased]: https://github.com/pishio/cassida/compare/v0.12.0...HEAD
[0.12.0]: https://github.com/pishio/cassida/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/pishio/cassida/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/pishio/cassida/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/pishio/cassida/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/pishio/cassida/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/pishio/cassida/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/pishio/cassida/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/pishio/cassida/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/pishio/cassida/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/pishio/cassida/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/pishio/cassida/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/pishio/cassida/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/pishio/cassida/releases/tag/v0.1.0
