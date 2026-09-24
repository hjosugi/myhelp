import { afterEach, describe, expect, it } from "vitest";
import {
  SUPPORTED_LOCALES,
  catalogs,
  en,
  errorReason,
  interpolate,
  ja,
  loadLocalePreference,
  LOCALE_STORAGE_KEY,
  MessageKey,
  OPTIONAL_PARAMETERS,
  placeholders,
  render,
  resolveLocale,
  saveLocalePreference,
  templateParts,
  translate,
  translatePlural,
} from "./i18n";

function memoryStorage(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
    values,
  };
}

const removedJapanese: Partial<Record<MessageKey, string>> = {};

afterEach(() => {
  Object.assign(ja, removedJapanese);
  for (const key of Object.keys(removedJapanese)) {
    delete removedJapanese[key as MessageKey];
  }
});

describe("locale resolution", () => {
  it("lets an explicit preference override the operating system", () => {
    expect(resolveLocale("en", ["ja-JP"])).toBe("en");
    expect(resolveLocale("ja", ["en-US"])).toBe("ja");
  });

  it("uses the first supported operating-system language", () => {
    expect(resolveLocale("system", ["ja-JP", "en-US"])).toBe("ja");
    expect(resolveLocale("system", ["fr-FR", "ja"])).toBe("ja");
    expect(resolveLocale("system", ["JA_jp"])).toBe("ja");
    expect(resolveLocale("system", ["en-GB", "ja-JP"])).toBe("en");
  });

  it("falls back to English for unsupported or missing languages", () => {
    expect(resolveLocale("system", ["fr-FR", "de"])).toBe("en");
    expect(resolveLocale("system", [])).toBe("en");
  });
});

describe("catalogs", () => {
  it("translates every English key for every supported locale", () => {
    for (const locale of SUPPORTED_LOCALES) {
      const categories = new Intl.PluralRules(locale).resolvedOptions().pluralCategories;
      const missing = (Object.keys(en) as MessageKey[]).filter((key) => {
        const plural = key.match(/\.(zero|one|two|few|many|other)$/);
        if (plural && !categories.includes(plural[1] as Intl.LDMLPluralRule)) return false;
        return catalogs[locale][key] === undefined;
      });
      expect(missing, locale).toEqual([]);
    }
  });

  it("keeps every English placeholder and adds only documented ones", () => {
    for (const locale of SUPPORTED_LOCALES) {
      for (const [key, template] of Object.entries(catalogs[locale])) {
        const source = placeholders(en[key as MessageKey]);
        const allowed = new Set([
          ...source,
          ...(OPTIONAL_PARAMETERS[key as MessageKey] ?? []),
        ]);
        const used = placeholders(template ?? "");
        expect(
          source.filter((name) => !used.includes(name)),
          `${locale} ${key} drops`,
        ).toEqual([]);
        expect(
          used.filter((name) => !allowed.has(name)),
          `${locale} ${key} adds`,
        ).toEqual([]);
      }
    }
  });

  it("does not ship keys that the English source does not define", () => {
    for (const locale of SUPPORTED_LOCALES) {
      const extra = Object.keys(catalogs[locale]).filter((key) => !(key in en));
      expect(extra, locale).toEqual([]);
    }
  });
});

describe("translation", () => {
  it("fills placeholders and leaves unknown ones visible", () => {
    expect(translate("en", "status.saved", { topic: "git" })).toBe("Saved git");
    expect(translate("ja", "status.saved", { topic: "git" })).toBe("git を保存しました");
    expect(interpolate("{known} {unknown}", { known: "x" })).toBe("x {unknown}");
  });

  it("falls back to English when a translation is missing", () => {
    removedJapanese["status.saved"] = ja["status.saved"];
    delete ja["status.saved"];
    expect(translate("ja", "status.saved", { topic: "git" })).toBe("Saved git");
  });

  it("selects plural forms per locale", () => {
    expect(translatePlural("en", "status.pageCount", 1)).toBe("1 page");
    expect(translatePlural("en", "status.pageCount", 2)).toBe("2 pages");
    expect(translatePlural("ja", "status.pageCount", 1)).toBe("1 ページ");
    expect(translatePlural("ja", "status.matchCount", 3)).toBe("一致するページ: 3 件");
  });

  it("falls back to the English plural form when a locale lacks one", () => {
    removedJapanese["status.pageCount.other"] = ja["status.pageCount.other"];
    delete ja["status.pageCount.other"];
    expect(translatePlural("ja", "status.pageCount", 1)).toBe("1 page");
  });

  it("labels storage errors in Japanese and keeps the English detail", () => {
    const error = { kind: "notFound", message: "page does not exist: git" };
    expect(errorReason("en", error)).toBe("page does not exist: git");
    expect(errorReason("ja", error)).toBe("ページが存在しません（page does not exist: git）");
    expect(errorReason("ja", { kind: "somethingNew", message: "boom" })).toBe(
      "予期しないエラー（boom）",
    );
  });

  it("renders status messages with errors in the current locale", () => {
    const message = {
      key: "status.saveFailed" as const,
      params: { topic: "git" },
      error: { kind: "conflict", message: "page changed on disk since it was read: git" },
    };
    expect(render("en", message)).toBe(
      "Could not save git: page changed on disk since it was read: git",
    );
    expect(render("ja", message)).toBe(
      "git を保存できませんでした: ページがディスク上で変更されています（page changed on disk since it was read: git）",
    );
    expect(render("en", { key: "status.pageCount", count: 1 })).toBe("1 page");
  });

  it("splits templates into text and markup slots", () => {
    expect(templateParts("ja", "undo.message")).toEqual([
      { slot: "topic" },
      { text: " は、読める形式の復元用ファイルに移動しました。" },
    ]);
  });
});

describe("preference storage", () => {
  it("stores explicit languages and forgets the system default", () => {
    const storage = memoryStorage();
    saveLocalePreference("ja", storage);
    expect(storage.values.get(LOCALE_STORAGE_KEY)).toBe("ja");
    expect(loadLocalePreference(storage)).toBe("ja");
    saveLocalePreference("system", storage);
    expect(storage.values.has(LOCALE_STORAGE_KEY)).toBe(false);
    expect(loadLocalePreference(storage)).toBe("system");
  });

  it("ignores invalid or unreadable stored values", () => {
    expect(loadLocalePreference(memoryStorage({ [LOCALE_STORAGE_KEY]: "fr" }))).toBe(
      "system",
    );
    const throwing = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
      removeItem: () => undefined,
    };
    expect(loadLocalePreference(throwing)).toBe("system");
    expect(() => saveLocalePreference("ja", throwing)).not.toThrow();
    expect(loadLocalePreference(null)).toBe("system");
  });
});
