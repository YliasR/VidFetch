/**
 * Maps raw yt-dlp / app failure output to a known cause with a suggested fix.
 *
 * Classification scans the queue item's error message plus captured log
 * lines. Patterns are ordered most-specific first; the first match wins.
 */

export type RecoveryAction = 'update-ytdlp' | 'install-ytdlp' | 'reinstall-ffmpeg';

export interface ErrorHint {
  kind: string;
  title: string;
  hint: string;
  /** One-click fix the UI can run before retrying, if any. */
  action: RecoveryAction | null;
  actionLabel: string | null;
}

interface Rule {
  kind: string;
  pattern: RegExp;
  title: string;
  hint: string;
  action?: RecoveryAction;
  actionLabel?: string;
}

const RULES: Rule[] = [
  {
    kind: 'ytdlp-missing',
    pattern: /yt-dlp binary missing/i,
    title: 'yt-dlp is missing',
    hint: 'The yt-dlp binary was not found. Install it and the download can run.',
    action: 'install-ytdlp',
    actionLabel: 'Install yt-dlp & retry',
  },
  {
    kind: 'ffmpeg-missing',
    pattern: /ffmpeg (?:binary )?(?:missing|not found|could not be found)|ffmpeg is not installed|ffmpeg-location .* does not exist/i,
    title: 'ffmpeg is missing',
    hint: 'Merging and conversion need ffmpeg. Reinstalling the bundled build usually fixes this.',
    action: 'reinstall-ffmpeg',
    actionLabel: 'Reinstall ffmpeg & retry',
  },
  {
    kind: 'sign-in-required',
    pattern: /sign in to confirm|login required|private video|age[ -]restricted|members[ -]only|member-only|this video is available to this channel's members|use --cookies/i,
    title: 'Sign-in required',
    hint: 'The site wants a logged-in session. Set Cookies in Advanced options (from your browser or a cookies.txt) and retry.',
  },
  {
    kind: 'geo-blocked',
    pattern: /not available in your country|geo[ -]?restrict|blocked it in your country/i,
    title: 'Region-blocked',
    hint: 'The uploader blocked this content for your region. A VPN or proxy is the only workaround.',
  },
  {
    kind: 'gone',
    pattern: /video unavailable|has been removed|account .* terminated|no longer available|content isn't available/i,
    title: 'Content unavailable',
    hint: 'The video looks deleted or taken down — retrying won’t help.',
  },
  {
    kind: 'rate-limited',
    pattern: /HTTP Error 429|too many requests|rate[ -]?limit/i,
    title: 'Rate-limited',
    hint: 'The site is throttling you. Wait a bit before retrying, or set a Rate limit in Advanced options to stay under the radar.',
  },
  {
    kind: 'extractor-stale',
    pattern: /unable to extract|signature extraction failed|nsig extraction failed|player response|confirm you.?re not a bot/i,
    title: 'yt-dlp looks outdated',
    hint: 'Extraction failures usually mean the site changed and yt-dlp needs its weekly update.',
    action: 'update-ytdlp',
    actionLabel: 'Update yt-dlp & retry',
  },
  {
    kind: 'http-403',
    pattern: /HTTP Error 403|403 Forbidden|status code 403/i,
    title: 'Blocked with HTTP 403',
    hint: 'A 403 from the site is most often fixed by updating yt-dlp. If it persists, try Cookies in Advanced options.',
    action: 'update-ytdlp',
    actionLabel: 'Update yt-dlp & retry',
  },
  {
    kind: 'unsupported-url',
    pattern: /unsupported url|is not a valid url/i,
    title: 'Unsupported URL',
    hint: 'yt-dlp has no extractor for this address. Double-check the link, or paste the direct video page URL.',
  },
  {
    kind: 'disk-full',
    pattern: /no space left|not enough (?:free )?(?:disk )?space|disk full/i,
    title: 'Out of disk space',
    hint: 'Free up space on the output drive (or pick a different folder) and retry.',
  },
  {
    kind: 'network',
    pattern: /getaddrinfo failed|temporary failure in name resolution|network is unreachable|connection (?:reset|refused|aborted|timed out)|timed out|ssl: |certificate verify failed|unable to download webpage/i,
    title: 'Network trouble',
    hint: 'Looks like a connection hiccup between you and the site. Check your internet and retry.',
  },
];

export function classifyError(
  message: string | null | undefined,
  logLines?: string[]
): ErrorHint | null {
  const haystack = [message ?? '', ...(logLines ?? [])].join('\n');
  if (!haystack.trim()) return null;

  for (const rule of RULES) {
    if (rule.pattern.test(haystack)) {
      return {
        kind: rule.kind,
        title: rule.title,
        hint: rule.hint,
        action: rule.action ?? null,
        actionLabel: rule.actionLabel ?? null,
      };
    }
  }
  return null;
}
