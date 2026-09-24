import { invoke } from "@tauri-apps/api/core";
import type { EpubReaderApi, ReadingUnit } from "$lib/types";
import { isTauriRuntime } from "$lib/isTauri";

/**
 * Чтение вслух. Движки: системный синтез (Web Speech), Piper или espeak-ng
 * (локально, через бэкенд). Текущий абзац подсвечивается, страницы листаются сами.
 */

export type TtsEngine = "system" | "piper" | "espeak-ng";

export interface TtsSettings {
  engine: TtsEngine;
  /** Web Speech: voiceURI */
  voiceURI: string;
  /** Piper: путь к модели .onnx; espeak-ng: имя голоса */
  localVoice: string;
  rate: number;
}

const KEY = "reader.tts.v1";

function load(): TtsSettings {
  const d: TtsSettings = { engine: "system", voiceURI: "", localVoice: "", rate: 1 };
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(KEY) : null;
    if (raw) return { ...d, ...(JSON.parse(raw) as Partial<TtsSettings>) };
  } catch {
    /* ignore */
  }
  return d;
}

export const tts = $state<{
  s: TtsSettings;
  active: boolean;
  paused: boolean;
  loading: boolean;
  error: string | null;
  current: string;
}>({ s: load(), active: false, paused: false, loading: false, error: null, current: "" });

export function saveTts(patch: Partial<TtsSettings>) {
  tts.s = { ...tts.s, ...patch };
  try {
    localStorage.setItem(KEY, JSON.stringify(tts.s));
  } catch {
    /* ignore */
  }
}

export function systemTtsAvailable(): boolean {
  return typeof window !== "undefined" && "speechSynthesis" in window;
}

export function systemVoices(): SpeechSynthesisVoice[] {
  if (!systemTtsAvailable()) return [];
  return speechSynthesis.getVoices();
}

export async function localEngines(): Promise<string[]> {
  if (!isTauriRuntime()) return [];
  try {
    return await invoke<string[]>("tts_engines");
  } catch {
    return [];
  }
}

/** Длинные абзацы режем по предложениям — синтезаторы не любят простыни. */
function sentences(text: string): string[] {
  const parts = text.match(/[^.!?…]+[.!?…]+["»”)]*\s*|[^.!?…]+$/g) ?? [text];
  const out: string[] = [];
  let buf = "";
  for (const p of parts) {
    if ((buf + p).length > 280 && buf) {
      out.push(buf.trim());
      buf = p;
    } else buf += p;
  }
  if (buf.trim()) out.push(buf.trim());
  return out;
}

let session = 0;
let audio: HTMLAudioElement | null = null;
let resumeFn: (() => void) | null = null;
let skipTo: number | null = null;
let markedUnit: ReadingUnit | null = null;

function speakSystem(text: string, my: number): Promise<void> {
  return new Promise((resolve) => {
    const u = new SpeechSynthesisUtterance(text);
    const voice = systemVoices().find((v) => v.voiceURI === tts.s.voiceURI);
    if (voice) u.voice = voice;
    u.lang = voice?.lang ?? "ru-RU";
    u.rate = tts.s.rate;
    u.onend = () => resolve();
    u.onerror = () => resolve();
    if (my !== session) return resolve();
    speechSynthesis.speak(u);
  });
}

const audioCache = new Map<string, Promise<string>>();

function synthLocal(text: string): Promise<string> {
  const key = `${tts.s.engine}|${tts.s.localVoice}|${tts.s.rate}|${text}`;
  let p = audioCache.get(key);
  if (!p) {
    p = invoke<string>("tts_synthesize", {
      engine: tts.s.engine,
      text,
      voice: tts.s.localVoice || null,
      rate: tts.s.rate,
    });
    audioCache.set(key, p);
    if (audioCache.size > 24) audioCache.delete(audioCache.keys().next().value!);
  }
  return p;
}

async function speakLocal(text: string, my: number, next?: string): Promise<void> {
  const b64 = await synthLocal(text);
  if (next) void synthLocal(next).catch(() => {});
  if (my !== session) return;
  await new Promise<void>((resolve) => {
    audio = new Audio(`data:audio/wav;base64,${b64}`);
    audio.onended = () => resolve();
    audio.onerror = () => resolve();
    void audio.play().catch(() => resolve());
  });
}

async function waitIfPaused(my: number) {
  while (tts.paused && my === session) {
    await new Promise<void>((r) => (resumeFn = r));
  }
}

/** Читать вслух от текущего места; главы переключаются автоматически. */
export async function startReading(api: EpubReaderApi) {
  stopReading();
  if (!api.unitsFromHere) return;
  const my = ++session;
  tts.active = true;
  tts.paused = false;
  tts.error = null;
  try {
    let units: ReadingUnit[] = await api.unitsFromHere();
    for (;;) {
      for (let i = 0; i < units.length; i++) {
        if (my !== session) return;
        const u = units[i]!;
        await u.reveal?.();
        u.mark?.();
        markedUnit = u;
        tts.current = u.text;
        const parts = sentences(u.text);
        for (let k = 0; k < parts.length; k++) {
          await waitIfPaused(my);
          if (my !== session || skipTo != null) break;
          tts.loading = tts.s.engine !== "system";
          try {
            if (tts.s.engine === "system") await speakSystem(parts[k]!, my);
            else await speakLocal(parts[k]!, my, parts[k + 1] ?? units[i + 1]?.text.slice(0, 280));
          } catch (e) {
            tts.error = e instanceof Error ? e.message : String(e);
            u.unmark?.();
            stopReading();
            return;
          } finally {
            tts.loading = false;
          }
        }
        u.unmark?.();
        markedUnit = null;
        if (skipTo != null) {
          const target = Math.max(0, Math.min(units.length - 1, i + skipTo));
          i = target - 1;
          skipTo = null;
        }
      }
      if (my !== session) return;
      const moved = await api.advanceChapter?.();
      if (!moved) break;
      await new Promise((r) => setTimeout(r, 350));
      units = await api.unitsFromHere();
      if (!units.length) break;
    }
  } finally {
    if (my === session) {
      tts.active = false;
      tts.current = "";
    }
  }
}

export function pauseReading() {
  tts.paused = true;
  if (tts.s.engine === "system") speechSynthesis.pause();
  else audio?.pause();
}

export function resumeReading() {
  tts.paused = false;
  if (tts.s.engine === "system") speechSynthesis.resume();
  else void audio?.play();
  resumeFn?.();
  resumeFn = null;
}

/** Перейти на абзац вперёд/назад. */
export function skipParagraph(delta: number) {
  skipTo = delta > 0 ? 1 : -1;
  if (tts.s.engine === "system") speechSynthesis.cancel();
  else if (audio) {
    audio.pause();
    audio.dispatchEvent(new Event("ended"));
  }
  if (tts.paused) resumeReading();
}

export function stopReading() {
  session++;
  markedUnit?.unmark?.();
  markedUnit = null;
  skipTo = null;
  tts.active = false;
  tts.paused = false;
  tts.current = "";
  if (systemTtsAvailable()) speechSynthesis.cancel();
  if (audio) {
    audio.pause();
    audio.dispatchEvent(new Event("ended"));
    audio = null;
  }
  resumeFn?.();
  resumeFn = null;
}
