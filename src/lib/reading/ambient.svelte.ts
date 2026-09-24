/**
 * Фоновые звуки, синтезированные на лету (Web Audio) — без файлов и сети.
 * Дождь, камин, ветер, море, кафе и мягкий «коричневый» шум.
 */

export type AmbientId = "rain" | "fire" | "wind" | "ocean" | "cafe" | "brown";

export const AMBIENT_OPTIONS: { id: AmbientId; label: string; icon: string }[] = [
  { id: "rain", label: "Дождь", icon: "☔" },
  { id: "fire", label: "Камин", icon: "🔥" },
  { id: "wind", label: "Ветер в лесу", icon: "🌲" },
  { id: "ocean", label: "Море", icon: "🌊" },
  { id: "cafe", label: "Кафе", icon: "☕" },
  { id: "brown", label: "Тихий шум", icon: "◌" },
];

export const ambient = $state<{ current: AmbientId | null; volume: number }>({
  current: null,
  volume: readVolume(),
});

function readVolume(): number {
  try {
    const v = Number(localStorage.getItem("reader.ambientVolume"));
    return v > 0 && v <= 1 ? v : 0.45;
  } catch {
    return 0.45;
  }
}

let ctx: AudioContext | null = null;
let master: GainNode | null = null;
let nodes: AudioNode[] = [];
let timers: ReturnType<typeof setTimeout>[] = [];
let sources: AudioScheduledSourceNode[] = [];

function noiseBuffer(c: AudioContext, kind: "white" | "pink" | "brown", seconds = 6): AudioBuffer {
  const len = c.sampleRate * seconds;
  const buf = c.createBuffer(2, len, c.sampleRate);
  for (let ch = 0; ch < 2; ch++) {
    const d = buf.getChannelData(ch);
    let b0 = 0,
      b1 = 0,
      b2 = 0,
      b3 = 0,
      b4 = 0,
      b5 = 0,
      b6 = 0,
      last = 0;
    for (let i = 0; i < len; i++) {
      const w = Math.random() * 2 - 1;
      if (kind === "white") d[i] = w * 0.5;
      else if (kind === "pink") {
        b0 = 0.99886 * b0 + w * 0.0555179;
        b1 = 0.99332 * b1 + w * 0.0750759;
        b2 = 0.969 * b2 + w * 0.153852;
        b3 = 0.8665 * b3 + w * 0.3104856;
        b4 = 0.55 * b4 + w * 0.5329522;
        b5 = -0.7616 * b5 - w * 0.016898;
        d[i] = (b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362) * 0.11;
        b6 = w * 0.115926;
      } else {
        last = (last + 0.02 * w) / 1.02;
        d[i] = last * 3.2;
      }
    }
  }
  return buf;
}

function loop(c: AudioContext, buf: AudioBuffer): AudioBufferSourceNode {
  const s = c.createBufferSource();
  s.buffer = buf;
  s.loop = true;
  s.start(0, Math.random() * buf.duration);
  sources.push(s);
  return s;
}

function filter(c: AudioContext, type: BiquadFilterType, freq: number, q = 0.7): BiquadFilterNode {
  const f = c.createBiquadFilter();
  f.type = type;
  f.frequency.value = freq;
  f.Q.value = q;
  nodes.push(f);
  return f;
}

function gain(c: AudioContext, v: number): GainNode {
  const g = c.createGain();
  g.gain.value = v;
  nodes.push(g);
  return g;
}

function lfo(c: AudioContext, freq: number, depth: number, target: AudioParam) {
  const o = c.createOscillator();
  o.frequency.value = freq;
  const g = gain(c, depth);
  o.connect(g).connect(target);
  o.start();
  sources.push(o);
}

/** Случайные короткие события: капли, треск поленьев, звон чашек. */
function sprinkle(c: AudioContext, out: AudioNode, every: [number, number], make: () => void) {
  const tick = () => {
    make();
    timers.push(setTimeout(tick, every[0] + Math.random() * (every[1] - every[0])));
  };
  timers.push(setTimeout(tick, every[0]));
  void out;
}

