/**
 * Промпты в стиле TranslateBooksWithLLMs/prompts/prompts.py:
 * system + user, теги SOURCE_TEXT / TRANSLATION, правила вывода.
 * Для PDF кусков вход — JSON-массив строк (раны текста), выход — JSON-массив внутри TRANSLATION.
 */
import {
  INPUT_TAG_IN,
  INPUT_TAG_OUT,
  TRANSLATE_TAG_IN,
  TRANSLATE_TAG_OUT,
} from "./translationTags";

function getOutputFormatSection(
  exampleFormat: string,
  extraRules: string,
): string {
  const additional = extraRules ? `\n${extraRules}` : "";
  return `# OUTPUT FORMAT

**CRITICAL OUTPUT RULES:**
1. Translate ONLY the text between "${INPUT_TAG_IN}" and "${INPUT_TAG_OUT}" tags
2. Your response MUST start with ${TRANSLATE_TAG_IN} (first characters, no text before)
3. Your response MUST end with ${TRANSLATE_TAG_OUT} (last characters, no text after)
4. Include NOTHING before ${TRANSLATE_TAG_IN} and NOTHING after ${TRANSLATE_TAG_OUT}
5. Do NOT add explanations, comments, notes, or greetings${additional}

**INCORRECT examples (DO NOT do this):**
❌ "Here is the translation: ${TRANSLATE_TAG_IN}...${TRANSLATE_TAG_OUT}"
❌ "${TRANSLATE_TAG_IN}...${TRANSLATE_TAG_OUT} (comment after)"
❌ "Sure! ${TRANSLATE_TAG_IN}...${TRANSLATE_TAG_OUT}"
❌ Missing tags entirely

**CORRECT format (ONLY this):**
✅ ${TRANSLATE_TAG_IN}
${exampleFormat}
${TRANSLATE_TAG_OUT}
`;
}

const EXAMPLE_BY_TARGET: Record<string, string> = {
  chinese: "您翻译的文本在这里",
  french: "Votre texte traduit ici",
  spanish: "Su texto traducido aquí",
  german: "Ihr übersetzter Text hier",
  japanese: "翻訳されたテキストはこちら",
  italian: "Il tuo testo tradotto qui",
  portuguese: "Seu texto traduzido aqui",
  russian: "Ваш переведенный текст здесь",
  korean: "번역된 텍스트는 여기에",
};

function exampleForTargetLanguage(targetLang: string, n: number): string {
  const key = targetLang.trim().toLowerCase();
  const sample =
    EXAMPLE_BY_TARGET[key] ?? "Your translated text here";
  return `["${sample} (run 1)", "${sample} (run 2)", …] — exactly ${n} strings, same order as input JSON`;
}

export function buildPdfChunkTranslationPromptPair(opts: {
  chunk: string[];
  sourceLang: string;
  targetLang: string;
}): { system: string; user: string } {
  const { chunk, sourceLang, targetLang } = opts;
  const n = chunk.length;
  const inputJson = JSON.stringify(chunk);

  const pdfJsonRules = `
**PDF TEXT RUNS (STRUCTURED):**
- Between ${INPUT_TAG_IN} and ${INPUT_TAG_OUT} is a JSON array of ${n} separate PDF text runs (order matters).
- Inside ${TRANSLATE_TAG_IN} and ${TRANSLATE_TAG_OUT} output ONLY a valid JSON array of exactly ${n} strings in the same order.
- Keep valid JSON (brackets, commas, double quotes). Translate the string values only; do not drop or merge runs.`;

  const exampleFormat = exampleForTargetLanguage(targetLang, n);
  const outputFormatSection = getOutputFormatSection(exampleFormat, pdfJsonRules);

  const system = `You are a professional ${targetLang} translator and writer.

# TRANSLATION PRINCIPLES

Translate ${sourceLang} to ${targetLang}. Output only the translation.

**PRIORITY ORDER:**
1. Preserve exact names where appropriate
2. Match original tone and formality
3. Use natural ${targetLang} phrasing - never word-for-word
4. Fix grammar/spelling errors in output
5. Translate idioms to ${targetLang} equivalents

**QUALITY CHECK:**
- Does it sound natural to a native ${targetLang} speaker?
- Are all details from the original included?
- Does punctuation follow ${targetLang} conventions?

If unsure between literal and natural phrasing: **choose natural**.

**LAYOUT / STRUCTURE:**
- For this task you output a JSON array of strings inside the translation tags; keep one output string per input run (same count: ${n}).

**FINAL REMINDER: OUTPUT LANGUAGE**

**YOU MUST TRANSLATE INTO ${targetLang.toUpperCase()}.**
Your translation strings must be written in ${targetLang}.
Do NOT write in ${sourceLang} or any other language - ONLY ${targetLang.toUpperCase()}.

${outputFormatSection}`;

  const user = `# TEXT TO TRANSLATE

${INPUT_TAG_IN}
${inputJson}
${INPUT_TAG_OUT}

REMINDER: Output ONLY your translation in this exact format:
${TRANSLATE_TAG_IN}
${exampleFormat}
${TRANSLATE_TAG_OUT}

Start with ${TRANSLATE_TAG_IN} and end with ${TRANSLATE_TAG_OUT}. Nothing before or after.

Provide your translation now:`;

  return { system: system.trim(), user: user.trim() };
}
