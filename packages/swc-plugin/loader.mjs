// ESM resolver for the WASM artefacts. We ship two builds because
// the SWC plugin ABI is version-bound to the host's `swc_core`:
//
//   • `wasmPath` / `wasmPathModern` — built against swc_core 66.x.
//     ABI-compatible with Rspack, @swc/core mainline, swc-loader,
//     swc-node, @vitejs/plugin-react-swc.
//   • `wasmPathNext` — built against swc_core 57.0.0. ABI-compatible
//     with `@next/swc` shipped in Next.js 16.2.x. Use this from inside
//     `@cassida/next-plugin` / any other Next.js integration.
//
// Pairs with the CommonJS sibling for dual-format consumption.
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const here = path.dirname(fileURLToPath(import.meta.url));

export const wasmPath = path.join(here, 'dist', 'cassida_swc_plugin.wasm');
export const wasmPathModern = wasmPath;
export const wasmPathNext = path.join(
  here,
  'dist',
  'cassida_swc_plugin.next.wasm',
);
