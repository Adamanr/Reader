/**
 * «Созвездие цитат»: похожие по смыслу выделения притягиваются друг к другу.
 * Без модели — словарное сходство (TF-IDF по усечённым основам слов);
 * с моделью эмбеддингов — косинус по векторам.
 */

const STOP = new Set(
  (
    "это этот эта эти того тому было были быть будет если когда чтобы потому которые который которая которое " +
    "только тоже также очень может можно нужно своей свой свои своих себя себе тебя тебе меня мне него неё нему " +
    "ними нами вами всех всем весь вся всё все ещё уже даже лишь пока тогда здесь там тут где куда какой какая " +
    "какие каждый перед после через между около более менее самый самая сама само сами what that this with have " +
    "from they there their would could which about into when were been will your than then them these those"
  ).split(" "),
);

function stems(text: string): string[] {
  const words = text.toLocaleLowerCase("ru").replace(/ё/g, "е").match(/[\p{L}]{4,}/gu) ?? [];
  const out: string[] = [];
  for (const w of words) {
    if (STOP.has(w)) continue;
    // Грубая «основа»: русская морфология меняет окончания, начало слова стабильнее.
    out.push(w.length > 6 ? w.slice(0, 6) : w);
  }
  return out;
}

export function tfidfVectors(texts: string[]): Map<string, number>[] {
  const docs = texts.map(stems);
  const df = new Map<string, number>();
  for (const d of docs) for (const t of new Set(d)) df.set(t, (df.get(t) ?? 0) + 1);
  const n = texts.length;
  return docs.map((d) => {
    const tf = new Map<string, number>();
    for (const t of d) tf.set(t, (tf.get(t) ?? 0) + 1);
    const v = new Map<string, number>();
    let norm = 0;
    for (const [t, c] of tf) {
      const w = (1 + Math.log(c)) * Math.log(1 + n / (df.get(t) ?? 1));
      v.set(t, w);
      norm += w * w;
    }
    norm = Math.sqrt(norm) || 1;
    for (const [t, w] of v) v.set(t, w / norm);
    return v;
  });
}

function sparseCos(a: Map<string, number>, b: Map<string, number>): number {
  const [s, l] = a.size < b.size ? [a, b] : [b, a];
  let dot = 0;
  for (const [t, w] of s) {
    const o = l.get(t);
    if (o) dot += w * o;
  }
  return dot;
}

function denseCos(a: number[], b: number[]): number {
  let dot = 0;
  let na = 0;
  let nb = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i]! * b[i]!;
    na += a[i]! * a[i]!;
    nb += b[i]! * b[i]!;
  }
  return dot / (Math.sqrt(na * nb) || 1);
}

export function similarityMatrix(texts: string[], embeddings?: number[][] | null): number[][] {
  const n = texts.length;
  const m: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  if (embeddings && embeddings.length === n) {
    for (let i = 0; i < n; i++)
      for (let j = i + 1; j < n; j++) {
        // Эмбеддинги почти всегда положительно похожи — растягиваем шкалу.
        const s = Math.max(0, (denseCos(embeddings[i]!, embeddings[j]!) - 0.35) / 0.65);
        m[i]![j] = s;
        m[j]![i] = s;
      }
    return m;
  }
  const vec = tfidfVectors(texts);
  for (let i = 0; i < n; i++)
    for (let j = i + 1; j < n; j++) {
      const s = sparseCos(vec[i]!, vec[j]!);
      m[i]![j] = s;
      m[j]![i] = s;
    }
  return m;
}

export interface StarPos {
  x: number;
  y: number;
}

/**
 * Силовая раскладка: похожие связаны пружинами, все слегка отталкиваются;
 * звёзды одной книги чуть тянутся друг к другу, чтобы появлялись «созвездия».
 */
export function layoutStars(sim: number[][], groups: number[], seed = 1): StarPos[] {
  const n = sim.length;
  let s = seed;
  const rnd = () => {
    s = (s * 16807) % 2147483647;
    return s / 2147483647;
  };
  const pos = Array.from({ length: n }, () => ({ x: rnd() - 0.5, y: rnd() - 0.5 }));
  const iters = n > 250 ? 160 : 260;
  for (let it = 0; it < iters; it++) {
    const cool = 0.06 * (1 - it / iters) + 0.004;
    const fx = new Array(n).fill(0);
    const fy = new Array(n).fill(0);
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const dx = pos[j]!.x - pos[i]!.x;
        const dy = pos[j]!.y - pos[i]!.y;
        const d2 = dx * dx + dy * dy + 1e-4;
        const d = Math.sqrt(d2);
        // Отталкивание
        let f = -0.0022 / d2;
        // Притяжение по сходству и по книге
        const w = sim[i]![j]! * 1.6 + (groups[i] === groups[j] ? 0.08 : 0);
        f += w * d * 0.9;
        const ux = (dx / d) * f;
        const uy = (dy / d) * f;
        fx[i] += ux;
        fy[i] += uy;
        fx[j] -= ux;
        fy[j] -= uy;
      }
      // Слабая гравитация к центру
      fx[i] -= pos[i]!.x * 0.02;
      fy[i] -= pos[i]!.y * 0.02;
    }
    for (let i = 0; i < n; i++) {
      const len = Math.hypot(fx[i], fy[i]) || 1;
      const step = Math.min(cool, len);
      pos[i]!.x += (fx[i] / len) * step;
      pos[i]!.y += (fy[i] / len) * step;
    }
  }
  // Нормализуем в [0.06, 0.94]
  const xs = pos.map((p) => p.x);
  const ys = pos.map((p) => p.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  return pos.map((p) => ({
    x: 0.06 + ((p.x - minX) / (maxX - minX || 1)) * 0.88,
    y: 0.08 + ((p.y - minY) / (maxY - minY || 1)) * 0.84,
  }));
}

/** Линии созвездий: у каждой звезды — до двух самых похожих соседей. */
export function constellationEdges(sim: number[][], threshold = 0.12): [number, number, number][] {
  const n = sim.length;
  const seen = new Set<string>();
  const out: [number, number, number][] = [];
  for (let i = 0; i < n; i++) {
    const best = sim[i]!
      .map((s, j) => [s, j] as [number, number])
      .filter(([s, j]) => j !== i && s >= threshold)
      .sort((a, b) => b[0] - a[0])
      .slice(0, 2);
    for (const [s, j] of best) {
      const key = i < j ? `${i}-${j}` : `${j}-${i}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push([i, j, s]);
    }
  }
  return out;
}
