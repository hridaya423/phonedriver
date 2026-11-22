export type Lists = Record<"EXACT" | "ACCOUNT" | "PREFIX" | "AMOUNT_VERBS" | "ORDER_VERBS" | "PAYMENT_CJK" | "ACCOUNT_CJK" | "SEND" | "SEND_CJK" | "DELETE" | "DELETE_CJK" | "INSTALL" | "INSTALL_CJK", string[]>;
export type Category = "payment" | "account" | "send" | "delete" | "install";
export type Verdict = { category: Category; why: string } | { category: null; why: string };

export const OPT_IN: Category[] = ["send", "delete", "install"];
export const WHAT: Record<Category, string> = {
  payment: "spends money or cannot be undone",
  account: "deletes an account",
  send: "sends a message or post",
  delete: "deletes something",
  install: "installs or removes an app",
};

const CODES = ["usd", "eur", "gbp", "cad", "aud", "inr", "jpy", "mxn", "chf", "cny", "brl", "krw"];
const SYMBOLS = ["$", "€", "£", "¥", "￥", "₹", "₩"];
const MAX_WORDS = 9;

const devanagari = (c: string) => c >= "ऀ" && c <= "ॿ" && c !== "।" && c !== "॥";
const cjk = (c: string) => /[ᄀ-ᇿ぀-ヿ㐀-䶿一-鿿가-힯々]/u.test(c);
const alnum = (c: string) => /[\p{Alphabetic}\p{N}]/u.test(c);
const digit = (t: string) => /[0-9]/.test(t);
const symbol = (t: string) => SYMBOLS.some((s) => t.includes(s));

export function tokens(text: string): string[] {
  const lower = [...text.toLowerCase()]
    .filter((c) => c !== "'" && c !== "’")
    .map((c) => (alnum(c) || SYMBOLS.includes(c) || c === "." || c === "," || c === "#" || devanagari(c) ? c : " "))
    .join("");
  return lower
    .split(/\s+/)
    .map((t) => t.replace(/^[.,]+|[.,]+$/g, ""))
    .filter(Boolean);
}

function amount(words: string[]): boolean {
  return words.some((w, i) => {
    const s = symbol(w);
    if (s && digit(w)) return true;
    if (s && i + 1 < words.length && digit(words[i + 1])) return true;
    if (s && i > 0 && digit(words[i - 1])) return true;
    return digit(w) && (CODES.some((c) => w.endsWith(c)) || (i + 1 < words.length && CODES.includes(words[i + 1])) || (i > 0 && CODES.includes(words[i - 1])));
  });
}

function payment(l: Lists, words: string[], phrase: string): string | null {
  if (l.EXACT.includes(phrase)) return `"${phrase}" is on the payment list`;
  const p = l.PREFIX.find((p) => phrase === p || phrase.startsWith(`${p} `));
  if (p) return `it starts with "${p}"`;
  if (words.length === 1 || !amount(words)) return null;
  const verb = words[0];
  if (l.ORDER_VERBS.includes(verb)) {
    const next = words[1];
    return next && (digit(next) || symbol(next) || next === "now" || next === "for") ? `"${verb}" with an amount` : null;
  }
  return l.AMOUNT_VERBS.includes(verb) ? `"${verb}" with an amount` : null;
}

const OTHERS = (l: Lists): [Category, string[], string[]][] => [
  ["account", l.ACCOUNT, l.ACCOUNT_CJK],
  ["send", l.SEND, l.SEND_CJK],
  ["delete", l.DELETE, l.DELETE_CJK],
  ["install", l.INSTALL, l.INSTALL_CJK],
];

function classifyCjk(l: Lists, label: string): Verdict {
  const run = label.split(/\s+/).join("");
  if ([...run].length > 40 || run.includes("#")) return { category: null, why: "too long to be a button" };
  let head = "";
  for (const c of run) {
    if (!cjk(c)) break;
    head += c;
  }
  const tail = run.slice(head.length);
  const plain = [...tail].every((c) => !alnum(c)) || [...tail].some((c) => /\p{N}/u.test(c) || SYMBOLS.includes(c));
  if (!plain) return { category: null, why: `"${head}" is followed by more text, so it is not the button's whole label` };
  if (l.PAYMENT_CJK.includes(head)) return { category: "payment", why: `"${head}" is on the payment list` };
  const hit = OTHERS(l).find(([, , list]) => list.includes(head));
  return hit ? { category: hit[0], why: `"${head}" is on the ${hit[0]} list` } : { category: null, why: `"${head}" is on no list` };
}

/** The guard's `classify` for a button, ported from crates/cli/src/daemon/guard.rs, with the reason it matched. */
export function classify(l: Lists, label: string): Verdict {
  if (!label.trim()) return { category: null, why: "it has no label, so there is nothing to read" };
  if (cjk(label.trimStart()[0])) return classifyCjk(l, label);
  const words = tokens(label);
  if (!words.length) return { category: null, why: "it has no words" };
  if (words.length > MAX_WORDS) return { category: null, why: "more than nine words reads as text, not a button" };
  if (words.some((w) => w.startsWith("#"))) return { category: null, why: "a hashtag is not a button" };
  const phrase = words.join(" ");
  const pay = payment(l, words, phrase);
  if (pay) return { category: "payment", why: pay };
  if (["delete", "permanently", "yes", "confirm"].includes(words[0]) && words.length <= 6 && ["delete account", "delete my account", "delete your account"].some((d) => phrase.includes(d)))
    return { category: "account", why: "it deletes an account" };
  const hit = OTHERS(l).find(([, list]) => list.includes(phrase));
  return hit ? { category: hit[0], why: `"${phrase}" is on the ${hit[0]} list` } : { category: null, why: `"${phrase}" is on no list` };
}

export const held = (c: Category | null, ask: Category[]) => c !== null && (!OPT_IN.includes(c) || ask.includes(c));
