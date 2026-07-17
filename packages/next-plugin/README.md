# @cassida/next-plugin

Next.js integration for Cassida. One-line drop-in for `next.config.js` that wires up the SWC plugin, the IR-comment loader, the `@layer cas` virtual CSS module, and the optional plugin registry.

**Status: Phase 1 scaffold.** Currently exports the `withCassida()` API surface and the `NextCassidaOptions` type. The actual integration lands in subsequent commits.

## Supported Next.js versions

Cassida supports the current stable Next.js release only (16.2.x at the time of writing). Earlier majors aren't tested in CI; future bumps in `@next/swc`'s embedded `swc_core` may break older Next.js installs of `@cassida/next-plugin` without notice.

**Next.js 15 is no longer supported.** The next-targeted WASM is pinned to the `swc_core` embedded in Next.js 16.2 (57.0.0); on Next.js 15 (swc_core 35) it fails to load with `failed to invoke plugin`. Next.js 15 apps must stay on the last Cassida release published before the repin.

The `@cassida/swc-plugin` package ships two ABI-pinned WASM artefacts — the next-targeted one tracks the `swc_core` pinned by the current stable Next.js release, and won't be backported when Next.js bumps its `swc_core` in a future release.

**Enforcement.** A weekly cron in this repo ([`.github/workflows/swc-core-drift.yml`](../../.github/workflows/swc-core-drift.yml)) watches the `swc_core` version embedded in the current stable Next.js release. When the pin drifts, an issue is auto-opened on this repo with the `Cassida-Phase-1.5` label so the `@cassida/swc-plugin-next` crate can be bumped before consumers' Next.js upgrades break.

## Install

```bash
pnpm add @cassida/next-plugin @cassida/core
```

`@cassida/next-plugin` depends transitively on `@cassida/swc-plugin`, `@cassida/parser`, and `@cassida/compiler`. Optional plugins (`@cassida/plugin-hover-fix`, `@cassida/plugin-conditional`, `@cassida/plugin-print`, `@cassida/plugin-global-css`) are lazy-loaded — they're only required if you enable them via `options.plugins`.

## Usage

```js
// next.config.js
import { withCassida } from '@cassida/next-plugin';

export default withCassida(
  {
    // ordinary next config
    reactStrictMode: true,
    experimental: { typedRoutes: true },
  },
  {
    // cassida options (all optional)
    layer: 'cas',
    hash: { prefix: 'cas-', length: 8 },
    shorthand: { policy: 'strict' },

    // Enable optional plugins
    plugins: {
      hoverFix: true,
      conditional: { shortCircuit: true },
      print: false, // skip the @media print preflight
      globalCss: { preflight: './styles/preflight.css' },
    },

    // Auto-discover tsconfig path aliases (default true)
    pathAliases: true,
  },
);
```

`withCassida()` returns the same `next.config` object, augmented with the SWC plugin registration, webpack/turbopack rules, and any other glue the requested options imply.

## Why a dedicated Next.js wrapper

`@cassida/swc-plugin` alone is just the AST transform; it needs a Node-side post-pass to compile the emitted IR into class names and to bundle the resulting CSS. `@cassida/next-plugin` is that glue, packaged as a one-line config wrapper so consumers don't manually hand-edit `experimental.swcPlugins`, `webpack.module.rules`, or `experimental.turbo.rules`.

It also ships the App Router (RSC) guard that warns when a `cas()` call would land in a Server Component runtime, the lazy plugin loader so you only require what you enable, and the standard `@layer cas` CSS bundle endpoint.

## Monorepo and `output: 'standalone'`

In a monorepo where the Next.js app directory holds its own lockfile but the workspace root also has one, Next.js 15 emits a "multiple lockfiles detected" warning on every build. The conventional fix — setting `outputFileTracingRoot` to the app directory — silences the warning, but it has a second effect that bites later: it also governs which `node_modules` files are copied into the `.next/standalone/` bundle when you use `output: 'standalone'`.

If `outputFileTracingRoot` points at the **app directory**, the standalone bundle won't include the `@cassida/*` packages (they live one or more levels up under the workspace `node_modules`), and the SWC plugin's WASM loader will fail to resolve at runtime in production.

The fix: point `outputFileTracingRoot` at the **workspace root**, not the app directory.

```js
// next.config.mjs
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { withCassida } from '@cassida/next-plugin';

const workspaceRoot = dirname(dirname(fileURLToPath(import.meta.url)));
// (or hard-code if your structure is stable)

export default withCassida({
  // ...
  outputFileTracingRoot: workspaceRoot,
});
```

In single-package consumers (no monorepo), `outputFileTracingRoot` can be left unset — the warning won't fire and the standalone bundle traces correctly by default.

## License

MIT