function build(id: AmbientId, c: AudioContext, out: AudioNode) {
  const white = noiseBuffer(c, "white");
  const pink = noiseBuffer(c, "pink");
  const brown = noiseBuffer(c, "brown");
  switch (id) {
    case "rain": {
      loop(c, pink).connect(filter(c, "highpass", 400)).connect(filter(c, "lowpass", 7000)).connect(gain(c, 0.9)).connect(out);
      loop(c, brown).connect(filter(c, "lowpass", 300)).connect(gain(c, 0.35)).connect(out);
      const drops = gain(c, 0.22);
      drops.connect(out);
      sprinkle(c, out, [40, 180], () => {
        const s = c.createBufferSource();
        s.buffer = white;
        const f = c.createBiquadFilter();
        f.type = "bandpass";
        f.frequency.value = 2500 + Math.random() * 4000;
        f.Q.value = 8;
        const g = c.createGain();
        const t = c.currentTime;
        g.gain.setValueAtTime(0.0001, t);
        g.gain.exponentialRampToValueAtTime(0.5 + Math.random() * 0.5, t + 0.005);
        g.gain.exponentialRampToValueAtTime(0.0001, t + 0.05);
        s.connect(f).connect(g).connect(drops);
        s.start(t, Math.random() * 5, 0.06);
      });
      break;
    }
    case "fire": {
      loop(c, brown).connect(filter(c, "lowpass", 480)).connect(gain(c, 0.9)).connect(out);
      const hiss = gain(c, 0.06);
      loop(c, pink).connect(filter(c, "bandpass", 3000, 0.5)).connect(hiss).connect(out);
      lfo(c, 0.13, 0.04, hiss.gain);
      const crack = gain(c, 0.5);
      crack.connect(out);
      sprinkle(c, out, [90, 900], () => {
        const bursts = 1 + Math.floor(Math.random() * 4);
        for (let k = 0; k < bursts; k++) {
          const s = c.createBufferSource();
          s.buffer = white;
          const f = c.createBiquadFilter();
          f.type = "bandpass";
          f.frequency.value = 1200 + Math.random() * 3500;
          f.Q.value = 3;
          const g = c.createGain();
          const t = c.currentTime + k * (0.01 + Math.random() * 0.04);
          g.gain.setValueAtTime(0.0001, t);
          g.gain.exponentialRampToValueAtTime(0.3 + Math.random() * 0.7, t + 0.002);
          g.gain.exponentialRampToValueAtTime(0.0001, t + 0.02 + Math.random() * 0.05);
          s.connect(f).connect(g).connect(crack);
          s.start(t, Math.random() * 5, 0.08);
        }
      });
      break;
    }
    case "wind": {
      const bp = filter(c, "bandpass", 600, 0.9);
      const g = gain(c, 0.8);
      loop(c, pink).connect(bp).connect(g).connect(out);
      lfo(c, 0.07, 350, bp.frequency);
      lfo(c, 0.05, 0.35, g.gain);
      loop(c, brown).connect(filter(c, "lowpass", 200)).connect(gain(c, 0.4)).connect(out);
      // Редкие птицы
      sprinkle(c, out, [5000, 14000], () => {
        const o = c.createOscillator();
        const gg = c.createGain();
        const t = c.currentTime;
        const f0 = 2600 + Math.random() * 1400;
        o.frequency.setValueAtTime(f0, t);
        for (let k = 0; k < 3; k++) {
          o.frequency.exponentialRampToValueAtTime(f0 * (1.25 + Math.random() * 0.3), t + 0.06 + k * 0.14);
          o.frequency.exponentialRampToValueAtTime(f0, t + 0.12 + k * 0.14);
        }
        gg.gain.setValueAtTime(0.0001, t);
        gg.gain.exponentialRampToValueAtTime(0.035, t + 0.02);
        gg.gain.exponentialRampToValueAtTime(0.0001, t + 0.5);
        o.connect(gg).connect(out);
        o.start(t);
        o.stop(t + 0.55);
      });
      break;
    }
    case "ocean": {
      const lp = filter(c, "lowpass", 900);
      const g = gain(c, 0.55);
      loop(c, pink).connect(lp).connect(g).connect(out);
      lfo(c, 0.09, 0.45, g.gain);
      lfo(c, 0.09, 500, lp.frequency);
      loop(c, brown).connect(filter(c, "lowpass", 160)).connect(gain(c, 0.5)).connect(out);
      break;
    }
    case "cafe": {
      const murmur = filter(c, "bandpass", 700, 0.6);
      const g = gain(c, 0.5);
      loop(c, pink).connect(murmur).connect(g).connect(out);
      lfo(c, 0.31, 0.12, g.gain);
      lfo(c, 0.17, 180, murmur.frequency);
      loop(c, brown).connect(filter(c, "lowpass", 250)).connect(gain(c, 0.35)).connect(out);
      sprinkle(c, out, [2500, 9000], () => {
        const o = c.createOscillator();
        const gg = c.createGain();
        const t = c.currentTime;
        o.type = "sine";
        o.frequency.value = 2200 + Math.random() * 1800;
        gg.gain.setValueAtTime(0.0001, t);
        gg.gain.exponentialRampToValueAtTime(0.03, t + 0.004);
        gg.gain.exponentialRampToValueAtTime(0.0001, t + 0.6);
        o.connect(gg).connect(out);
        o.start(t);
        o.stop(t + 0.65);
      });
      break;
    }
    case "brown":
      loop(c, brown).connect(filter(c, "lowpass", 700)).connect(gain(c, 0.9)).connect(out);
      break;
  }
}

function teardown() {
  for (const t of timers) clearTimeout(t);
  timers = [];
  for (const s of sources) {
    try {
      s.stop();
    } catch {
      /* ignore */
    }
    s.disconnect();
  }
  sources = [];
  for (const n of nodes) n.disconnect();
  nodes = [];
}

export async function playAmbient(id: AmbientId) {
  if (!ctx) {
    ctx = new AudioContext();
    master = ctx.createGain();
    master.connect(ctx.destination);
  }
  await ctx.resume();
  const c = ctx;
  const m = master!;
  m.gain.cancelScheduledValues(c.currentTime);
  m.gain.setTargetAtTime(0, c.currentTime, 0.2);
  await new Promise((r) => setTimeout(r, ambient.current ? 450 : 0));
  teardown();
  const bus = c.createGain();
  bus.gain.value = 1;
  bus.connect(m);
  nodes.push(bus);
  build(id, c, bus);
  m.gain.setTargetAtTime(ambient.volume, c.currentTime, 0.8);
  ambient.current = id;
}

export function stopAmbient() {
  if (!ctx || !master) {
    ambient.current = null;
    return;
  }
  master.gain.setTargetAtTime(0, ctx.currentTime, 0.3);
  ambient.current = null;
  setTimeout(() => {
    if (!ambient.current) teardown();
  }, 1200);
}

export function setAmbientVolume(v: number) {
  ambient.volume = v;
  try {
    localStorage.setItem("reader.ambientVolume", String(v));
  } catch {
    /* ignore */
  }
  if (ctx && master && ambient.current) master.gain.setTargetAtTime(v, ctx.currentTime, 0.1);
}
