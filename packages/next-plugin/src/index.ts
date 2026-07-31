/**
 * `@cassida/next-plugin` — Next.js integration for Cassida.
 *
 * `withCassida(nextConfig, options)` wraps a Next.js config to:
 *   1. Register `@cassida/swc-plugin` in `experimental.swcPlugins`
 *      so `cas()` chains in `.tsx` files lift to `Op[]` IR comments.
 *   2. Inject a webpack loader that scans those IR comments,
 *      compiles them via `@cassida/compiler`, substitutes the
 *      placeholder class names, and accumulates the rules.
 *   3. Expose the aggregated `@layer cas` CSS bundle as a virtual
 *      module the consumer's app entry can import.
 *
 * Design reference: `.claude/plans/swc-port-phase-1.md`.
 */

import { createRequire } from 'node:module';

import type { NextConfig } from 'next';
import { mergeConfig, resolveMacros, type CassConfig, type CassPlugin, type Registry } from '@cassida/compiler';
import type { CassParserPlugin, PathAliases } from '@cassida/parser';

import type { IrLoaderOptions } from './ir-loader.js';
import { CassidaWebpackPlugin } from './webpack-plugin.js';
export { rewriteIrComments, default as cassidaIrLoader } from './ir-loader.js';
export type { IrLoaderOptions } from './ir-loader.js';
export { buildVirtualCss } from './virtual-css.js';
import type { VirtualCssOptions } from './virtual-css.js';
import { resolveWebpackPluginOptions } from './webpack-options.js';
export type { VirtualCssOptions };
export { CassidaWebpackPlugin } from './webpack-plugin.js';
export {
  setRulesForFile,
  deleteRulesForFile,
  trackedFiles,
  trackedFilesForCompiler,
  allRulesForCompiler,
  clearCompilerNamespace,
  knownCompilerNames,
  lastWrittenAtForCompiler,
  subscribe as subscribeToRules,
} from './store.js';

/**
 * Functional form of `next.config.{js,mjs,ts}`: receives the build
 * phase + a `defaultConfig` and returns the object (sync or async).
 */
export type NextConfigFn = (
  phase: string,
  ctx: { defaultConfig: NextConfig },
) => NextConfig | Promise<NextConfig>;

/**
 * Either form Next.js permits for a config file. `withCassida` accepts
 * both shapes via overloads so the return type stays narrowed to the
 * shape that was passed in — callers don't need a type guard on the
 * returned value.
 */
export type NextConfigInput = NextConfig | NextConfigFn;

/**
 * Options accepted by `withCassida()`. A superset of the `CassConfig`
 * shape that `cassida.config.json` accepts, plus Next.js-specific knobs
 * for plugin selection, registry override, and the path-alias auto-discovery
 * toggle.
 */
export interface NextCassidaOptions extends CassConfig {
  /**
   * Registry override. Defaults to `@cassida/compiler`'s `defaultRegistry`.
   * Custom registries are rarely needed; the typical override is to extend
   * the property table for proprietary CSS properties.
   */
  readonly registry?: Registry;

  /**
   * Which optional plugins to load. Each key matches an `@cassida/plugin-*`
   * package; `true` enables with defaults, `false` disables, an object
   * passes per-plugin options. Unknown keys are ignored with a warning.
   *
   * Plugins are lazy-imported, so disabling one means the package isn't
   * loaded at all — useful for keeping the build cold-start cheap.
   */
  readonly plugins?: {
    readonly hoverFix?: boolean;
    readonly conditional?: boolean | { readonly shortCircuit?: boolean };
    readonly print?: boolean;
    readonly globalCss?: boolean | { readonly preflight?: string };
  };

  /**
   * AST-level parser plugins. Same shape as `@cassida/parser`'s
   * `CassParserPlugin`. Inline-only — these are functions, not
   * serialisable. Phase 1 ships the loader path; the SWC plugin Phase 4
   * will route unclaimed spreads to JS plugins via the same interface.
   */
  readonly parserPlugins?: readonly CassParserPlugin[];

  /**
   * CSS plugins forwarded to `compileOps`. Like `parserPlugins`, inline-only.
   */
  readonly cssPlugins?: readonly CassPlugin[];

