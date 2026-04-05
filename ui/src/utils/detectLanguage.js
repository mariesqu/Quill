/**
 * Lightweight heuristic language detection — pure JS, no dependencies.
 * Uses Unicode script ranges for non-Latin languages and common word
 * frequency for Latin-script languages.
 * Returns a language name string or null (likely English / unknown).
 */

// High-frequency function words per language (Latin scripts)
const WORD_SIGNATURES = {
  French:     ["le","la","les","de","du","un","une","des","je","tu","vous","nous","est","sont","avec","pour","dans","sur","pas","ne","au"],
  Spanish:    ["el","la","los","las","de","en","y","que","un","una","es","son","con","para","por","como","pero","no","lo","al"],
  German:     ["der","die","das","ein","eine","ist","sind","ich","du","wir","sie","mit","auf","von","zu","nicht","dem","den","sich"],
  Portuguese: ["o","a","os","as","de","em","e","que","um","uma","é","são","com","para","por","como","não","no","na","se"],
  Italian:    ["il","la","lo","i","le","di","da","in","e","un","è","sono","con","per","come","non","che","si","gli","del"],
  Dutch:      ["de","het","een","van","is","zijn","in","op","met","voor","niet","dat","dit","er","maar","als","ook","dan","nog"],
  Polish:     ["w","i","z","na","do","że","się","nie","to","jak","jest","ale","po","przy","przez","co","go","jej","mu"],
};

/**
 * Count characters in a Unicode range.
 */
function countInRange(chars, start, end) {
  return chars.filter((c) => {
    const cp = c.codePointAt(0);
    return cp >= start && cp <= end;
  }).length;
}

/**
 * Detect the language of the given text.
 * Returns a language name (e.g. "French") or null if uncertain / likely English.
 */
export function detectLanguage(text) {
  if (!text || text.trim().length < 15) return null;

  const chars = [...text];
  const total = chars.length;

  // 1. Japanese — check for Hiragana/Katakana FIRST (before CJK, to avoid Chinese overlap)
  //    Hiragana: 0x3040–0x309F | Katakana: 0x30A0–0x30FF
  const hiraganaCount  = countInRange(chars, 0x3040, 0x309F);
  const katakanaCount  = countInRange(chars, 0x30A0, 0x30FF);
  if ((hiraganaCount + katakanaCount) / total > 0.08) return "Japanese";

  // 2. Chinese — CJK Unified Ideographs (no Hiragana/Katakana present from step 1)
  const cjkCount = countInRange(chars, 0x4E00, 0x9FFF);
  if (cjkCount / total > 0.15) return "Chinese";

  // 3. Other unambiguous scripts
  const UNAMBIGUOUS = [
    { name: "Korean",  start: 0xAC00, end: 0xD7A3 },
    { name: "Arabic",  start: 0x0600, end: 0x06FF },
    { name: "Russian", start: 0x0400, end: 0x04FF },
    { name: "Greek",   start: 0x0370, end: 0x03FF },
    { name: "Hebrew",  start: 0x0590, end: 0x05FF },
    { name: "Thai",    start: 0x0E00, end: 0x0E7F },
    { name: "Hindi",   start: 0x0900, end: 0x097F },
  ];
  for (const script of UNAMBIGUOUS) {
    if (countInRange(chars, script.start, script.end) / total > 0.15) return script.name;
  }

  // 4. Latin-script word matching
  const words = text.toLowerCase().match(/\b[a-zàáâãäåæçèéêëìíîïðñòóôõöùúûüý]+\b/g) || [];
  if (words.length < 4) return null;

  let bestLang  = null;
  let bestScore = 0;

  for (const [lang, signature] of Object.entries(WORD_SIGNATURES)) {
    const sigSet = new Set(signature);
    const score  = words.filter((w) => sigSet.has(w)).length / words.length;
    if (score > bestScore && score > 0.07) {
      bestScore = score;
      bestLang  = lang;
    }
  }

  return bestLang; // null = probably English or not enough signal
}
