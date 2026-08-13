import { StreamLanguage, type StreamParser } from "@codemirror/language";
import type { Extension } from "@codemirror/state";

type HtmlState = {
  comment: boolean;
  inTag: boolean;
  expectTagName: boolean;
  quote: "\"" | "'" | null;
};

const htmlStream: StreamParser<HtmlState> = {
  name: "html",
  startState: () => ({ comment: false, inTag: false, expectTagName: false, quote: null }),
  languageData: { commentTokens: { block: { open: "<!--", close: "-->" } } },
  token(stream, state) {
    if (state.comment) {
      const end = stream.string.indexOf("-->", stream.pos);
      if (end >= 0) {
        stream.pos = end + 3;
        state.comment = false;
      } else {
        stream.skipToEnd();
      }
      return "comment";
    }

    if (state.quote) {
      const end = stream.string.indexOf(state.quote, stream.pos);
      if (end >= 0) {
        stream.pos = end + 1;
        state.quote = null;
      } else {
        stream.skipToEnd();
      }
      return "string";
    }

    if (stream.eatSpace()) return null;

    if (!state.inTag) {
      if (stream.match("<!--")) {
        state.comment = true;
        const end = stream.string.indexOf("-->", stream.pos);
        if (end >= 0) {
          stream.pos = end + 3;
          state.comment = false;
        } else {
          stream.skipToEnd();
        }
        return "comment";
      }
      if (stream.match(/^<!DOCTYPE\b[^>]*>/i)) return "meta";
      if (stream.match(/^<\/?/)) {
        state.inTag = true;
        state.expectTagName = true;
        return "punctuation";
      }
      if (stream.match(/^&(?:#\d+|#x[\da-f]+|[a-z][\w-]*);/i)) return "atom";
      if (stream.match(/^[^<&]+/)) return null;
      stream.match(/^./);
      return null;
    }

    if (state.expectTagName && stream.match(/^[A-Za-z][\w:-]*/)) {
      state.expectTagName = false;
      return "tagName";
    }
    if (stream.match(/^\/?>/)) {
      state.inTag = false;
      state.expectTagName = false;
      return "punctuation";
    }
    if (stream.match(/^=/)) return "operator";
    if (stream.match(/^["']/)) {
      state.quote = stream.current() as "\"" | "'";
      return "string";
    }
    if (stream.match(/^[A-Za-z_:][\w:.-]*/)) return "attributeName";
    stream.match(/^./);
    return "punctuation";
  },
};

type TomlState = {
  multilineString: "\"\"\"" | "'''" | null;
};

const tomlStream: StreamParser<TomlState> = {
  name: "toml",
  startState: () => ({ multilineString: null }),
  languageData: { commentTokens: { line: "#" } },
  token(stream, state) {
    if (state.multilineString) {
      const end = stream.string.indexOf(state.multilineString, stream.pos);
      if (end >= 0) {
        stream.pos = end + 3;
        state.multilineString = null;
      } else {
        stream.skipToEnd();
      }
      return "string";
    }

    if (stream.eatSpace()) return null;
    if (stream.match(/^#.*/)) return "comment";
    if (stream.match(/^\[\[[^\]]+\]\]|^\[[^\]]+\]/)) return "typeName";
    if (stream.match(/^(?:"""|''')/)) {
      state.multilineString = stream.current() as "\"\"\"" | "'''";
      return "string";
    }
    if (stream.match(/^"(?:\\.|[^"\\])*"|^'[^']*'/)) return "string";
    if (stream.match(/^[A-Za-z0-9_-]+(?=\s*=)/)) return "propertyName";
    if (stream.match(/^(?:true|false)\b/)) return "bool";
    if (stream.match(/^\d{4}-\d{2}-\d{2}(?:[Tt ][\d:.+-]+[Zz]?)?/)) return "atom";
    if (stream.match(/^[+-]?(?:0x[\da-fA-F_]+|0o[0-7_]+|0b[01_]+|(?:\d[\d_]*)(?:\.[\d_]+)?(?:[eE][+-]?[\d_]+)?|inf|nan)\b/)) return "number";
    if (stream.match(/^[=.,{}\[\]]/)) return "punctuation";
    stream.match(/^./);
    return null;
  },
};

type YamlState = {
  quoted: "\"" | "'" | null;
};

const yamlStream: StreamParser<YamlState> = {
  name: "yaml",
  startState: () => ({ quoted: null }),
  languageData: { commentTokens: { line: "#" } },
  token(stream, state) {
    if (state.quoted) {
      const end = stream.string.indexOf(state.quoted, stream.pos);
      if (end >= 0) {
        stream.pos = end + 1;
        state.quoted = null;
      } else {
        stream.skipToEnd();
      }
      return "string";
    }

    if (stream.eatSpace()) return null;
    if (stream.match(/^#.*/)) return "comment";
    if (stream.sol() && stream.match(/^(?:---|\.\.\.)(?:\s|$)/)) return "meta";
    if (stream.match(/^["']/)) {
      state.quoted = stream.current() as "\"" | "'";
      return "string";
    }
    if (stream.match(/^![\w!:/.-]+/)) return "typeName";
    if (stream.match(/^[&*][\w.-]+/)) return "variableName";
    if (stream.match(/^[^\s\[\]{},#][^:#\[\]{},]*(?=\s*:)/)) return "propertyName";
    if (stream.match(/^(?:true|false|null|~|yes|no|on|off)\b/i)) return "atom";
    if (stream.match(/^[+-]?(?:\d[\d_]*)?(?:\.[\d_]+)?(?:[eE][+-]?\d+)?\b/) && stream.current().length > 0) return "number";
    if (stream.match(/^(?:\||>|-)(?:[+-]?\d*)?/)) return "operator";
    if (stream.match(/^[\[\]{},?:]/)) return "punctuation";
    if (stream.match(/^[^\s#\[\]{},:]+/)) return "string";
    stream.match(/^./);
    return null;
  },
};

const htmlLanguage = StreamLanguage.define(htmlStream);
const tomlLanguage = StreamLanguage.define(tomlStream);
const yamlLanguage = StreamLanguage.define(yamlStream);

export function editorLanguageExtension(language: string, _path: string): Extension {
  if (language === "html") return htmlLanguage;
  if (language === "toml") return tomlLanguage;
  if (language === "yaml") return yamlLanguage;
  return [];
}

export async function loadEditorLanguageExtension(language: string, path: string): Promise<Extension> {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  if (language === "rust") {
    const { rust } = await import("@codemirror/lang-rust");
    return rust();
  }
  if (language === "typescript") {
    const { javascript } = await import("@codemirror/lang-javascript");
    return javascript({ typescript: true, jsx: extension === "tsx" });
  }
  if (language === "javascript") {
    const { javascript } = await import("@codemirror/lang-javascript");
    return javascript({ jsx: extension === "jsx" });
  }
  if (language === "json") {
    const { json } = await import("@codemirror/lang-json");
    return json();
  }
  return editorLanguageExtension(language, path);
}