  /**
   * TypeScript-style path aliases for cross-file evaluation. When omitted
   * (default), the loader auto-discovers `compilerOptions.paths` from
   * `tsconfig.json` via `loadTsconfigPaths(projectRoot)`. Pass `false` to
   * disable auto-discovery, or an explicit map to override.
   */
  readonly pathAliases?: PathAliases | false;

  /**
   * Files / paths the IR-comment loader should skip. Defaults to
   * `/node_modules/` — third-party packages don't run through the
   * Cassida SWC plugin and therefore can't contain IR comments. In
   * a pnpm / yarn workspace where local packages are symlinked into
   * `node_modules`, override this (or pass `null` for no exclusion)
   * so chains inside those workspace packages still get compiled.
   *
   * Mirrors webpack's `Rule.exclude` shape: a regex, a string path,
   * a predicate, or `null` to disable.
   */
  readonly loaderExclude?: RegExp | string | ((path: string) => boolean) | null;

  /**
   * Experimental knobs that are not yet stable. Reserved API surface
   * — the keys here may change shape without a semver bump until they
   * graduate.
   */
  readonly experimental?: {
    /**
     * Sidecar process for harvesting compiled rules out-of-band
     * from the webpack module graph. Reserved scaffolding for
     * Phase 2: when Turbopack lands as a sibling backend, the
     * cross-compiler bridge can't ride on a Node singleton because
     * Turbopack and webpack run in separate processes. The sidecar
     * would proxy writes via IPC or a shared file. Currently a
     * no-op — set to `true` to opt in once the implementation lands.
     *
     * TODO Phase 2 — wire to the actual sidecar transport.
     */
    readonly sidecar?: boolean;
  };
}

/**
 * Wrap a Next.js config with Cassida integration. Registers the SWC
 * plugin, wires up the CSS pipeline, and configures path-alias resolution.
 *
 * ```js
 * // next.config.js
 * import { withCassida } from '@cassida/next-plugin';
 *
 * export default withCassida(
 *   { reactStrictMode: true },
 *   { plugins: { hoverFix: true, conditional: true } },
 * );
 * ```
 *
 * Phase 1 returns the next config unchanged. The full integration lands
 * in subsequent commits.
 */
// Function-form overload is declared first: NextConfig has an open
// index signature, so a `(phase, ctx) => ...` arrow is also assignable
// to NextConfig itself. TypeScript picks the first matching overload,
// so flipping the order would silently route function-form callers
// into the object-form return and break narrowing at call sites.
export function withCassida(
  nextConfig: NextConfigFn,
  cassidaOptions?: NextCassidaOptions,
): NextConfigFn;
export function withCassida(
  nextConfig?: NextConfig,
  cassidaOptions?: NextCassidaOptions,
): NextConfig;
export function withCassida(
  nextConfig: NextConfigInput = {},
  cassidaOptions: NextCassidaOptions = {},
): NextConfigInput {
  // Function-form `next.config.js` returns the wrapped function so
  // the integration applies to whatever NextConfig it eventually
  // resolves to. Preserve the user function's sync / async shape:
  // forcing an async wrapper around a sync user function would
  // surprise third-party tools that introspect the return type.
  if (typeof nextConfig === 'function') {
    const userFn = nextConfig;
    return ((phase, ctx) => {
      const resolved = userFn(phase, ctx);
      if (resolved instanceof Promise) {
        return resolved.then((cfg) => applyCassida(cfg, cassidaOptions));
      }
      return applyCassida(resolved, cassidaOptions);
    }) as NextConfigFn;
  }
  return applyCassida(nextConfig, cassidaOptions);
}

// Lazy memoisation — defer the wasm lookup until `withCassida` is
// actually invoked. Importing the package shouldn't fail when the
// wasm artefact is missing (e.g. during monorepo bootstrap, static
// analysis, or unit tests that don't exercise the integration).
let cachedWasmPath: string | undefined;
function getWasmPath(): string {
  return (cachedWasmPath ??= resolveWasmPath());
}

