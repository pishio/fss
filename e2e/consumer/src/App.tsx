import { useState } from 'react';
import { cas, type CassChain } from '@cassida/core';

// Same-file mixin — exercises the parser's tryFunctionComposition path.
const withCard = (c: CassChain) =>
  c.padding(16).borderRadius(8).backgroundColor('#fff').color('#111');

export function App() {
  const [hue, setHue] = useState(180);
  const [muted, setMuted] = useState(false);

  return (
    <main {...cas().padding(24).maxWidth(720).props}>
      <h1 {...cas().fontSize(24).fontWeight(700).color('#111').props}>Cassida e2e</h1>

      <section {...withCard(cas()).props}>
        <p {...cas().margin(0).fontSize(14).props}>Static chain compiles to a class.</p>
      </section>

      {/* `.cond()` fed through same-file composition — the branch
          placeholders must survive tryFunctionComposition and expand
          into build-time Cartesian leaves (ternary className), never
          the runtime fallback. The branches set `opacity`, which the
          mixin body doesn't touch, so LIFO collapse can't merge the
          two leaves into one. */}
      <section
        {...withCard(
          cas().cond(
            muted,
            (c) => c.opacity(0.55),
            (c) => c.opacity(1),
          ),
        ).props}
      >
        <p {...cas().margin(0).fontSize(14).props}>
          Composed cond: {muted ? 'muted' : 'full'} card.
        </p>
      </section>

      <button onClick={() => setMuted((m) => !m)}>toggle muted</button>

      {/* Modifier scope (:hover) — must produce a single class with a
          nested rule, never a separate selector. */}
      <button
        {...cas()
          .padding(8, 'px')
          .backgroundColor('#3b82f6')
          .color('#fff')
          .borderRadius(4)
          .cursor('pointer')
          .hover((c) => c.backgroundColor('#2563eb'))
          .props}
      >
        Click me
      </button>

      {/* Dynamic value — must materialize as a CSS custom property
          driven from inline style, with the class itself stable. */}
      <div
        {...cas()
          .padding(16)
          .marginTop(16)
          .color(`hsl(${hue}deg 70% 50%)`)
          .props}
      >
        Dynamic colour: hue = {hue}
      </div>

      <button onClick={() => setHue((h) => (h + 30) % 360)}>shift hue</button>
    </main>
  );
}
