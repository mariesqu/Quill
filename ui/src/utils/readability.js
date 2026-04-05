/**
 * Flesch-Kincaid Grade Level calculator — pure JS, no dependencies.
 * Returns a grade level (e.g. 8.2 = 8th grade reading level).
 * Works only for English text; returns null for very short texts.
 */

function syllableCount(word) {
  word = word.toLowerCase().replace(/[^a-z]/g, "");
  if (!word) return 0;
  if (word.length <= 3) return 1;
  // Strip silent e endings and common suffixes
  word = word.replace(/(?:[^laeiouy]es|ed|[^laeiouy]e)$/, "");
  word = word.replace(/^y/, "");
  const matches = word.match(/[aeiouy]{1,2}/g);
  return matches ? matches.length : 1;
}

function countSentences(text) {
  // Count sentence-ending punctuation followed by whitespace or end
  const terminators = (text.match(/[.!?]+[\s\n]|[.!?]+$/g) || []).length;
  return Math.max(1, terminators);
}

/**
 * Compute Flesch-Kincaid Grade Level for the given text.
 * Returns null if text is too short to be meaningful.
 */
export function fleschKincaid(text) {
  const words = text.trim().split(/\s+/).filter(Boolean);
  if (words.length < 5) return null;

  const wordCount     = words.length;
  const sentenceCount = countSentences(text);
  const syllables     = words.reduce((sum, w) => sum + syllableCount(w), 0);

  // FK Grade Level = 0.39*(words/sentences) + 11.8*(syllables/words) - 15.59
  const grade = 0.39 * (wordCount / sentenceCount) + 11.8 * (syllables / wordCount) - 15.59;
  return Math.max(0, Math.round(grade * 10) / 10);
}

/**
 * Return a human label + colour for a FK grade level.
 */
export function gradeLabel(grade) {
  if (grade === null) return null;
  if (grade < 6)  return { label: "Easy",            color: "#4ade80" };
  if (grade < 9)  return { label: `Grade ${Math.round(grade)}`, color: "#fbbf24" };
  if (grade < 13) return { label: `Grade ${Math.round(grade)}`, color: "#fb923c" };
  return           { label: "Complex",           color: "#f87171" };
}