/**
 * Cassida's CSS bridge is webpack-only: it emits `virtual.css` through a
 * `CassidaWebpackPlugin` tap. Under Turbopack (the default bundler for
 * `next dev` / `next build` since Next.js 16) the webpack config is never
 * consulted, so the bridge never runs and the page ships with no Cassida
 * CSS. Detect that at config time and fail loud with the remedy, rather
 * than letting the styles silently vanish.
 *
 * `process.env.TURBOPACK === '1'` is the signal Next sets when running
 * Turbopack. It is a Next internal, so this guard is best-effort: it
 * never false-positives on a real webpack run (the var is only set under
 * Turbopack), and if Next ever stops setting it the guard simply goes
 * quiet and we fall back to today's behaviour. Set
 * `CASSIDA_ALLOW_TURBOPACK=1` to bypass (e.g. you supply the CSS another
 * way). Full Turbopack support is tracked for a later release.
 */
function assertWebpackBundler(): void {
  if (process.env.CASSIDA_ALLOW_TURBOPACK === '1') return;
  if (process.env.TURBOPACK === '1') {
    throw new Error(
      '[cassida/next-plugin] Cassida needs the webpack bundler, but Turbopack is active. ' +
        'Turbopack does not run the webpack plugin that emits Cassida CSS, so styling would silently be missing. ' +
        'Run `next dev --webpack` / `next build --webpack` (full Turbopack support is planned for a later release), ' +
        'or set CASSIDA_ALLOW_TURBOPACK=1 to bypass this check.',
    );
  }
}

function applyCassida(
  cfg: NextConfig,
  options: NextCassidaOptions,
): NextConfig {
  assertWebpackBundler();
  // `options.plugins` is the declarative form (`{ hoverFix: true, ... }`);
  // resolve each enabled flag into the actual `CassPlugin` instance and
  // merge with the inline `options.cssPlugins` list. Without this the
  // declarative form is silently inert.
  const resolvedPlugins = resolveDeclarativeCssPlugins(options.plugins);
  const mergedPlugins: CassPlugin[] = [
    ...resolvedPlugins,
    ...(options.cssPlugins ?? []),
  ];
  // Resolve the full config once so the loader and the webpack plugin
  // both see the same macros.disable list. `mergeConfig(options)` is
  // re-invoked below for the webpack plugin (the wrapping cost is
  // negligible; the resolved value is structurally equal).
  const resolved = mergeConfig(options);
  const macros = resolveMacros(resolved.macros.disable);

  const loaderOptions: IrLoaderOptions = {
    ...(options.registry !== undefined ? { registry: options.registry } : {}),
    ...(options.shorthand?.policy !== undefined
      ? { shorthandPolicy: options.shorthand.policy }
      : {}),
    ...(mergedPlugins.length > 0 ? { plugins: mergedPlugins } : {}),
    ...(macros.length > 0 ? { macros } : {}),
  };

  // 1. Register the SWC plugin (the chain → IR-comment transform).
  // Next.js's `experimental.swcPlugins` is an array of
  // `[pathOrPackage, jsonOptions]` tuples. The wasm artefact ships
  // alongside this package via the `./loader` subpath export.
  // Skip the append if the same wasm path is already registered
  // (e.g. the user wrapped twice, or another tool added it) so the
  // plugin doesn't run on every file twice.
  const wasm = getWasmPath();
  const existingSwcPlugins = readSwcPlugins(cfg);
  const alreadyRegistered = existingSwcPlugins.some(
    ([path]) => path === wasm,
  );
  const swcPlugins: SwcPluginEntry[] = alreadyRegistered
    ? [...existingSwcPlugins]
    : [...existingSwcPlugins, [wasm, {}]];

  // `CssEmitter`'s `layer` field defaults to `'fss'` (a vestige of
  // the pre-rename project name); for `withCassida` callers we want
  // `'cas'` — matching `defaultConfig.layer`, the README, and the
  // documented `@layer cas { ... }` contract every consumer asserts
  // against. Honour `options.layer` if the user explicitly set one
  // (string for custom name, `null` for "no @layer wrap").
  const layer: string | null = options.layer ?? 'cas';

  // 2. Wrap user's `webpack` hook to inject the IR-comment loader.
  const userWebpack = cfg.webpack;
  const loaderExclude =
    options.loaderExclude === undefined ? /node_modules/ : options.loaderExclude;
  const wrappedWebpack: NextWebpackHook = (config, ctx) => {
    const base =
      typeof userWebpack === 'function' ? userWebpack(config, ctx) : config;
    return injectIrLoader(
      base,
      loaderOptions,
      loaderExclude,
      resolveWebpackPluginOptions(resolved, layer),
    );
  };

  return {
    ...cfg,
    experimental: {
      ...(cfg.experimental ?? {}),
      swcPlugins,
    },
    webpack: wrappedWebpack,
  };
}

