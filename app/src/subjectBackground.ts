import { api } from './api';

// Backgrounds are immutable (subjects never change title/description and
// regenerations keep the cached bytes), so an in-memory LRU keyed by
// subject is always fresh. One fetch per subject per session instead of
// one per screen.
const BACKGROUND_CACHE_SIZE = 10;
const backgroundCache = new Map<number, string>();

// Subject ambient background as a data URI, or null when unavailable.
// Decorative by design: never throws, so callers can fire and forget.
export async function subjectBackgroundUri(
  token: string,
  subjectId: number,
): Promise<string | null> {
  const cached = backgroundCache.get(subjectId);
  if (cached !== undefined) {
    backgroundCache.delete(subjectId);
    backgroundCache.set(subjectId, cached);
    return cached;
  }
  try {
    const uri = await api.assetImage(token, `/subjects/${subjectId}/background`);
    backgroundCache.delete(subjectId);
    backgroundCache.set(subjectId, uri);
    while (backgroundCache.size > BACKGROUND_CACHE_SIZE) {
      const oldest = backgroundCache.keys().next();
      if (oldest.done) {
        break;
      }
      backgroundCache.delete(oldest.value);
    }
    return uri;
  } catch {
    return null;
  }
}
