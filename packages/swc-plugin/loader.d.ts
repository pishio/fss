/**
 * Absolute filesystem path to the modern SWC plugin WASM artefact
 * (built against swc_core 66.x). ABI-compatible with Rspack,
 * @swc/core mainline, swc-loader, swc-node, @vitejs/plugin-react-swc.
 * Alias of {@link wasmPathModern}.
 */
export declare const wasmPath: string;

/** Same as {@link wasmPath}. Use when you want the name to read
 * "modern" at the call site for symmetry with {@link wasmPathNext}. */
export declare const wasmPathModern: string;

/**
 * Absolute filesystem path to the Next.js-targeted SWC plugin WASM
 * artefact (built against swc_core 57.0.0). ABI-compatible with the
 * `@next/swc` binary shipped in Next.js 16.2.x. Use this from inside
 * `@cassida/next-plugin` or any other Next.js integration.
 */
export declare const wasmPathNext: string;