/**
 * Build a `createRequire`-backed resolver anchored at THIS module's
 * file, regardless of whether the runtime loaded the package as ESM
 * (`import.meta.url`) or CJS (`__filename`). `createRequire` accepts
 * either form directly — a file URL or an absolute path — so we can
 * hand it the raw value without round-tripping through
 * `pathToFileURL` / `fileURLToPath` / `dirname`.
 *
 * The `__filename` reference is guarded behind a `typeof` check so
 * strict ESM hosts that don't define it (some bundlers, Deno-compat
 * shims) don't trip a `ReferenceError` while evaluating the ternary.
 */
function getRequire(): NodeRequire {
  if (typeof import.meta !== 'undefined' && import.meta.url) {
    return createRequire(import.meta.url);
  }
  if (typeof __filename !== 'undefined') {
    return createRequire(__filename);
  }
  // Last-resort fallback: anchor at the current working directory.
  // Reached only on truly exotic hosts with neither `import.meta.url`
  // nor `__filename`.
  return createRequire(`${process.cwd()}/`);
}

/**
 * Resolve the Next.js-targeted WASM artefact path via the loader
 * subpath. `@cassida/swc-plugin` ships two builds (modern for
 * Rspack / @swc/core mainline, next for `@next/swc`) — we deliberately
 * pick `wasmPathNext` because the SWC plugin ABI is version-bound to
 * the host's swc_core and Next.js 16.2.x pins swc_core 57.0.0, which
 * the modern (66.x) WASM is not ABI-compatible with. Loading the
 * wrong build manifests as "failed to invoke plugin" on every file.
 *
 * Going through the `loader` subpath avoids
 * `require.resolve('@cassida/swc-plugin')` — the package's `main`
 * points at the modern WASM file directly and some toolchains stumble
 * on that.
 */
function resolveWasmPath(): string {
  const req = getRequire();
  const loaderEntry = req('@cassida/swc-plugin/loader') as {
    wasmPathNext: string;
  };
  return loaderEntry.wasmPathNext;
}

type SwcPluginEntry = [string, Record<string, unknown>];

function readSwcPlugins(cfg: NextConfig): readonly SwcPluginEntry[] {
  const experimental = cfg.experimental as
    | { swcPlugins?: readonly SwcPluginEntry[] }
    | undefined;
  return experimental?.swcPlugins ?? [];
}

type NextWebpackHook = NonNullable<NextConfig['webpack']>;
type WebpackConfig = Parameters<NextWebpackHook>[0];

function injectIrLoader(
  config: WebpackConfig,
  options: IrLoaderOptions,
  exclude: RegExp | string | ((path: string) => boolean) | null,
  webpackPluginOptions: VirtualCssOptions,
): WebpackConfig {
  const loaderRule = {
    test: /\.[cm]?[jt]sx?$/,
    enforce: 'post' as const,
    // Default to skipping node_modules — third-party packages don't
    // carry Cassida IR. Monorepo consumers with symlinked workspace
    // packages can override via `loaderExclude` so chains inside
    // those packages still get compiled.
    ...(exclude !== null ? { exclude } : {}),
    use: [
      {
        loader: cassidaIrLoaderPath(),
        options,
      },
    ],
  };
  const rules = (config.module?.rules ?? []) as unknown[];
  const existingPlugins = (config.plugins ?? []) as unknown[];
  // Next.js invokes the user's `webpack` hook once per compilation
  // (client + server + edge + middleware in App Router). A naive
  // append would land N CassidaWebpackPlugin instances on the same
  // shared config object, each with its own listener wired into
  // the singleton store — redundant rewrites at best, double-fire
  // bugs at worst. Check for an existing instance before append,
  // both `instanceof` (same realm) and constructor-name (different
  // realm — bundled / re-exported / hoisted copies).
  const hasCassidaPlugin = existingPlugins.some(
    (p) =>
      p instanceof CassidaWebpackPlugin ||
      (typeof p === 'object' &&
        p !== null &&
        (p as { constructor?: { name?: string } }).constructor?.name ===
          'CassidaWebpackPlugin'),
  );
  return {
    ...config,
    module: {
      ...(config.module ?? {}),
      rules: [...rules, loaderRule],
    },
    plugins: hasCassidaPlugin
      ? existingPlugins
      : [...existingPlugins, new CassidaWebpackPlugin(webpackPluginOptions)],
  };
}

