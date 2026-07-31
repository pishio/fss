import type React from 'react';
import { cas } from '@cassida/core';
import { useT } from '../lib/locale.js';
import { Code } from '../components/Code.js';

export default function NextSetup(): React.JSX.Element {
  const copy = useT({
    en: {
      title: 'Next.js setup',
      lead: 'Three blocks: install, wrap next.config.mjs, import the virtual stylesheet from app/layout.tsx. After that the App Router renders one cas-XXXXXXXX class per element and ships the styles inside @layer cas.',
      constraintsHeading: 'Supported configuration',
      constraintsAppRouter:
        'App Router only. Pages Router has no @cassida/next-plugin entry; Server Components and Server Actions are part of the bridge that delivers styles into the client bundle.',
      constraintsLts:
        'Next.js 16.2.x only. The next-targeted WASM is ABI-pinned to the swc_core embedded in Next.js 16.2 (57.0.0), so Next.js 15 and older cannot load it — stay on the last Cassida release published before the repin. The dual-WASM build follows the current stable Next.js line.',
      constraintsTurbopack:
        'Webpack only for now. Turbopack support is queued behind Phase 1.5 — the webpack-virtual-modules plumbing that delivers virtual.css does not exist in Turbopack yet.',
      installHeading: 'Install',
      configHeading: 'next.config.mjs',
      configCopy: 'Wrap your existing config with withCassida(). The wrapper registers the SWC plugin under experimental.swcPlugins and attaches the webpack plugin that emits virtual.css.',
      layoutHeading: 'app/layout.tsx',
      layoutCopy: 'Import the virtual stylesheet at the root of the App Router tree. The import has no JS payload — the CSS lives in the bundle as a regular stylesheet, the import is the signal to Next.js that the route depends on it.',
      verifyHeading: 'Verify',
      verifyCopy: 'Run a production build and open DevTools on the rendered page. The styled element carries one cas- class, the stylesheet under .next/static/css contains @layer cas, and the rendered HTML / RSC payload references the same class hash. No cas() call survives in the client JS chunks.',
      deeperHeading: 'Going deeper',
      deeperCopy: 'The Quick Start above is what 95% of users need. If you want to know what the SWC plugin does, what the webpack plugin emits, or how server-only styles reach the client bundle, the @cassida/swc-plugin and @cassida/next-plugin pages explain the internals.',
      monorepoHeading: 'Monorepo with output: \'standalone\'',
      monorepoCopy: 'If your Next.js app sits inside a monorepo with output: \'standalone\', set outputFileTracingRoot to the monorepo root, not to the app directory. Setting it to the app directory silences Next.js\'s "multiple lockfiles" warning but silently drops @cassida/* from the standalone bundle\'s node_modules tree.',
    },
    ja: {
      title: 'Next.js セットアップ',
      lead: '3 ブロックで動く。install、next.config.mjs を withCassida() で包む、app/layout.tsx で virtual な stylesheet を import する。これで App Router は 1 要素 = 1 個の cas-XXXXXXXX クラスをレンダリングし、CSS は @layer cas に入って出力される。',
      constraintsHeading: '対応構成',
      constraintsAppRouter:
        'App Router 専用。Pages Router 用の @cassida/next-plugin エントリは無い。Server Component と Server Action から書かれたスタイルもクライアントの bundle まで届けるための bridge が前提になるため、App Router の構造に依存している。',
      constraintsLts:
        'Next.js 16.2 系のみ対応。Next.js 向け WASM は Next.js 16.2 が埋め込む swc_core (57.0.0) に ABI を pin しているため、Next.js 15 以前では読み込めない。15 系のアプリは repin 前の最後の Cassida リリースに留まること。dual-WASM ビルドは現行 stable の Next.js ラインに追従する。',
      constraintsTurbopack:
        '現在は webpack のみ。Turbopack 対応は Phase 1.5 に積まれている。virtual.css を配信している webpack-virtual-modules 相当の仕組みが、まだ Turbopack 側に存在しないため。',
      installHeading: 'インストール',
      configHeading: 'next.config.mjs',
      configCopy: '既存の config を withCassida() で包む。ラッパが SWC プラグインを experimental.swcPlugins に登録し、virtual.css を出力する webpack プラグインを config に追加する。',
      layoutHeading: 'app/layout.tsx',
      layoutCopy: 'App Router のルートで virtual な stylesheet を import する。この import は JS の中身を持たない。CSS は通常の stylesheet として bundle に乗り、import 文は「このルートはこの CSS に依存している」という signal として Next.js に伝わる。',
      verifyHeading: '動作確認',
      verifyCopy: 'production ビルドを走らせて、表示されたページを DevTools で開く。スタイル付き要素には cas- クラスが 1 つだけ、.next/static/css にある stylesheet には @layer cas が含まれ、レンダリングされた HTML と RSC payload は同じ class ハッシュを参照しているはずだ。クライアント JS チャンクには cas() の呼び出しは残らない。',
      deeperHeading: 'さらに深く知る',
      deeperCopy: '上の Quick Start で 95% のユーザーは十分。SWC プラグインが何をしているか、webpack プラグインが何を emit しているか、サーバ側だけで書かれたスタイルがどうクライアント bundle に届くか。これらが知りたいときは @cassida/swc-plugin と @cassida/next-plugin のページに internals が書いてある。',
      monorepoHeading: 'monorepo + output: \'standalone\' の組み合わせ',
      monorepoCopy: 'Next.js アプリが monorepo の中にあって output: \'standalone\' を使う場合、outputFileTracingRoot は monorepo のルートに設定する。アプリのディレクトリに設定すると Next.js の "multiple lockfiles" 警告は消えるが、standalone bundle の node_modules ツリーから @cassida/* が黙って落ちる。',
    },
  });

  return (
    <article {...cas().display('flex').flexDirection('column').gap(16).props}>
      <h1 {...cas().fontSize(36).marginBottom(8).props}>{copy.title}</h1>
      <Code source={`# 1. install
pnpm add @cassida/core @cassida/next-plugin

# 2. next.config.mjs
import { withCassida } from '@cassida/next-plugin';
export default withCassida({ /* your existing next config */ });

# 3. app/layout.tsx
import '@cassida/next-plugin/virtual.css';`} />
      <p>{copy.lead}</p>

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.constraintsHeading}</h2>
      <ul {...cas().display('flex').flexDirection('column').gap(8).props}>
        <li>{copy.constraintsAppRouter}</li>
        <li>{copy.constraintsLts}</li>
        <li>{copy.constraintsTurbopack}</li>
      </ul>

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.installHeading}</h2>
      <Code source={`pnpm add @cassida/core @cassida/next-plugin`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.configHeading}</h2>
      <p>{copy.configCopy}</p>
      <Code source={`// next.config.mjs
import { withCassida } from '@cassida/next-plugin';

export default withCassida({
  reactStrictMode: true,
  // ...your existing Next.js config
});`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.layoutHeading}</h2>
      <p>{copy.layoutCopy}</p>
      <Code source={`// app/layout.tsx
import type React from 'react';
import '@cassida/next-plugin/virtual.css';

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.verifyHeading}</h2>
      <p>{copy.verifyCopy}</p>
      <Code source={`pnpm build && pnpm start
# DevTools: every styled element → exactly one cas-XXXXXXXX class
# .next/static/css/*.css → contains @layer cas { ... }
# .next/static/chunks/*.js → no cas( occurrences`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.monorepoHeading}</h2>
      <p>{copy.monorepoCopy}</p>
      <Code source={`// next.config.mjs (monorepo + standalone)
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { withCassida } from '@cassida/next-plugin';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default withCassida({
  output: 'standalone',
  outputFileTracingRoot: path.join(__dirname, '../..'),
});`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.deeperHeading}</h2>
      <p>{copy.deeperCopy}</p>
    </article>
  );
}
