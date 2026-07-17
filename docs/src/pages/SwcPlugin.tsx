import type React from 'react';
import { cas } from '@cassida/core';
import { useT } from '../lib/locale.js';
import { Code } from '../components/Code.js';

export default function SwcPlugin(): React.JSX.Element {
  const copy = useT({
    en: {
      title: '@cassida/swc-plugin',
      lead: 'A Rust-authored SWC plugin compiled to WASM. It walks JSX in the source, finds cas() chains used in JSX spread position, and emits an IR-shaped marker comment that the matching @cassida/next-plugin loader picks up at the next stage. It does no styling work on its own — it is the SWC-side half of the build pipeline.',
      dualHeading: 'Dual-WASM build',
      dualCopy: 'The plugin ships as two WASM artefacts side by side. The SWC plugin ABI is version-bound: a plugin compiled against swc_core X.Y can only be loaded by a host whose swc_core is also at X.Y. The two artefacts cover the two host families this matters for.',
      dualModern:
        'cassida_swc_plugin.wasm — built against swc_core 66.x. Used by Rspack, @swc/core, and @vitejs/plugin-react-swc.',
      dualNext:
        'cassida_swc_plugin.next.wasm — built against swc_core 57.0.0. Used by @next/swc 16.2.x (current stable Next.js).',
      dualEntry:
        'The exports map ("." → modern, "./next" → next-targeted) is what lets @cassida/next-plugin pick the correct artefact at config time. Direct consumers (Rspack, @swc/core) import from "@cassida/swc-plugin" and get the modern one.',
      driftHeading: 'ABI drift policy',
      driftCopy: 'Next.js bumps the embedded swc_core periodically — the SWC plugin\'s Next.js-targeted build pinned to 57.0.0 will drift out of alignment with the latest stable release. A weekly GitHub Actions cron compares the pin against the latest stable Next.js release and opens a tracking issue when they diverge. The intent is that older Next.js majors break first if anything breaks; the current stable line stays aligned.',
      routesHeading: 'Three install routes',
      routesNextHeading: 'Through @cassida/next-plugin (default for Next.js users)',
      routesNextCopy: 'You do not install @cassida/swc-plugin directly. It comes in as a transitive dependency of @cassida/next-plugin, which wires the "./next" entry into experimental.swcPlugins for you.',
      routesRspackHeading: 'Direct install for Rspack',
      routesRspackCopy: 'Pass the modern WASM through Rspack\'s SWC plugin slot. Match the host swc_core version (Rspack 0.x → swc_core 66.x).',
      routesSwcHeading: 'Direct install for @swc/core',
      routesSwcCopy: 'Same idea via @swc/core\'s jsc.experimental.plugins. Works inside any tool that calls @swc/core directly (bun, esbuild-swc bridges, custom build scripts).',
      pairingHeading: 'What it pairs with',
      pairingCopy: 'The plugin emits IR-shaped comments, not finished CSS. To turn those into a stylesheet you also need @cassida/next-plugin (when consuming under Next.js) or the equivalent loader configured by hand (in custom setups). The Phase 1 release pairs swc-plugin with next-plugin; standalone consumption outside Next.js is on the Phase 1.5 roadmap.',
    },
    ja: {
      title: '@cassida/swc-plugin',
      lead: 'Rust で書かれて WASM にコンパイルされた SWC プラグイン。ソース内の JSX を走査し、JSX spread 位置で使われている cas() チェーンを見つけて、後段の @cassida/next-plugin ローダーが拾うための IR 形式のマーカーコメントを emit する。スタイリングの実作業はここでは行わない。ビルドパイプラインのうち、SWC 側を受け持つ半分にあたる。',
      dualHeading: 'dual-WASM ビルド',
      dualCopy: 'プラグインは 2 つの WASM artefact を並べて配布している。SWC プラグインの ABI はバージョン束縛で、swc_core X.Y を前提にコンパイルされたプラグインは、swc_core が X.Y のホストでないと読み込めない。2 つの artefact は、それが効くホストの 2 系列をカバーしている。',
      dualModern:
        'cassida_swc_plugin.wasm：swc_core 66.x を前提にビルド。Rspack、@swc/core、@vitejs/plugin-react-swc などが対象。',
      dualNext:
        'cassida_swc_plugin.next.wasm：swc_core 57.0.0 を前提にビルド。@next/swc 16.2.x (現行 stable の Next.js) が対象。',
      dualEntry:
        'exports map ("." → modern、"./next" → next 向け) を @cassida/next-plugin が config 時に正しい方を選び取る材料にしている。Rspack や @swc/core から直接読む場合は "@cassida/swc-plugin" を import すれば modern 版になる。',
      driftHeading: 'ABI drift ポリシー',
      driftCopy: 'Next.js は埋め込み swc_core を定期的に上げる。SWC プラグインの Next.js 向けビルドが pin している 57.0.0 は、最新の stable と少しずつズレていく。毎週の GitHub Actions cron が、現行 stable の Next.js リリースに含まれる swc_core 版数と pin を比較し、ズレた瞬間に tracking issue を立てる。基本姿勢は「壊れるなら古い Next.js major から壊れる、現行 stable のラインは常に揃え続ける」というものだ。',
      routesHeading: '3 つの install ルート',
      routesNextHeading: '@cassida/next-plugin 経由 (Next.js ユーザーの既定)',
      routesNextCopy: '@cassida/swc-plugin を直接 install することはない。@cassida/next-plugin の transitive 依存として一緒に入り、next-plugin が "./next" エントリを experimental.swcPlugins に登録する。',
      routesRspackHeading: 'Rspack で直接 install',
      routesRspackCopy: 'Rspack の SWC プラグインスロットに modern 版 WASM を渡す。ホスト側の swc_core バージョンを揃えること (Rspack 0.x → swc_core 66.x)。',
      routesSwcHeading: '@swc/core で直接 install',
      routesSwcCopy: '@swc/core の jsc.experimental.plugins から同じ流れで読む。@swc/core を直接呼ぶツール (bun、esbuild-swc bridge、独自のビルドスクリプト) ならどれでも同じ手順で使える。',
      pairingHeading: '組み合わせ前提',
      pairingCopy: 'このプラグインが emit するのは IR 形式のコメントであって、完成した CSS ではない。コメントを stylesheet に変換するには @cassida/next-plugin (Next.js で使う場合) か、それに相当するローダーを手動で設定する必要がある。Phase 1 では swc-plugin + next-plugin の組み合わせを動作確認している。Next.js 以外の単体構成は Phase 1.5 のロードマップに乗っている。',
    },
  });

  return (
    <article {...cas().display('flex').flexDirection('column').gap(16).props}>
      <h1 {...cas().fontSize(36).marginBottom(8).props}>{copy.title}</h1>
      <Code source={`// Most Next.js users don't touch this package directly —
// it comes in transitively via @cassida/next-plugin.
// Direct install is for Rspack / @swc/core consumers.
pnpm add @cassida/swc-plugin`} />
      <p>{copy.lead}</p>

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.dualHeading}</h2>
      <p>{copy.dualCopy}</p>
      <ul {...cas().display('flex').flexDirection('column').gap(8).props}>
        <li>{copy.dualModern}</li>
        <li>{copy.dualNext}</li>
      </ul>
      <p>{copy.dualEntry}</p>
      <Code source={`// package.json (excerpt) — what the dual-WASM exports look like
{
  "exports": {
    ".":            "./dist/cassida_swc_plugin.wasm",
    "./next":       "./dist/cassida_swc_plugin.next.wasm",
    "./package.json": "./package.json"
  }
}`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.driftHeading}</h2>
      <p>{copy.driftCopy}</p>

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.routesHeading}</h2>

      <h3 {...cas().fontSize(18).marginTop(16).props}>{copy.routesNextHeading}</h3>
      <p>{copy.routesNextCopy}</p>
      <Code source={`// next.config.mjs
import { withCassida } from '@cassida/next-plugin';
export default withCassida({});
// → loads cassida_swc_plugin.next.wasm via experimental.swcPlugins`} />

      <h3 {...cas().fontSize(18).marginTop(16).props}>{copy.routesRspackHeading}</h3>
      <p>{copy.routesRspackCopy}</p>
      <Code source={`// rspack.config.js
import { fileURLToPath } from 'node:url';

export default {
  module: {
    rules: [
      {
        test: /\\.(j|t)sx?$/,
        loader: 'builtin:swc-loader',
        options: {
          jsc: {
            experimental: {
              plugins: [
                [fileURLToPath(import.meta.resolve('@cassida/swc-plugin')), {}],
              ],
            },
          },
        },
      },
    ],
  },
};`} />

      <h3 {...cas().fontSize(18).marginTop(16).props}>{copy.routesSwcHeading}</h3>
      <p>{copy.routesSwcCopy}</p>
      <Code source={`// .swcrc
{
  "jsc": {
    "experimental": {
      "plugins": [
        ["@cassida/swc-plugin", {}]
      ]
    }
  }
}`} />

      <h2 {...cas().fontSize(24).marginTop(24).props}>{copy.pairingHeading}</h2>
      <p>{copy.pairingCopy}</p>
    </article>
  );
}
