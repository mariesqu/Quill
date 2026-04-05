/**
 * Lightweight heuristic language detection — pure JS, no dependencies.
 * Uses Unicode script ranges for non-Latin languages and common word
 * frequency for Latin-script languages.
 * Returns a language name string or null (likely English / unknown).
 */

// Non-Latin script ranges — unambiguous when enough characters match
const SCRIPTS = [
  { name: "Arabic",      start: 0x0600, end: 0x06FF },
  { name: "Chinese",     start: 0x4E00, end: 0x9FFF },
  { name: "Japanese",    start: 0x3040, end: 0x30FF },
  { name: "Korean",      start: 0xAC00, end: 0xD7A3 },
  { name: "Russian",     start: 0x0400, end: 0x04FF },
  { name: "Greek",       start: 0x0370, end: 0x03FF },
  { name: "Hebrew",      start: 0x0590, end: 0x05FF },
  { name: "Thai",        start: 0x0E00, end: 0x0E7F },
  { name: "Hindi",       start: 0x0900, end: 0x097F },
];

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
 * Detect the language of the given text.
 * Returns a language name (e.g. "French") or null if uncertain / likely English.
 */
export function detectLanguage(text) {
  if (!text || text.trim().length < 15) return null;

  // 1. Check non-Latin scripts first — these are unambiguous
  const chars = [...text];
  for (const script of SCRIPTS) {
    const count = chars.filter((c) => {
      const cp = c.codePointAt(0);
      return cp >= script.start && cp <= script.end;
    }).length;
    // >15% of characters from a single script → confident detection
    if (count / chars.length > 0.15) return script.name;
  }

  // 2. Latin-script word matching
  const words = text.toLowerCase().match(/\b[a-zàáâãäåæçèéêëìíîïðñòóôõöùúûüý]+\b/g) || [];
  if (words.length < 4) return null;

  let bestLang  = null;
  let bestScore = 0;

  for (const [lang, signature] of Object.entries(WORD_SIGNATURES)) {
    const sigSet = new Set(signature);
    const matches = words.filter((w) => sigSet.has(w)).length;
    const score   = matches / words.length;
    if (score > bestScore && score > 0.07) {
      bestScore = score;
      bestLang  = lang;
    }
  }

  return bestLang; // null = probably English or not enough signal
}