/**
 * Resolve the enabled entries in `options.plugins` (the declarative
 * convenience form) into actual `CassPlugin` instances.
 *
 * Phase 1.x scope:
 *   - `hoverFix`: synchronously require'd from `@cassida/plugin-hover-fix`.
 *
 * The remaining flags (`conditional`, `print`, `globalCss`) wire to
 * pieces that aren't CSS plugins:
 *   - `conditional` is a parser plugin and needs the Phase 4 parser-
 *     plugin API on the SWC side before it can fire here.
 *   - `print` is a CSS-string factory; it's served through the global
 *     CSS pipeline, not via `compileOps`.
 *   - `globalCss` is a Vite plugin; the Next.js equivalent is a
 *     separate webpack integration that lands in a follow-up.
 * Until each lands, enabling its flag throws at config time rather
 * than silently no-op'ing: a recognised-but-inert option is a semver
 * trap. Each throw is removed as its feature is wired.
 */
function resolveDeclarativeCssPlugins(
  flags: NextCassidaOptions['plugins'],
): CassPlugin[] {
  if (!flags) return [];
  const plugins: CassPlugin[] = [];
  if (flags.hoverFix) {
    try {
      // `@cassida/plugin-hover-fix` exports a default `hoverFix()`
      // factory. createRequire'd CJS surfaces ESM defaults as
      // `mod.default`; the type cast threads both possibilities.
      const mod = getRequire()('@cassida/plugin-hover-fix') as
        | { default: (opts?: unknown) => CassPlugin }
        | ((opts?: unknown) => CassPlugin);
      const factory = typeof mod === 'function' ? mod : mod.default;
      plugins.push(factory());
    } catch (cause) {
      throw new Error(
        `[cassida/next-plugin] options.plugins.hoverFix is enabled but @cassida/plugin-hover-fix could not be required: ${(cause as Error).message}`,
      );
    }
  }
  const STUB_REASONS = {
    conditional:
      'conditional JSX-spread lifting needs the SWC-side parser-plugin API, which the Next.js path does not expose yet',
    print:
      'print preflight is a CSS-string factory; serve printPreflight() through your global CSS instead of this flag',
    globalCss:
      'global CSS injection is a Vite-only plugin; the Next.js equivalent is not implemented yet',
  } as const;
  for (const stubKey of ['conditional', 'print', 'globalCss'] as const) {
    if (flags[stubKey]) {
      throw new Error(
        `[cassida/next-plugin] options.plugins.${stubKey} is not supported in the Next.js path: ${STUB_REASONS[stubKey]}. Remove the flag to proceed.`,
      );
    }
  }
  return plugins;
}

function cassidaIrLoaderPath(): string {
  // Relative resolve sidesteps the package's `exports` map. The
  // map only exposes `.` and `./package.json`, so a
  // `require.resolve('@cassida/next-plugin/dist/ir-loader.js')`
  // call would land on `ERR_PACKAGE_PATH_NOT_EXPORTED` in modern
  // Node. The relative form anchors at this file's directory and
  // walks the publish layout directly.
  return getRequire().resolve('./ir-loader.js');
}

export type { CassConfig, CassPlugin, CassParserPlugin, PathAliases };
